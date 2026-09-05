use std::{
    collections::{
        HashMap,
        HashSet,
    },
    sync::Arc,
    time::{
        Duration,
        Instant,
    },
};

use rayon::iter::{
    IntoParallelIterator,
    ParallelIterator,
};
use tokio::{
    task,
    time::sleep,
};
use wana_kana::IsJapaneseStr;

use super::{
    api::{
        get_field_names,
        get_intervals,
        get_model_ids,
        get_note_ids,
        get_notes,
        get_version,
    },
    scoring::{
        MatchEvidence,
        MatchResult,
    },
    types::{
        FieldMapping,
        Model,
        Vocab,
    },
};
use crate::{
    anki::comprehensibility::comp_term,
    core::{
        utils::{
            normalize_japanese_text,
            FilterKana,
            NormalizeLongVowel,
        },
        Term,
    },
    dictionary::frequency_manager::FrequencyManager,
};

pub(crate) const ANKI_VOCAB_CACHE: &str = "anki_vocab_cache.json";

pub struct AnkiState {
    vocab: Vec<Vocab>,
    frequency_manager: Arc<FrequencyManager>,
    relevance_map: HashMap<String, Vec<usize>>, // Map to indices
    known_interval: u32,                        // From settings, for calculating comprehension
}

impl AnkiState {
    pub async fn new(
        model_mapping: HashMap<String, FieldMapping>,
        frequency_manager: Arc<FrequencyManager>,
        known_interval: u32,
    ) -> Result<Self, reqwest::Error> {
        let start = Instant::now();
        let mut vocab = get_total_vocab(&model_mapping).await?;
        println!(
            "Loaded {} vocab items from Anki ({:.1}s)",
            vocab.len(),
            start.elapsed().as_secs_f32()
        );

        // Fetch card intervals and set them on vocab
        let card_ids: Vec<u64> = vocab.iter().filter_map(|v| v.card_id).collect();

        let intervals_request_start = Instant::now();
        let intervals = get_intervals(card_ids.clone()).await?;
        println!(
            "  getIntervals request: {} cards ({:.2}s)",
            card_ids.len(),
            intervals_request_start.elapsed().as_secs_f32()
        );

        let processing_start = Instant::now();
        let card_intervals: HashMap<u64, i32> =
            card_ids.into_iter().zip(intervals.into_iter()).collect();

        // Set intervals on vocab items
        let mut intervals_set = 0;
        for vocab_item in &mut vocab {
            if let Some(card_id) = vocab_item.card_id {
                if let Some(&interval) = card_intervals.get(&card_id) {
                    // Negative intervals are in seconds (learning/relearning), positive in days
                    vocab_item.interval = Some(if interval >= 0 {
                        interval as f32
                    } else {
                        interval.abs() as f32 / 86400.0
                    });
                    intervals_set += 1;
                }
            }
        }
        println!(
            "  Processing intervals: {}/{} set ({:.2}s)",
            intervals_set,
            vocab.len(),
            processing_start.elapsed().as_secs_f32()
        );

        // Persist the freshly fetched vocab so it can be reused offline / for fast loads
        if vocab.is_empty() {
            eprintln!("Anki returned no vocab; keeping the existing cache");
        } else if let Err(e) = crate::persistence::save_json(&vocab, ANKI_VOCAB_CACHE) {
            eprintln!("Failed to save Anki vocab cache: {}", e);
        }

        println!("AnkiState initialized ({:.1}s total)", start.elapsed().as_secs_f32());
        Ok(Self::from_vocab(vocab, frequency_manager, known_interval))
    }

    /// Build an `AnkiState` from an already-fetched vocab list (no network).
    fn from_vocab(
        vocab: Vec<Vocab>,
        frequency_manager: Arc<FrequencyManager>,
        known_interval: u32,
    ) -> Self {
        let relevance_map = Self::build_relevance_map(&vocab);
        Self { vocab, frequency_manager, relevance_map, known_interval }
    }

    /// Build an `AnkiState` from the on-disk vocab cache, if one exists. Returns
    /// `None` when no cache is present so callers can fall back gracefully.
    pub fn from_cache(
        frequency_manager: Arc<FrequencyManager>,
        known_interval: u32,
    ) -> Option<Self> {
        let vocab: Vec<Vocab> = crate::persistence::load_json_or_default(ANKI_VOCAB_CACHE);
        if vocab.is_empty() {
            return None;
        }
        println!("Loaded {} vocab items from Anki cache", vocab.len());
        Some(Self::from_vocab(vocab, frequency_manager, known_interval))
    }

    /// One time map for the anki vocab to quickly find the potential matches by key
    fn build_relevance_map(vocab: &[Vocab]) -> HashMap<String, Vec<usize>> {
        let mut relevance_map: HashMap<String, Vec<usize>> = HashMap::new();

        for (index, vocab_item) in vocab.iter().enumerate() {
            relevance_map.entry(vocab_item.reading.clone()).or_insert_with(Vec::new).push(index);
            relevance_map.entry(vocab_item.term.clone()).or_insert_with(Vec::new).push(index);
            relevance_map
                .entry(normalize_japanese_text(vocab_item.reading.as_str()))
                .or_insert_with(Vec::new)
                .push(index);
            relevance_map
                .entry(normalize_japanese_text(vocab_item.term.as_str()))
                .or_insert_with(Vec::new)
                .push(index);
        }

        relevance_map
    }

    fn classify<'a>(
        &'a self,
        surface: &str,
        surface_reading: &str,
        citation: &str,
        reading: &str,
        pos: &crate::segmentation::word::POS,
        promoted: bool,
    ) -> MatchResult<'a> {
        use crate::segmentation::word::POS;
        for (form, reading, evidence) in [
            (surface, surface_reading, MatchEvidence::ExactSurface),
            (citation, reading, MatchEvidence::Citation),
        ] {
            if form.is_empty() {
                continue;
            }
            let normalized = normalize_japanese_text(form);
            if let Some(card) =
                self.cards_read_as(reading).find(|v| normalize_japanese_text(&v.term) == normalized)
            {
                return MatchResult::Known { card, evidence };
            }
        }
        if !matches!(
            pos,
            POS::Noun
                | POS::ProperNoun
                | POS::CompoundNoun
                | POS::NounExpression
                | POS::Verb
                | POS::SuruVerb
                | POS::Adjective
                | POS::AdjectivalNoun
                | POS::Adverb
        ) && !promoted
        {
            return MatchResult::Unmatched;
        }
        let evidence = self.frequency_manager.lexical_families(reading);
        let cards: Vec<_> = self.cards_read_as(reading).collect();
        let belongs = |family: &crate::dictionary::lexical_evidence::LexicalFamily,
                       card: &Vocab| {
            family
                .spellings
                .iter()
                .any(|s| normalize_japanese_text(s) == normalize_japanese_text(&card.term))
        };
        let written: Vec<_> = evidence.written_families().collect();
        if surface.is_kana() {
            let supported =
                cards.iter().copied().find(|card| written.iter().any(|f| belongs(f, card)));
            if let Some(card) = supported {
                if !written.is_empty()
                    && written.iter().all(|f| cards.iter().any(|card| belongs(f, card)))
                {
                    return MatchResult::Known { card, evidence: MatchEvidence::LexicalFamily };
                }
                for family in &written {
                    if let Some(card) = cards.iter().copied().find(|card| belongs(family, card)) {
                        if self.frequency_manager.frequency_favors_family(&evidence, family) {
                            return MatchResult::Known {
                                card,
                                evidence: MatchEvidence::FrequencySupported,
                            };
                        }
                    }
                }
                return MatchResult::Possible { card };
            }
            // A matching reading proposes a card, but no candidates establish identity.
            return cards
                .first()
                .map_or(MatchResult::Unmatched, |card| MatchResult::Possible { card });
        }
        if let Some(family) = evidence.family_for(citation).or_else(|| evidence.family_for(surface))
        {
            if let Some(card) = cards.iter().copied().find(|card| belongs(family, card)) {
                return MatchResult::Known { card, evidence: MatchEvidence::LexicalFamily };
            }
            if self.frequency_manager.frequency_favors_family(&evidence, family) {
                if let Some(card) = cards.iter().copied().find(|card| card.term.as_str().is_kana())
                {
                    return MatchResult::Known {
                        card,
                        evidence: MatchEvidence::FrequencySupported,
                    };
                }
            }
        }
        MatchResult::Unmatched
    }

    fn cards_read_as<'a>(&'a self, reading: &str) -> impl Iterator<Item = &'a Vocab> {
        let reading = normalize_japanese_text(reading);
        self.relevance_map
            .get(&reading)
            .into_iter()
            .flatten()
            .map(|&i| &self.vocab[i])
            .filter(move |card| normalize_japanese_text(&card.reading) == reading)
    }

    pub fn filter_existing_terms(&self, terms: Vec<Term>) -> (Vec<Term>, Vec<Term>) {
        let classified: Vec<_> = terms
            .into_par_iter()
            .map(|mut term| {
                term.possible_known_match = None;
                term.comprehension = 0.0;
                let known = match self.classify(
                    &term.surface_form,
                    &term.surface_reading,
                    &term.lemma_form,
                    &term.lemma_reading,
                    &term.part_of_speech,
                    term.lexical_family.is_some(),
                ) {
                    MatchResult::Known { card, evidence: _ } => {
                        term.comprehension =
                            comp_term(card.interval.or(Some(1.0)), self.known_interval);
                        true
                    }
                    MatchResult::Possible { card } => {
                        term.possible_known_match = Some(card.term.clone());
                        false
                    }
                    MatchResult::Unmatched => false,
                };
                (term, known)
            })
            .collect();
        let mut unknown = Vec::new();
        let mut known = Vec::new();
        for (term, established) in classified {
            if established {
                known.push(term);
            } else {
                unknown.push(term);
            }
        }
        (unknown, known)
    }

    pub fn word_stats(
        &self,
        term: &str,
        reading: &str,
        pos: &crate::segmentation::word::POS,
    ) -> (bool, f32) {
        let promoted = *pos == crate::segmentation::word::POS::Expression
            && self.frequency_manager.lexical_families(reading).phrase_family(term).is_some();
        match self.classify(term, reading, term, reading, pos, promoted) {
            MatchResult::Known { card, .. } => {
                (true, comp_term(card.interval.or(Some(1.0)), self.known_interval))
            }
            _ => (false, 0.0),
        }
    }

    pub fn vocab(&self) -> &[Vocab] {
        &self.vocab
    }

    pub fn known_interval(&self) -> u32 {
        self.known_interval
    }
}

pub async fn get_total_vocab(
    model_mapping: &HashMap<String, FieldMapping>,
) -> Result<Vec<Vocab>, reqwest::Error> {
    let deck_query = "deck:*";

    let note_ids_start = Instant::now();
    let note_ids = get_note_ids(&deck_query).await?;
    println!(
        "  findNotes request: {} notes ({:.2}s)",
        note_ids.len(),
        note_ids_start.elapsed().as_secs_f32()
    );

    let notes_start = Instant::now();
    let notes = get_notes(note_ids).await?;
    let notes_request_time = notes_start.elapsed();
    println!(
        "  notesInfo request: {} notes ({:.2}s)",
        notes.len(),
        notes_request_time.as_secs_f32()
    );

    // Harvest sentence-field values while every note is in hand (issue #3).
    let mined_sentences: Vec<super::mined::MinedSentence> = notes
        .iter()
        .filter_map(|note| {
            let mapping = model_mapping.get(&note.model_name)?;
            let sentence_field = mapping.sentence_field.as_ref()?;
            let value = &note.fields.get(sentence_field)?.value;
            let normalized = super::mined::normalize_sentence(value);
            (!normalized.is_empty()).then_some(super::mined::MinedSentence {
                note_id: note.note_id,
                sentence: normalized,
            })
        })
        .collect();
    super::mined::save_harvested_sentences(&mined_sentences);

    let processing_start = Instant::now();
    let relevant_models: HashSet<&String> = model_mapping.keys().collect();
    let vocab: Vec<Vocab> = notes
        .into_par_iter()
        .filter_map(|note| {
            if relevant_models.contains(&note.model_name) {
                if let Some(field_mapping) = model_mapping.get(&note.model_name) {
                    let term = note.fields.get(&field_mapping.term_field).map(|f| f.value.clone());
                    let reading =
                        note.fields.get(&field_mapping.reading_field).map(|f| f.value.clone());
                    if let (Some(term), Some(mut reading)) = (term, reading) {
                        if reading.trim().is_empty() && term.as_str().is_kana() {
                            reading = term.clone();
                        }

                        return Some(Vocab {
                            term,
                            reading: reading.filter_kana().normalize_long_vowel().into_owned(),
                            card_id: note.cards.first().copied(),
                            interval: None, // Will be set after fetching cards
                        });
                    }
                }
            }
            None
        })
        .collect();

    println!(
        "  Processing notes: {} vocab items ({:.2}s)",
        vocab.len(),
        processing_start.elapsed().as_secs_f32()
    );

    Ok(vocab)
}

pub async fn get_models() -> Result<Vec<Model>, reqwest::Error> {
    let model_ids = get_model_ids().await?;

    let handles: Vec<_> = model_ids
        .into_iter()
        .map(|(model_name, id)| {
            task::spawn(async move {
                let fields = get_field_names(&model_name).await?;

                // Get note count for this model
                let query = if model_name.contains(' ')
                    || model_name.contains(':')
                    || model_name.contains('"')
                {
                    format!("note:\"{}\"", model_name.replace('"', "\\\""))
                } else {
                    format!("note:{}", model_name)
                };

                let note_count = match get_note_ids(&query).await {
                    Ok(note_ids) => note_ids.len(),
                    Err(_) => 0,
                };

                // Skip models with no notes
                if note_count == 0 {
                    return Ok(None); // Return None to filter out later
                }

                Ok::<Option<Model>, reqwest::Error>(Some(Model {
                    name: model_name,
                    id,
                    fields,
                    note_count,
                    sample_note: None, // Will be loaded separately
                }))
            })
        })
        .collect();

    let models: Vec<Model> = futures::future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|result| result.ok())
        .filter_map(|inner_result| inner_result.ok())
        .flatten()
        .collect();

    Ok(models)
}

pub async fn wait_awake(wait_time: u64, max_attempts: u32) -> Result<bool, reqwest::Error> {
    for attempt in 1..=max_attempts {
        match get_version().await {
            Ok(version) => {
                println!("AnkiConnect is online. Version: {}", version);
                return Ok(true);
            }
            Err(err) => {
                println!(
                    "AnkiConnect attempt {} of {} failed. Retrying in {} seconds... Error: {}",
                    attempt, max_attempts, wait_time, err
                );
                if attempt < max_attempts {
                    sleep(Duration::from_secs(wait_time)).await;
                }
            }
        }
    }
    Ok(false)
}

pub async fn get_sample_note_for_model(
    model_name: &str,
) -> Result<Option<HashMap<String, String>>, reqwest::Error> {
    use super::api::get_sample_note_for_model;

    match get_sample_note_for_model(model_name).await? {
        Some(note) => {
            let mut sample_fields = HashMap::new();
            for (field_name, field) in note.fields {
                sample_fields.insert(field_name, field.value);
            }
            Ok(Some(sample_fields))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod classification_tests {
    use super::*;
    use crate::{
        core::Sentence,
        dictionary::{
            frequency_dict::FrequencyDictionary,
            JsonFrequency,
            JsonFrequencyData,
            TermMetaBankV3,
        },
        segmentation::{
            tokenizer::extract_words,
            word::POS,
        },
    };
    fn dictionary(name: &str, entries: &[(&str, &str, u32)]) -> FrequencyDictionary {
        let metas = entries
            .iter()
            .map(|(term, reading, rank)| TermMetaBankV3 {
                term: term.to_string(),
                data_type: "freq".to_string(),
                data: Some(if reading.is_empty() {
                    JsonFrequencyData::Simple(JsonFrequency::Number(*rank))
                } else {
                    JsonFrequencyData::Nested {
                        reading: reading.to_string(),
                        frequency: JsonFrequency::Number(*rank),
                    }
                }),
            })
            .collect();
        FrequencyDictionary::new(name.to_string(), "1".to_string(), metas)
    }

    fn state(frequencies: Arc<FrequencyManager>, cards: &[(&str, &str)]) -> AnkiState {
        AnkiState::from_vocab(
            cards
                .iter()
                .map(|(term, reading)| Vocab {
                    term: term.to_string(),
                    reading: reading.to_string(),
                    card_id: Some(1),
                    interval: Some(400.0),
                })
                .collect(),
            frequencies,
            21,
        )
    }
    fn term(surface: &str, reading: &str) -> Term {
        Term {
            possible_known_match: None,
            lexical_family: None,
            id: 0,
            lemma_form: surface.into(),
            lemma_reading: reading.into(),
            surface_form: surface.into(),
            surface_reading: reading.into(),
            is_kana: surface.is_kana(),
            part_of_speech: POS::Noun,
            frequencies: HashMap::new(),
            full_segment: surface.into(),
            full_segment_reading: reading.into(),
            sentence_references: vec![],
            comprehension: 0.0,
            jlpt_level: None,
        }
    }
    fn linked_dictionary(name: &str, rival: u32, kana: u32) -> FrequencyDictionary {
        let mut dict = dictionary(name, &[("行く", "いく", 44), ("逝く", "いく", rival)]);
        dict.terms.get_mut("行く").unwrap().push(crate::dictionary::CacheFrequencyData::Nested {
            reading: "イク".into(),
            frequency: crate::dictionary::CacheFrequency::Complex {
                value: kana,
                display_value: Some(format!("{kana}㋕")),
            },
        });
        dict
    }

    #[test]
    fn linked_common_pair_filters_in_both_directions_and_clears_stale_labels() {
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![linked_dictionary(
            "linked", 9328, 65,
        )]));
        manager.set_dictionary_state("linked", 0.0, false).unwrap();
        let known = state(manager.clone(), &[("行く", "いく")]);
        let mut input = term("いく", "いく");
        input.possible_known_match = Some("old".into());
        let (unknown, filtered) = known.filter_existing_terms(vec![input.clone()]);
        assert!(unknown.is_empty());
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].possible_known_match.is_none());
        assert!(filtered[0].comprehension > 0.0);
        assert!(known.word_stats("いく", "いく", &POS::Verb).0);
        assert!(matches!(
            known.classify("いく", "いく", "いく", "いく", &POS::Verb, false),
            MatchResult::Known { evidence: MatchEvidence::FrequencySupported, .. }
        ));
        let reverse = state(manager.clone(), &[("いく", "いく")]);
        assert!(reverse.word_stats("行く", "いく", &POS::Verb).0);
        assert!(!reverse.word_stats("逝く", "いく", &POS::Verb).0);
        let removed = state(manager.clone(), &[]);
        let (unknown, filtered) = removed.filter_existing_terms(vec![input]);
        assert!(filtered.is_empty());
        assert!(unknown[0].possible_known_match.is_none());
        assert_eq!(unknown[0].comprehension, 0.0);
        // Frequency chooses the common family, not whichever family has a card.
        let rare = state(manager, &[("逝く", "いく")]);
        assert!(!rare.word_stats("いく", "いく", &POS::Verb).0);
    }

    #[test]
    fn linked_pair_requires_common_comparable_ranks_and_complete_uncontested_evidence() {
        let cases = vec![
            vec![linked_dictionary("close", 650, 65)],
            vec![linked_dictionary("uneven", 9328, 200)],
            vec![linked_dictionary("zero", 9328, 0)],
            vec![
                linked_dictionary("linked", 9328, 65),
                dictionary("conflict", &[("行く", "いく", 44), ("逝く", "いく", 100)]),
            ],
            vec![
                linked_dictionary("linked", 9328, 65),
                dictionary("missing", &[("異口", "いく", 20000)]),
            ],
            vec![dictionary(
                "unlinked",
                &[("行く", "いく", 44), ("いく", "いく", 65), ("逝く", "いく", 9328)],
            )],
        ];
        for dictionaries in cases {
            let known = state(
                Arc::new(FrequencyManager::from_dictionaries(dictionaries)),
                &[("行く", "いく")],
            );
            let (unknown, filtered) = known.filter_existing_terms(vec![term("いく", "いく")]);
            assert!(filtered.is_empty());
            assert_eq!(unknown[0].possible_known_match.as_deref(), Some("行く"));
            assert_eq!(unknown[0].comprehension, 0.0);
            assert!(!known.word_stats("いく", "いく", &POS::Verb).0);
        }
    }

    fn promoted_nantonaku() -> Option<(Term, Arc<FrequencyManager>)> {
        let tokenizer = crate::segmentation::lexeme_resolver::test_tokenizer()?;
        let frequencies = FrequencyManager::from_dictionaries(vec![
            dictionary("Ranked", &[("なんとなく", "", 2231), ("何と無く", "なんとなく", 1686)]),
            dictionary("Unranked", &[("何となく", "なんとなく", 2016)]),
        ]);
        frequencies.set_dictionary_state("Unranked", 0.0, false).unwrap();

        let mut sentences = vec![Sentence {
            id: 0,
            source_id: 0,
            text: "なんとなく".to_string(),
            segments: Vec::new(),
            timestamp: None,
            comprehension: 0.0,
        }];
        let terms = extract_words(tokenizer.new_worker(), &mut sentences, &frequencies);
        let phrase = terms
            .into_iter()
            .find(|t| t.surface_form == "なんとなく")
            .expect("production extraction must promote なんとなく");
        Some((phrase, Arc::new(frequencies)))
    }

    #[test]
    fn promoted_identity_preserves_surface_frequency_and_matches_other_spelling() {
        let (phrase, manager) = promoted_nantonaku().expect("UniDic is required");
        assert!(phrase.lexical_family.is_some());
        assert_eq!(phrase.lemma_form, "なんとなく");
        assert_eq!(phrase.frequencies.get("Ranked"), Some(&2231));
        let known = state(manager.clone(), &[("何となく", "なんとなく")]);
        assert!(known.filter_existing_terms(vec![phrase.clone()]).0.is_empty());
        assert!(known.word_stats("なんとなく", "なんとなく", &POS::Expression).0);
        assert_eq!(state(manager, &[]).filter_existing_terms(vec![phrase]).0.len(), 1);
    }
    #[test]
    fn card_changes_clear_labels_and_stats_agree() {
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![dictionary(
            "test",
            &[("橋", "はし", 1), ("箸", "はし", 9000)],
        )]));
        let known = state(manager.clone(), &[("橋", "はし")]);
        let (possible, established) = known.filter_existing_terms(vec![term("はし", "はし")]);
        assert!(established.is_empty());
        assert_eq!(possible[0].possible_known_match.as_deref(), Some("橋"));
        assert_eq!(possible[0].comprehension, 0.0);
        assert_eq!(known.word_stats("はし", "はし", &POS::Noun), (false, 0.0));
        let removed = state(manager.clone(), &[]);
        let (unmatched, _) = removed.filter_existing_terms(possible.clone());
        assert_eq!(unmatched[0].possible_known_match, None);
        let established = state(manager, &[("橋", "はし"), ("箸", "はし")]);
        let (unknown, known) = established.filter_existing_terms(possible);
        assert!(unknown.is_empty());
        assert_eq!(known[0].possible_known_match, None);
        assert!(known[0].comprehension > 0.0);
    }
    #[test]
    fn zero_candidates_and_excluded_pos_do_not_infer_identity() {
        let state = state(
            Arc::new(FrequencyManager::from_dictionaries(vec![])),
            &[("騙す", "だます"), ("何", "なん")],
        );
        assert_eq!(state.word_stats("だます", "だます", &POS::Verb), (false, 0.0));
        assert_eq!(state.word_stats("なん", "なん", &POS::Pronoun), (false, 0.0));
        assert!(!state.word_stats("ダマス", "だます", &POS::Verb).0);
        assert!(state.word_stats("騙す", "だます", &POS::Verb).0);
        assert!(!state.word_stats("", "なん", &POS::Expression).0);
    }
    #[test]
    fn exact_citation_and_script_normalization_remain_known() {
        let state = state(
            Arc::new(FrequencyManager::from_dictionaries(vec![])),
            &[("する", "する"), ("ハシ", "はし")],
        );
        let mut inflected = term("した", "した");
        inflected.lemma_form = "する".into();
        inflected.lemma_reading = "する".into();
        assert!(state.filter_existing_terms(vec![inflected]).0.is_empty());
        assert!(state.word_stats("はし", "ハシ", &POS::Noun).0);
    }
    #[test]
    fn production_ambiguous_readings_offer_possible_cards_without_becoming_known() {
        let Some(tokenizer) = crate::segmentation::lexeme_resolver::test_tokenizer() else {
            return;
        };
        let cases = vec![
            (
                "うまい",
                "うまい",
                "上手い",
                "旨い",
                vec![
                    dictionary(
                        "JPDB",
                        &[
                            ("上手い", "うまい", 3055),
                            ("旨い", "うまい", 15639),
                            ("美い", "うまい", 62978),
                            ("甘い", "うまい", 203801),
                        ],
                    ),
                    dictionary(
                        "Jiten",
                        &[
                            ("上手い", "うまい", 3267),
                            ("旨い", "うまい", 14231),
                            ("美い", "うまい", 33311),
                            ("甘い", "うまい", 196331),
                        ],
                    ),
                    dictionary("BCCWJ", &[("旨い", "", 375)]),
                    dictionary("CC100", &[("右舞", "うまい", 127616)]),
                ],
            ),
            (
                "大事なことだ。",
                "こと",
                "事",
                "事",
                vec![
                    dictionary(
                        "JPDB",
                        &[("事", "こと", 497), ("古都", "こと", 41264), ("縡", "こと", 208767)],
                    ),
                    dictionary(
                        "Jiten",
                        &[("事", "こと", 614), ("古都", "こと", 45881), ("縡", "こと", 380694)],
                    ),
                    dictionary("BCCWJ", &[("事", "", 15), ("言", "", 8490)]),
                    dictionary("CC100", &[("湖都", "こと", 114543)]),
                ],
            ),
            (
                "明日くると思う。",
                "くる",
                "来る",
                "来る",
                vec![
                    dictionary("JPDB", &[("来る", "くる", 53), ("繰る", "くる", 15253)]),
                    dictionary("Jiten", &[("来る", "くる", 57), ("繰る", "くる", 22943)]),
                    dictionary("BCCWJ", &[("来る", "", 56), ("繰る", "", 8195)]),
                    dictionary("CC100", &[("刳る", "くる", 130946)]),
                ],
            ),
            (
                "学校にいく。",
                "いく",
                "行く",
                "行く",
                vec![
                    dictionary(
                        "JPDB",
                        &[
                            ("行く", "いく", 44),
                            ("往く", "いく", 18835),
                            ("逝く", "いく", 9328),
                            ("幾", "いく", 6358),
                        ],
                    ),
                    dictionary(
                        "Jiten",
                        &[
                            ("行く", "いく", 48),
                            ("往く", "いく", 30482),
                            ("逝く", "いく", 6894),
                            ("幾", "いく", 12949),
                        ],
                    ),
                    dictionary(
                        "BCCWJ",
                        &[("行く", "", 58), ("逝く", "", 10637), ("幾", "", 72576)],
                    ),
                    dictionary("VN Freq", &[("異口", "いく", 31496)]),
                ],
            ),
            (
                "あとで話す。",
                "あと",
                "後",
                "後",
                vec![
                    dictionary(
                        "JPDB",
                        &[("後", "あと", 235), ("跡", "あと", 2666), ("痕", "あと", 7039)],
                    ),
                    dictionary(
                        "Jiten",
                        &[("後", "あと", 574), ("跡", "あと", 3259), ("痕", "あと", 8137)],
                    ),
                    dictionary("BCCWJ", &[("後", "", 109), ("跡", "", 3219)]),
                ],
            ),
            (
                "そういうわけだ。",
                "わけ",
                "訳",
                "訳",
                vec![
                    dictionary(
                        "JPDB",
                        &[("訳", "わけ", 1756), ("分け", "わけ", 14563), ("別け", "わけ", 136413)],
                    ),
                    dictionary(
                        "Jiten",
                        &[("訳", "わけ", 1929), ("分け", "わけ", 16847), ("別け", "わけ", 193222)],
                    ),
                    dictionary("BCCWJ", &[("訳", "", 92), ("分け", "", 88853), ("分", "", 1057)]),
                ],
            ),
        ];
        for (text, surface, card, _lexeme, dictionaries) in cases {
            let frequencies = Arc::new(FrequencyManager::from_dictionaries(dictionaries));
            for name in ["CC100", "VN Freq"] {
                if frequencies.get_dictionary_state(name).is_some() {
                    frequencies.set_dictionary_state(name, 0.0, false).unwrap();
                }
            }
            let mut sentences = vec![Sentence {
                id: 0,
                source_id: 0,
                text: text.to_string(),
                segments: Vec::new(),
                timestamp: None,
                comprehension: 0.0,
            }];
            let terms = extract_words(tokenizer.new_worker(), &mut sentences, &frequencies);
            let term = terms
                .into_iter()
                .find(|term| term.surface_form == surface)
                .unwrap_or_else(|| panic!("production extraction lost {surface} in {text}"));

            let known = state(frequencies.clone(), &[(card, surface)]);
            let (possible, established) = known.filter_existing_terms(vec![term.clone()]);
            assert!(established.is_empty(), "{text}: ambiguous readings are not known");
            assert_eq!(possible.len(), 1, "{text}: possible match must remain minable");
            assert_eq!(possible[0].possible_known_match.as_deref(), Some(card), "{text}");
            assert_eq!(possible[0].comprehension, 0.0, "{text}");
            let unknown = state(frequencies, &[]);
            let (unmatched, established) = unknown.filter_existing_terms(vec![term]);
            assert!(established.is_empty());
            assert_eq!(unmatched.len(), 1, "{text}: no matching card must remain minable");
            assert_eq!(unmatched[0].possible_known_match, None, "{text}");
            assert_eq!(unmatched[0].comprehension, 0.0, "{text}");
        }
    }
}

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

    fn classify(
        &self,
        surface: &str,
        surface_reading: &str,
        citation: &str,
        reading: &str,
        pos: &crate::segmentation::word::POS,
        promoted: bool,
    ) -> MatchResult<'_> {
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
        let preference = crate::dictionary::kana_preference::preference(reading, pos);
        if let Some(preference) = &preference {
            if citation.is_kana() && !citation.is_empty() {
                return self
                    .cards_read_as(reading)
                    .find(|card| preference.matches(&card.term))
                    .map_or(MatchResult::Unmatched, |card| MatchResult::Known {
                        card,
                        evidence: MatchEvidence::KanaPreference,
                    });
            }
            if preference.matches(citation) {
                if let Some(card) =
                    self.cards_read_as(reading).find(|card| card.term.as_str().is_kana())
                {
                    return MatchResult::Known { card, evidence: MatchEvidence::KanaPreference };
                }
            }
        }
        use crate::dictionary::lexical_evidence::ExpressionSelection;
        let evidence = self.frequency_manager.lexical_families(reading);
        let selection = evidence.select_expression(surface, citation);
        let cards: Vec<_> = self.cards_read_as(reading).collect();
        let belongs = |family: &crate::dictionary::lexical_evidence::LexicalFamily,
                       card: &Vocab| {
            family
                .spellings
                .iter()
                .any(|s| normalize_japanese_text(s) == normalize_japanese_text(&card.term))
        };
        match selection {
            ExpressionSelection::Selected { family } => {
                if let Some(card) = cards.iter().copied().find(|card| {
                    belongs(family, card) && (preference.is_none() || !card.term.as_str().is_kana())
                }) {
                    return MatchResult::Known { card, evidence: MatchEvidence::LexicalFamily };
                }
                // A kana card must independently resolve to this same expression.
                // Sharing its reading alone does not establish a duplicate.
                for card in &cards {
                    if preference.is_some() || !card.term.as_str().is_kana() {
                        continue;
                    }
                    if let ExpressionSelection::Selected { family: card_family } =
                        evidence.select_expression(&card.term, &card.term)
                    {
                        if card_family == family {
                            return MatchResult::Known {
                                card,
                                evidence: MatchEvidence::LexicalFamily,
                            };
                        }
                    }
                }
                // A card for a rejected interpretation is not a possible duplicate.
            }
            ExpressionSelection::Ambiguous(families) => {
                if let Some(card) =
                    cards.iter().copied().find(|card| families.iter().any(|f| belongs(f, card)))
                {
                    if families.iter().all(|f| cards.iter().any(|card| belongs(f, card))) {
                        return MatchResult::Known { card, evidence: MatchEvidence::LexicalFamily };
                    }
                    return MatchResult::Possible { card };
                }
                return cards
                    .first()
                    .map_or(MatchResult::Unmatched, |card| MatchResult::Possible { card });
            }
            ExpressionSelection::Unresolved => {
                if surface.is_kana() {
                    return cards
                        .first()
                        .map_or(MatchResult::Unmatched, |card| MatchResult::Possible { card });
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

    fn classify_term(&self, term: &Term) -> MatchResult<'_> {
        self.classify(
            &term.surface_form,
            &term.surface_reading,
            &term.lemma_form,
            &term.lemma_reading,
            &term.part_of_speech,
            term.lexical_family.is_some(),
        )
    }

    /// Statistics for an extracted term use the same classification as file filtering.
    pub fn term_stats(&self, term: &Term) -> (bool, f32) {
        match self.classify_term(term) {
            MatchResult::Known { card, .. } => {
                (true, comp_term(card.interval.or(Some(1.0)), self.known_interval))
            }
            _ => (false, 0.0),
        }
    }

    pub fn filter_existing_terms(&self, terms: Vec<Term>) -> (Vec<Term>, Vec<Term>) {
        let classified: Vec<_> = terms
            .into_par_iter()
            .map(|mut term| {
                term.possible_known_match = None;
                term.comprehension = 0.0;
                let known = match self.classify_term(&term) {
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
    fn precomputed_preference_ignores_display_weights_and_clears_stale_labels() {
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![linked_dictionary(
            "linked", 9328, 65,
        )]));
        manager.set_dictionary_state("linked", 0.0, false).unwrap();
        let known = state(manager.clone(), &[("行く", "いく")]);
        let mut input = term("いく", "いく");
        input.part_of_speech = POS::Verb;
        input.possible_known_match = Some("old".into());
        let (unknown, filtered) = known.filter_existing_terms(vec![input.clone()]);
        assert!(unknown.is_empty());
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].possible_known_match.is_none());
        assert!(filtered[0].comprehension > 0.0);
        assert!(known.word_stats("いく", "いく", &POS::Verb).0);
        assert!(matches!(
            known.classify("いく", "いく", "いく", "いく", &POS::Verb, false),
            MatchResult::Known { evidence: MatchEvidence::KanaPreference, .. }
        ));
        let reverse = state(manager.clone(), &[("いく", "いく")]);
        assert!(reverse.word_stats("行く", "いく", &POS::Verb).0);
        assert!(!reverse.word_stats("逝く", "いく", &POS::Verb).0);
        let removed = state(manager.clone(), &[]);
        let (unknown, filtered) = removed.filter_existing_terms(vec![input]);
        assert!(filtered.is_empty());
        assert!(unknown[0].possible_known_match.is_none());
        assert_eq!(unknown[0].comprehension, 0.0);
        // The bundled preference is selected independently of the available cards.
        let rare = state(manager, &[("逝く", "いく")]);
        assert!(!rare.word_stats("いく", "いく", &POS::Verb).0);
    }

    #[test]
    fn dictionary_selection_is_independent_of_cards_and_rejects_other_expressions() {
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![]));
        let preference =
            crate::dictionary::kana_preference::preference("いく", &POS::Verb).unwrap();
        assert!(preference.matches("行く"));
        assert!(!preference.matches("逝く"));
        for cards in [vec![], vec![("逝く", "いく")]] {
            let anki = state(manager.clone(), &cards);
            let mut input = term("いく", "いく");
            input.part_of_speech = POS::Verb;
            input.possible_known_match = Some("逝く".into());
            let (unknown, known) = anki.filter_existing_terms(vec![input]);
            assert!(known.is_empty());
            assert!(unknown[0].possible_known_match.is_none());
            assert_eq!(unknown[0].comprehension, 0.0);
        }
        for cards in
            [vec![("逝く", "いく"), ("行く", "いく")], vec![("行く", "いく"), ("逝く", "いく")]]
        {
            let anki = state(manager.clone(), &cards);
            let MatchResult::Known { card, .. } =
                anki.classify("いく", "いく", "いく", "いく", &POS::Verb, false)
            else {
                panic!("selected expression should match");
            };
            assert_eq!(card.term, "行く");
        }
    }

    #[test]
    fn written_citation_takes_priority_over_preference_for_kana_source() {
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![linked_dictionary(
            "linked", 9328, 65,
        )]));
        let anki = state(manager, &[("行く", "いく")]);
        // The validated citation says 逝く; the common homophone must not replace it.
        assert!(matches!(
            anki.classify("いった", "いった", "逝く", "いく", &POS::Verb, false),
            MatchResult::Unmatched
        ));
    }

    #[test]
    fn installed_iku_entries_match_the_extracted_kana_to_iku_card() {
        // Read-only snapshot of the independent installed dictionaries' いく records.
        // In particular, JPDB repeats 65㋕ under 行く AND 逝く.
        let snapshot: HashMap<String, Vec<(String, String, JsonFrequencyData)>> =
            serde_json::from_str(include_str!("../../tests/fixtures/iku_frequency.json")).unwrap();
        let dictionaries = snapshot
            .into_iter()
            .map(|(name, rows)| {
                FrequencyDictionary::new(
                    name,
                    "fixture".into(),
                    rows.into_iter()
                        .map(|(term, data_type, data)| TermMetaBankV3 {
                            term,
                            data_type,
                            data: Some(data),
                        })
                        .collect(),
                )
            })
            .collect();
        let manager = Arc::new(FrequencyManager::from_dictionaries(dictionaries));
        let Some(tokenizer) = crate::segmentation::lexeme_resolver::test_tokenizer() else {
            return;
        };
        let mut sentences = vec![Sentence {
            id: 0,
            source_id: 0,
            text: "学校にいく。".into(),
            segments: vec![],
            timestamp: None,
            comprehension: 0.0,
        }];
        let terms = extract_words(tokenizer.new_worker(), &mut sentences, &manager);
        let input = terms.into_iter().find(|t| t.surface_form == "いく").expect("extract いく");
        let anki = state(manager.clone(), &[("行く", "いく")]);
        let (unknown, known) = anki.filter_existing_terms(vec![input.clone()]);
        assert!(unknown.is_empty(), "actual dictionary entries must match 行く: {unknown:?}");
        assert_eq!(known.len(), 1);
        assert!(known[0].comprehension > 0.0);
        assert!(known[0].possible_known_match.is_none());
        assert!(anki.word_stats("いく", "いく", &POS::Verb).0);
        let rare = state(manager.clone(), &[("逝く", "いく")]);
        let (unknown, known) = rare.filter_existing_terms(vec![input]);
        assert!(known.is_empty());
        assert!(unknown[0].possible_known_match.is_none());
        assert_eq!(unknown[0].comprehension, 0.0);
        let reverse = state(manager, &[("いく", "いく")]);
        assert!(reverse.word_stats("行く", "いく", &POS::Verb).0);
        assert!(!reverse.word_stats("逝く", "いく", &POS::Verb).0);
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
            &[("未登録語", "みとうろくよみ"), ("何", "なん")],
        );
        assert_eq!(state.word_stats("みとうろくよみ", "みとうろくよみ", &POS::Verb), (false, 0.0));
        assert_eq!(state.word_stats("なん", "なん", &POS::Pronoun), (false, 0.0));
        assert!(!state.word_stats("ミトウロクヨミ", "みとうろくよみ", &POS::Verb).0);
        assert!(state.word_stats("未登録語", "みとうろくよみ", &POS::Verb).0);
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
    fn production_readings_use_dictionary_preferences_and_preserve_unresolved_matches() {
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
            if matches!(surface, "うまい" | "こと" | "いく" | "くる" | "あと") {
                assert!(
                    possible.is_empty(),
                    "{text}: precomputed preference should match: {term:?}"
                );
                assert_eq!(established.len(), 1);
                assert!(established[0].comprehension > 0.0);
                assert!(established[0].possible_known_match.is_none());
                assert!(known.term_stats(&term).0);
            } else {
                assert!(established.is_empty(), "{text}: unresolved readings are not known");
                assert_eq!(possible.len(), 1, "{text}: possible match must remain minable");
                assert_eq!(possible[0].possible_known_match.as_deref(), Some(card), "{text}");
                assert_eq!(possible[0].comprehension, 0.0, "{text}");
            }
            let unknown = state(frequencies, &[]);
            let (unmatched, established) = unknown.filter_existing_terms(vec![term]);
            assert!(established.is_empty());
            assert_eq!(unmatched.len(), 1, "{text}: no matching card must remain minable");
            assert_eq!(unmatched[0].possible_known_match, None, "{text}");
            assert_eq!(unmatched[0].comprehension, 0.0, "{text}");
        }
    }
    #[test]
    fn dictionary_preferences_match_extracted_examples_and_refresh_cards() {
        let Some(tokenizer) = crate::segmentation::lexeme_resolver::test_tokenizer() else {
            return;
        };
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![]));
        for (text, surface, reading, card, wrong_card) in [
            ("大事なことだ。", "こと", "こと", "事", "琴"),
            ("日本語ができる。", "できる", "できる", "出来る", "出切る"),
            ("昨日はできなかった。", "できなかった", "できる", "出来る", "出切る"),
            ("おっきな花火を上げてみせるわ", "みせる", "みせる", "見せる", "診せる"),
            ("まさにその通りだ。", "まさに", "まさに", "正に", "将に"),
            ("大喜び 間違いなし", "なし", "なし", "無し", "梨"),
        ] {
            let mut sentences = vec![Sentence {
                id: 0,
                source_id: 0,
                text: text.into(),
                segments: vec![],
                timestamp: None,
                comprehension: 0.0,
            }];
            let terms = extract_words(tokenizer.new_worker(), &mut sentences, &manager);
            let input = terms
                .into_iter()
                .find(|t| t.surface_form == surface)
                .unwrap_or_else(|| panic!("missing {surface}"));
            let matching = state(manager.clone(), &[(card, reading)]);
            assert!(
                matches!(
                    matching.classify_term(&input),
                    MatchResult::Known { evidence: MatchEvidence::KanaPreference, .. }
                ),
                "{input:?}"
            );
            let (unknown, known) = matching.filter_existing_terms(vec![input.clone()]);
            assert!(unknown.is_empty());
            assert!(matching.term_stats(&input).0);
            assert!(known[0].comprehension > 0.0);
            for cards in [vec![], vec![(wrong_card, reading)], vec![(card, "invalid")]] {
                let changed = state(manager.clone(), &cards);
                let (unknown, known) = changed.filter_existing_terms(known.clone());
                assert!(known.is_empty());
                assert_eq!(unknown[0].comprehension, 0.0);
                assert!(unknown[0].possible_known_match.is_none());
                assert!(!changed.term_stats(&input).0);
                let (unknown, known) = matching.filter_existing_terms(unknown);
                assert!(unknown.is_empty());
                assert!(known[0].possible_known_match.is_none());
            }
        }
    }

    #[test]
    fn reverse_precomputed_matches_work_without_installed_frequency_entries() {
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![]));
        for (reading, written, rival, pos) in [
            ("みせる", "見せる", "診せる", POS::Verb),
            ("なし", "無し", "梨", POS::Noun),
            ("まさに", "正に", "将に", POS::Adverb),
            ("いく", "行く", "逝く", POS::Verb),
        ] {
            let anki = state(manager.clone(), &[(reading, reading)]);
            assert!(anki.word_stats(written, reading, &pos).0);
            assert!(!anki.word_stats(rival, reading, &pos).0);
            // Even a lone installed rival must not override the compiled preference.
            let partial = Arc::new(FrequencyManager::from_dictionaries(vec![dictionary(
                "partial",
                &[(rival, reading, 1), (reading, reading, 2)],
            )]));
            assert!(!state(partial, &[(reading, reading)]).word_stats(rival, reading, &pos).0);
        }
    }

    #[test]
    fn precomputed_matching_needs_no_context_but_respects_readings_and_pos() {
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![]));
        let anki = state(manager, &[("事", "こと"), ("嘴", "はし")]);
        assert!(anki.term_stats(&term("こと", "こと")).0);
        assert!(!anki.term_stats(&term("こと", "じ")).0);
        assert!(!anki.term_stats(&term("はし", "はし")).0);
        assert!(!anki.word_stats("こと", "こと", &POS::Pronoun).0);
    }
    #[test]
    fn mixed_dekiru_conjugations_keep_the_same_dictionary_identity() {
        let Some(tokenizer) = crate::segmentation::lexeme_resolver::test_tokenizer() else {
            return;
        };
        let manager = Arc::new(FrequencyManager::from_dictionaries(vec![]));
        let mut sentences: Vec<_> = [
            "今までに算術ができなくて困ったことはありますか？",
            "できた",
            "よくできていますよ",
            "２人ともできるかな",
        ]
        .into_iter()
        .enumerate()
        .map(|(id, text)| Sentence {
            id,
            source_id: 0,
            text: text.into(),
            segments: vec![],
            timestamp: None,
            comprehension: 0.0,
        })
        .collect();
        let terms = extract_words(tokenizer.new_worker(), &mut sentences, &manager);
        let input = terms.into_iter().find(|t| t.lemma_form == "できる").expect("extract できる");
        assert_eq!(input.sentence_references.len(), 4);
        let anki = state(manager.clone(), &[("出来る", "できる")]);
        assert!(
            anki.term_stats(&input).0,
            "equivalent kana scripts must not clear context: {input:?}"
        );
        let (unknown, known) = anki.filter_existing_terms(vec![input.clone()]);
        assert!(unknown.is_empty());
        assert!(known[0].possible_known_match.is_none());
        assert!(known[0].comprehension > 0.0);
        let wrong = state(manager, &[("出切る", "できる")]);
        assert!(!wrong.term_stats(&input).0);
        let (unknown, known) = wrong.filter_existing_terms(vec![input]);
        assert!(known.is_empty());
        assert!(unknown[0].possible_known_match.is_none());
    }
}

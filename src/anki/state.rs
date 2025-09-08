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
        get_model_ids,
        get_note_ids,
        get_notes,
        get_version,
    },
    scoring::{
        AnkiMatcher,
        KEEP_TERM_THRESHOLD,
    },
    types::{
        FieldMapping,
        Model,
        Vocab,
    },
};
use crate::{
    core::{
        utils::{
            normalize_japanese_text,
            NormalizeLongVowel,
        },
        Term,
    },
    dictionary::frequency_manager::FrequencyManager,
};

pub struct AnkiState {
    vocab: Vec<Vocab>,
    matcher: AnkiMatcher,
    relevance_map: HashMap<String, Vec<usize>>, // Map to indices
}

impl AnkiState {
    pub async fn new(
        model_mapping: HashMap<String, FieldMapping>,
        frequency_manager: Arc<FrequencyManager>,
    ) -> Result<Self, reqwest::Error> {
        let start = Instant::now();
        let vocab = get_total_vocab(&model_mapping).await?;
        let duration = start.elapsed();
        println!("AnkiState::new - get_total_vocab took: {:?}", duration);

        // Build relevance map once during initialization
        let relevance_map = Self::build_relevance_map(&vocab);
        let matcher = AnkiMatcher::new(frequency_manager);

        let result = Ok(Self { vocab, matcher, relevance_map });
        println!("AnkiState::new total time: {:?}", start.elapsed());
        result
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

    fn highest_inclusivity_score(
        &self,
        term: &str,
        reading: &str,
        pos: &crate::segmentation::word::POS,
    ) -> f32 {
        let mut highest_score = 0.0;

        // Use cached relevance map for efficient lookup
        for key in [
            reading,
            term,
            normalize_japanese_text(reading).as_str(),
            normalize_japanese_text(term).as_str(),
        ] {
            if let Some(vocab_indices) = self.relevance_map.get(key) {
                for &index in vocab_indices {
                    let vocab = &self.vocab[index];
                    let score = self.matcher.inclusivity_score(term, reading, vocab, pos);

                    if score == 1.0 {
                        return 1.0; // Early return for perfect matches
                    }
                    if score > highest_score {
                        highest_score = score;
                    }
                }
            }
        }

        highest_score
    }

    pub fn filter_existing_terms(&self, terms: Vec<Term>) -> Vec<Term> {
        let start = Instant::now();

        // Calculate scores for all terms in parallel, maintaining order
        let term_scores: Vec<(Term, f32)> = terms
            .into_par_iter()
            .map(|term| {
                // Check both surface and lemma forms, take the higher score
                let surface_score = self.highest_inclusivity_score(
                    &term.surface_form,
                    &term.surface_reading,
                    &term.part_of_speech,
                );
                let lemma_score = self.highest_inclusivity_score(
                    &term.lemma_form,
                    &term.lemma_reading,
                    &term.part_of_speech,
                );

                let score = f32::max(surface_score, lemma_score);
                (term, score)
            })
            .collect();

        // Filter terms based on Anki inclusivity threshold
        let (filtered_terms, _scores): (Vec<Term>, Vec<f32>) = term_scores
            .into_iter()
            .filter_map(
                |(term, score)| {
                    if score < KEEP_TERM_THRESHOLD {
                        Some((term, score))
                    } else {
                        None
                    }
                },
            )
            .unzip();

        let duration = start.elapsed();
        println!("filter_existing_terms took: {:?}", duration);
        filtered_terms
    }
}

pub async fn get_total_vocab(
    model_mapping: &HashMap<String, FieldMapping>,
) -> Result<Vec<Vocab>, reqwest::Error> {
    let deck_query = "deck:*";
    let note_ids = get_note_ids(&deck_query).await?;

    let notes = get_notes(note_ids).await?;

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
                            reading: reading.normalize_long_vowel().into_owned(),
                        });
                    }
                }
            }
            None
        })
        .collect();

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

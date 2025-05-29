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

use api::{
    get_field_names,
    get_model_ids,
    get_note_ids,
    get_notes,
    get_version,
};
use rayon::iter::{
    IntoParallelIterator,
    ParallelIterator,
};
use tokio::{
    task::{
        self,
    },
    time::sleep,
};
use wana_kana::IsJapaneseStr;

use crate::{
    core::{
        utils::NormalizeLongVowel,
        Term,
    },
    dictionary::frequency_manager::FrequencyManager,
};

pub mod api;

pub struct AnkiState {
    models: Vec<Model>,
    model_mapping: HashMap<String, FieldMapping>,
    vocab: Vec<Vocab>,
    frequency_manager: Arc<FrequencyManager>,
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

        let result = Ok(Self { models: Vec::new(), model_mapping, vocab, frequency_manager });
        println!("AnkiState::new total time: {:?}", start.elapsed());
        result
    }

    fn inclusivity_score(&self, word: &str, word_reading: &str, anki_card: &Vocab) -> f32 {
        let anki_word = &anki_card.term;
        let anki_reading = &anki_card.reading;
        let frequencies = self.frequency_manager.get_frequency_data_by_term(anki_word);

        if anki_word.eq(word) {
            return 1.0;
        }

        if word.is_kana() && anki_word.as_str().is_kana() {
            if word_reading == anki_reading {
                return 1.0;
            }
        }

        if word.is_kana() && !anki_word.as_str().is_kana() {
            if anki_reading.eq(&word_reading) {
                let mut grouped_frequencies: HashMap<String, Vec<f32>> = HashMap::new();
                for freq in frequencies {
                    if let Some(reading) = freq.reading() {
                        grouped_frequencies
                            .entry(reading.to_string())
                            .or_insert_with(Vec::new)
                            .push(freq.value() as f32);
                    }
                }

                let average_frequencies: Vec<(String, f32)> = grouped_frequencies
                    .into_iter()
                    .map(|(reading, values)| {
                        let avg_freq = values.iter().sum::<f32>() / (values.len() as f32);
                        (reading, avg_freq)
                    })
                    .collect();

                let (min_freq, max_freq) = average_frequencies
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(min, max), (_, freq)| {
                        (min.min(*freq), max.max(*freq))
                    });

                if let Some((_, matched_freq)) =
                    average_frequencies.iter().find(|(reading, _)| reading == anki_reading)
                {
                    if max_freq > min_freq {
                        let normalized = (*matched_freq - min_freq) / (max_freq - min_freq);
                        let probability = 1.0 - (0.1 + normalized * 0.8);
                        return probability;
                    } else {
                        return 0.9;
                    }
                }
            }
        }

        0.0
    }

    fn highest_inclusivity_score(&self, term: &str, reading: &str) -> f32 {
        let mut relevance_map: HashMap<String, Vec<&Vocab>> = HashMap::new();

        for vocab in &self.vocab {
            relevance_map.entry(vocab.reading.clone()).or_insert_with(Vec::new).push(vocab);
            relevance_map.entry(vocab.term.clone()).or_insert_with(Vec::new).push(vocab);
        }

        let mut highest_score = 0.0;

        for key in [&reading, term] {
            if let Some(vocab_list) = relevance_map.get(key) {
                for vocab in vocab_list {
                    let score = self.inclusivity_score(term, reading, vocab);
                    if score == 1.0 {
                        return 1.0;
                    }
                    if score > highest_score {
                        highest_score = score;
                    }
                }
            }
        }

        highest_score
    }

    pub fn filter_existing_terms(&self, terms: Vec<Term>, surface_form: bool) -> Vec<Term> {
        let start = Instant::now();
        let filtered_terms: Vec<Term> = terms
            .into_par_iter()
            .filter(|term| {
                let score = match surface_form {
                    true => self.highest_inclusivity_score(
                        &term.surface_form.normalize_long_vowel(),
                        &term.surface_reading.normalize_long_vowel(),
                    ),
                    false => self.highest_inclusivity_score(
                        &term.lemma_form.normalize_long_vowel(),
                        &term.lemma_reading.normalize_long_vowel(),
                    ),
                };
                !(score > 0.75)
            })
            .collect();
        let duration = start.elapsed();
        println!("filter_existing_terms took: {:?}", duration);
        filtered_terms
    }
}

#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub id: u64,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldMapping {
    pub term_field: String,
    pub reading_field: String,
}

#[derive(Debug)]
pub struct Vocab {
    pub term: String,
    pub reading: String,
}

pub async fn get_models() -> Result<Vec<Model>, reqwest::Error> {
    let model_ids = get_model_ids().await?;

    let join_start = Instant::now();
    let handles: Vec<_> = model_ids
        .into_iter()
        .map(|(model_name, id)| {
            task::spawn(async move {
                let fields = get_field_names(&model_name).await?;
                Ok::<Model, reqwest::Error>(Model { name: model_name, id, fields })
            })
        })
        .collect();

    let models: Vec<Model> = futures::future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|result| result.ok())
        .filter_map(|inner_result| inner_result.ok())
        .collect();
    Ok(models)
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
                    if let (Some(term), Some(reading)) = (term, reading) {
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

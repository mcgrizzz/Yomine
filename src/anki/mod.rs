use std::{collections::{HashMap, HashSet}, sync::Arc, time::Duration};

use api::{get_deck_ids, get_field_names, get_model_ids, get_note_ids, get_notes, get_version, Note};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use regex::Replacer;
use tokio::{task::{self}, time::sleep};
use wana_kana::{ConvertJapanese, IsJapaneseStr};

use crate::{core::{utils::SwapLongVowel, Term}, frequency_dict::FrequencyManager};

pub mod api;

pub struct AnkiState {
    models: Vec<Model>,
    model_mapping: HashMap<String, FieldMapping>, //<ModelName, FieldMapping>
    vocab: Vec<Vocab>,
    frequency_manager: Arc<FrequencyManager>,
}

impl AnkiState {

    pub async fn new(model_mapping: HashMap<String, FieldMapping>, frequency_manager: Arc<FrequencyManager>) -> Result<Self, reqwest::Error> {
        let models = get_models().await?;
        let vocab = get_total_vocab(&model_mapping).await?;

        Ok(Self {
            models,
            model_mapping,
            vocab,
            frequency_manager,
        })
    }

    fn inclusivity_score(&self, word: &str, word_reading: &str, anki_card: &Vocab) -> f32 {
        let hiragana_reading = word_reading.to_hiragana(); 
        let alternate_reading = hiragana_reading.swap_long_vowel();

        let anki_word = &anki_card.term;
        let anki_reading = anki_card.reading.to_hiragana();
        let frequencies = self.frequency_manager.get_kanji_frequency(anki_word);

        if anki_word.eq(word) {
            if (anki_reading == hiragana_reading) || (anki_reading == alternate_reading) {
                return 1.0;
            }

            //We cannot trust tokenizer readings as ground truth readings but... For for now we will say this counts
            return 1.0
        }

        //This is the case if (いただく, いただく) is matched against (いただく, いただく)... we have to assume they're the same
        //There's no way for us to gain confidence otherwise
        if anki_word.as_str().is_kana() && word.is_kana() {
            let alternate_term = word.to_hiragana().swap_long_vowel();
            if alternate_term == anki_word.to_hiragana() || word.to_hiragana() == anki_word.to_hiragana() { 
                return 1.0
            }
        }

        //This is potentially the same word: For example in Anki (頂く, いただく) but in the tokenizer you get (いただく, いただく)
        if word.is_kana() && !anki_word.as_str().is_kana(){
            if anki_reading.eq(&hiragana_reading) || anki_reading.eq(&alternate_reading) {
                //Reading is the same, how do we quantify how likely the words are the same. 
                //Get the most likely kana reading value that has a kanji associated with it. Check to see its the same kanji as we have. 
                let mut grouped_frequencies: HashMap<String, Vec<f32>> = HashMap::new();
                for freq in frequencies {
                    if let Some(reading) = freq.reading() {
                        grouped_frequencies
                        .entry(reading.to_string()) // Group by reading
                        .or_insert_with(Vec::new)
                        .push(freq.value() as f32);
                    }
                    
                }
                
                //Frequencies averaged by reading...
                let average_frequencies: Vec<(String, f32)> = grouped_frequencies
                .into_iter()
                .map(|(reading, values)| {
                    let avg_freq = values.iter().sum::<f32>() / values.len() as f32;
                    (reading, avg_freq)
                })
                .collect();
                
                //Get the min and max freqeuncy 
                let (min_freq, max_freq) = average_frequencies
                .iter()
                .fold((f32::MAX, f32::MIN), |(min, max), (_, freq)| {
                    (min.min(*freq), max.max(*freq))
                });
               
                //If there is a frequency for this reading...
                if let Some((_, matched_freq)) = average_frequencies
                    .iter()
                    .find(|(reading, _)| reading == &anki_reading)
                {  
                    if max_freq > min_freq {
                        let normalized = (*matched_freq - min_freq) / (max_freq - min_freq); //scale frequency into the range between min and max
                        let probability = 1.0 - (0.1 + (normalized * 0.8)); 
                        return probability;
                    } else { //In this case they're equal and there is only one reading
                        return 0.9; 
                    }
                }

            }
        }

        return 0.0;
    }

    // How likely is it that the given term and reading is already in our anki decks.
    // Exact matches will give a score of 1.0, anything less than exact will have some degree of uncertainty
    fn highest_inclusivity_score(&self, term: &str, reading: &str) -> f32 {
        let normalized_reading = reading.to_hiragana();
        let alternate_reading = normalized_reading.swap_long_vowel();

        let mut relevance_map: HashMap<String, Vec<&Vocab>> = HashMap::new();

        for vocab in &self.vocab {
            relevance_map.entry(vocab.reading.to_hiragana())
                .or_insert_with(Vec::new)
                .push(vocab);
            
            relevance_map.entry(vocab.term.clone())
            .or_insert_with(Vec::new)
            .push(vocab);
        }

        let mut highest_score = 0.0;

        for key in [&normalized_reading, &alternate_reading, term] {
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
        let filtered_terms: Vec<Term> = terms
        .into_par_iter()
        .filter(|term| {

            let score = match surface_form {
                true => {
                    self.highest_inclusivity_score(&term.surface_form, &term.get_surface_reading())
                },
                false => {
                    self.highest_inclusivity_score(&term.lemma_form, &term.lemma_reading)
                }
            };

            !(score > 0.75)
           

        })
        .collect();

        filtered_terms
    }
 
}

#[derive(Debug)]
pub struct Model {
    name: String,
    id: u64,
    fields: Vec<String>,
}

#[derive(Debug)]
pub struct FieldMapping {
    pub term_field: String, //Expression
    pub reading_field: String, //ExpressionReading
}

#[derive(Debug)]
pub struct Vocab {
    pub term: String,
    pub reading: String,
}


pub async fn get_models() -> Result<Vec<Model>, reqwest::Error> {
    let model_ids = get_model_ids().await?;

    let handles: Vec<_> = model_ids
        .into_iter()
        .map(|(model_name, id)| {
            task::spawn(async move {
                let fields = get_field_names(&model_name).await?;
                Ok::<Model, reqwest::Error>(Model {
                    name: model_name,
                    id,
                    fields,
                })
            })
        })
        .collect();

    let models: Vec<Model> = futures::future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|result| result.ok()) // Task errors
        .filter_map(|inner_result| inner_result.ok()) // API errors
        .collect();

    Ok(models)
}


pub async fn get_total_vocab(model_mapping: &HashMap<String, FieldMapping>) -> Result<Vec<Vocab>, reqwest::Error> {        
    let decks = get_deck_ids().await?;
    let deck_names: Vec<String> = decks
        .into_iter()
        .map(|deck| {
            let deck_name = deck.name;
            format!("deck:\"{deck_name}\"")
        })
        .collect();

    let deck_query = deck_names.join(" OR ");

    let note_ids = get_note_ids(&deck_query).await?;
    let notes = get_notes(note_ids).await?;
    
    let vocab: Vec<Vocab> = notes
        .into_iter()
        .filter_map(|note| {
            if let Some(field_mapping) = model_mapping.get(&note.model_name) {

                let term = note.fields.get(&field_mapping.term_field).map(|f| f.value.clone());
                let reading = note.fields.get(&field_mapping.reading_field).map(|f| f.value.clone());

                if let (Some(term), Some(reading)) = (term, reading) {
                    return Some(Vocab { term, reading });
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

    println!("AnkiConnect did not respond after {} attempts.", max_attempts);
    Ok(false)
}
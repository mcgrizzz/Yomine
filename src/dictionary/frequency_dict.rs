use std::collections::HashMap;
use serde::Deserialize;
use wana_kana::{ConvertJapanese, IsJapaneseStr};

use super::{CacheFrequencyData, FrequencyData, TermMetaBankV3};


#[derive(serde::Serialize, Deserialize, Clone)]
pub struct FrequencyDictionary {
    pub title: String,
    pub revision: String,
    pub terms: HashMap<String, Vec<CacheFrequencyData>>, // Map term -> multiple frequency entries
}

impl FrequencyDictionary {

    pub fn new(title: String, revision: String, term_meta_list: Vec<TermMetaBankV3>) -> Self {
        let mut terms = HashMap::new();
    
        for term_meta in term_meta_list {
            if let Some(json_freq_data) = term_meta.data {
                let cache_freq_data: CacheFrequencyData = json_freq_data.into(); 
                terms
                    .entry(term_meta.term.clone())
                    .or_insert_with(Vec::new)
                    .push(cache_freq_data);
            }
        }
    
        FrequencyDictionary {
            title,
            revision,
            terms,
        }
    }
    
    
    //If dictionary form is in kana
    pub fn get_frequency(&self, lemma_form: &str, lemma_reading: &str, is_kana: bool) -> Option<&FrequencyData> {
        if is_kana {
            self.get_kana_frequency(lemma_form, lemma_reading)
                .or_else(|| self.get_normal_frequency(lemma_form, lemma_reading))
        } else {
            self.get_normal_frequency(lemma_form, lemma_reading)
        }
    }

    fn get_kana_frequency(&self, lemma_form: &str, lemma_reading: &str) -> Option<&FrequencyData> {
        self.terms.get(lemma_form).and_then(|entries| {

            let matching_entry = entries.iter().find(|entry| {
                if let Some(entry_reading) = entry.reading().as_deref() {
                    let normalized_reading = if entry_reading.is_hiragana() {
                        lemma_reading.to_hiragana()
                    } else {
                        lemma_reading.to_katakana()
                    };
                    entry.has_special_marker() && normalized_reading.eq(entry_reading)
                } else {
                    false
                }
            });
            
            matching_entry.or_else(|| entries.iter().find(|entry| entry.reading().is_none()))
        })
    }
    
    fn get_normal_frequency(&self, lemma_form: &str, lemma_reading: &str) -> Option<&FrequencyData> {
        self.terms.get(lemma_form).and_then(|entries| {

            let matching_entries: Vec<_> = entries
                .iter()
                .filter(|entry| {
                    if let Some(entry_reading) = entry.reading().as_deref() {
                        let normalized_reading = if entry_reading.is_hiragana() {
                            lemma_reading.to_hiragana()
                        } else {
                            lemma_reading.to_katakana()
                        };
                        !entry.has_special_marker() && normalized_reading.eq(entry_reading)
                    } else {
                        false
                    }
                })
                .collect();
    
            
            if !matching_entries.is_empty() {
                matching_entries
                    .into_iter()
                    .min_by(|a, b| a.value().partial_cmp(&b.value()).unwrap())
            } else {
                
                entries.iter().find(|entry| entry.reading().is_none())
            }
        })
    }

    //Grab all the matching frequencies by key (just directly look up the key we want)
    pub fn get_frequencies_by_key(&self, key: &str) -> Option<&Vec<FrequencyData>> {
        self.terms.get(key)
    }
}
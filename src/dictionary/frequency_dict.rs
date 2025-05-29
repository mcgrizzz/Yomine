use std::collections::HashMap;

use serde::Deserialize;

use super::{
    CacheFrequencyData,
    FrequencyData,
    TermMetaBankV3,
};
use crate::core::utils::NormalizeLongVowel;

#[derive(serde::Serialize, Deserialize, Clone, Debug)]
pub struct FrequencyDictionary {
    pub title: String,
    pub revision: String,
    pub terms: HashMap<String, Vec<CacheFrequencyData>>, // Map term -> multiple frequency entries
}

impl FrequencyDictionary {
    pub fn new(title: String, revision: String, term_meta_list: Vec<TermMetaBankV3>) -> Self {
        let mut terms = HashMap::new();

        //Do reading normalization here so we don't ever have to compute again.
        for term_meta in term_meta_list {
            if let Some(json_freq_data) = term_meta.data {
                let mut cache_freq_data: CacheFrequencyData = json_freq_data.into();

                // Normalize the long vowel of the reading if it exists
                if let Some(reading) = cache_freq_data.reading() {
                    let normalized_reading = reading.normalize_long_vowel().into_owned();
                    cache_freq_data.set_reading(normalized_reading);
                }

                // Normalize the long vowel of the term/key
                let normalized_term = term_meta.term.normalize_long_vowel().into_owned();

                terms.entry(normalized_term).or_insert_with(Vec::new).push(cache_freq_data);
            }
        }

        FrequencyDictionary { title, revision, terms }
    }

    //If dictionary form is in kana
    pub fn get_frequency(
        &self,
        lemma_form: &str,
        lemma_reading: &str,
        is_kana: bool,
    ) -> Option<&FrequencyData> {
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
                    entry.has_special_marker() && lemma_reading.eq(entry_reading)
                } else {
                    false
                }
            });

            matching_entry.or_else(|| entries.iter().find(|entry| entry.reading().is_none()))
        })
    }

    fn get_normal_frequency(
        &self,
        lemma_form: &str,
        lemma_reading: &str,
    ) -> Option<&FrequencyData> {
        self.terms.get(lemma_form).and_then(|entries| {
            let matching_entries: Vec<_> = entries
                .iter()
                .filter(|entry| {
                    if let Some(entry_reading) = entry.reading().as_deref() {
                        !entry.has_special_marker() && lemma_reading.eq(entry_reading)
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

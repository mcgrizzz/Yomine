use std::fs::File;
use std::io;
use std::sync::Mutex;
use std::{collections::HashMap, fs, path::Path};

use rayon::iter::{ParallelBridge, ParallelIterator};
use regex::Regex;
use serde::Deserialize;
use wana_kana::IsJapaneseStr;
use wana_kana::ConvertJapanese;

use crate::core::YomineError;

pub struct FrequencyManager {
    dictionaries: HashMap<String, FrequencyDictionary>, 
    toggled_states: HashMap<String, bool>,              
}

impl FrequencyManager {

    fn new() -> Self {
        FrequencyManager {
            dictionaries: HashMap::new(),
            toggled_states: HashMap::new(),
        }
    }

    fn add_dictionary(&mut self, name: String, dictionary: FrequencyDictionary) {
        self.dictionaries.insert(name.clone(), dictionary);
        self.toggled_states.insert(name, true);
    }

    fn toggle_dictionary(&mut self, name: &str, enabled: bool) -> Result<(), YomineError> {
        if let Some(state) = self.toggled_states.get_mut(name) {
            *state = enabled;
            Ok(())
        } else {
            Err(YomineError::Custom(format!("Dictionary '{}' not found", name)))
        }
    }

    fn get_enabled_dictionaries(&self) -> Vec<&FrequencyDictionary> {
        self.toggled_states
            .iter()
            .filter_map(|(name, &enabled)| {
                if enabled {
                    self.dictionaries.get(name)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn build_freq_map(&self, lemma_form: &str, lemma_reading: &str, is_kana: bool) -> HashMap<String, u32> {
        let mut freq_map: HashMap<String, u32> = self.get_enabled_dictionaries().iter()
                .filter_map(|dict| {
                    let freq = dict.get_frequency(&lemma_form, &lemma_reading, is_kana);
                    if let Some(term_freq) = freq {
                        Some((dict.title.clone(), term_freq.value())) 
                    } else {
                        None 
                    }
                })
                .collect();

        freq_map.insert("HARMONIC".to_string(), self.harmonic_frequency(lemma_form, lemma_reading, is_kana));
        freq_map
    }

    pub fn harmonic_frequency(&self, lemma_form: &str, lemma_reading: &str, is_kana: bool) -> u32 {
        let mut sum_of_reciprocals = 0.0;
        let mut count = 0;

        for (dict_name, dictionary) in &self.dictionaries {
            if *self.toggled_states.get(dict_name).unwrap_or(&false) {
                if let Some(freq_data) = dictionary.get_frequency(lemma_form, lemma_reading, is_kana) {
                    let frequency = freq_data.value();
                    if frequency > 0 {
                        sum_of_reciprocals += 1.0 / frequency as f64;
                        count += 1;
                    }
                }
            }
        }

        if count > 0 {
            (count as f64 / sum_of_reciprocals).round() as u32
        } else {
            u32::MAX
        }
    }

    pub fn get_kanji_frequency(&self, kanji: &str) -> Vec<&FrequencyData> {
        let mut freqs = Vec::new();
        for (dict_name, dictionary) in &self.dictionaries {
            if *self.toggled_states.get(dict_name).unwrap_or(&false) {
                if let Some(freq_data) = dictionary.get_frequencies_by_kanji(kanji) {
                    freqs.extend(freq_data);
                }
            }
        }

        freqs
    }
}


//https://github.com/yomidevs/yomitan/tree/master/ext/data/schemas
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")] // Match the JSON naming convention
enum FrequencyMode {
    OccurrenceBased,
    RankBased,
}

#[derive(Deserialize, Debug)]
struct DictionaryIndex {
    title: String,
    revision: String,

    format: Option<u8>, //Must have one 
    version: Option<u8>
}

#[derive(Deserialize, Debug)]
struct TermMetaBankV3 {
    term: String, // The text for the term
    #[serde(rename = "type")]
    data_type: String, // "freq", "pitch", or "ipa"
    data: Option<FrequencyData>, // Only populated for "freq" types
}


#[derive(serde::Serialize, Deserialize)]
pub struct FrequencyDictionary {
    pub title: String,
    revision: String,
    terms: HashMap<String, Vec<FrequencyData>>, // Map term -> multiple frequency entries
}

#[derive(serde::Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Frequency {
    Number(u32),
    Complex {
        value: u32,

        #[serde(rename = "displayValue")]
        display_value: Option<String>,
    }
}

#[derive(serde::Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum FrequencyData {
    Simple(Frequency),
    Nested {
        reading: String,
        frequency: Frequency,
    }
}

impl Frequency {
    pub fn value(&self) -> u32 {
        match self {
            Frequency::Number(num) => *num,
            Frequency::Complex {value, .. } => *value,
        }
    }

    pub fn display_value(&self) -> Option<&str> {
        match self {
            Frequency::Number(_) => None,
            Frequency::Complex { display_value, .. } => display_value.as_deref(),
        }
    }
}


impl FrequencyData {

    pub fn value(&self) -> u32 {
        match self {
            FrequencyData::Simple(simple) => simple.value(),
            FrequencyData::Nested { frequency, .. } => frequency.value(),
        }
    }

    pub fn display_value(&self) -> Option<&str> {
        match self {
            FrequencyData::Simple(simple) => simple.display_value(),
            FrequencyData::Nested { frequency, .. } => frequency.display_value(),
        }
    }

    pub fn reading(&self) -> Option<&str> {
        match self {
            FrequencyData::Nested { reading, .. } => Some(reading.as_str()),
            FrequencyData::Simple(_) => None,
        }
    }

    pub fn has_special_marker(&self) -> bool {
        self.display_value()
            .map_or(false, |value| value.contains('㋕'))
    }
}

impl FrequencyDictionary {

    fn new(title: String, revision: String, term_meta_list: Vec<TermMetaBankV3>) -> Self {
        let mut terms = HashMap::new();
    
        for term_meta in term_meta_list {
            if let Some(freq_data) = term_meta.data {
                terms
                    .entry(term_meta.term.clone())
                    .or_insert_with(Vec::new)
                    .push(freq_data);
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

    //Grab all the matching frequencies by kanji
    fn get_frequencies_by_kanji(&self, kana: &str) -> Option<&Vec<FrequencyData>> {
        self.terms.get(kana)
    }
}

fn parse_index_json(folder_path: &Path) -> Result<Option<DictionaryIndex>, YomineError> {
    let index_path = folder_path.join("index.json");
    let index_data = fs::read_to_string(index_path)?;
    let index: DictionaryIndex = serde_json::from_str(&index_data)?;

    let version = index
        .format
        .or(index.version)
        .ok_or(YomineError::MissingVersion)?;

    if version == 3 {
        Ok(Some(index))
    } else {
        Ok(None)
    }
}

fn parse_term_meta_bank(folder_path: &Path) -> Result<Vec<TermMetaBankV3>, YomineError> {
    let re = Regex::new(r"^term_meta_bank_\d+\.json$")?;
    let term_meta_list: Vec<TermMetaBankV3> = fs::read_dir(folder_path)?
        .filter_map(|entry| entry.ok()) // Ensure valid entries
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |name| re.is_match(name))
        })
        .flat_map(|path| {
            fs::read_to_string(&path)
                .ok() // Read file contents
                .and_then(|data| serde_json::from_str::<Vec<Vec<serde_json::Value>>>(&data).ok()) // Parse JSON as nested arrays
                .into_iter() // Handle Option
                .flat_map(|nested_array| nested_array.into_iter()) // Flatten nested arrays
                .filter_map(|raw_entry| serde_json::from_value::<TermMetaBankV3>(serde_json::Value::Array(raw_entry)).ok()) // Convert to TermMetaBankV3
                .filter(|meta| meta.data_type == "freq") // Filter by data type
        })
        .collect();

    println!("Parsed {} entries from term meta bank files.", term_meta_list.len());

    Ok(term_meta_list)
}

pub fn process_frequency_dictionaries() -> Result<FrequencyManager, YomineError> {

    let manager = Mutex::new(FrequencyManager::new());

    fs::read_dir("frequency_dict")?
        .filter_map(|e| e.ok())
        .par_bridge()
        .for_each(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return;
            }

            // Skip unknown index types
            if let Ok(Some(index)) = parse_index_json(&path) {
                if let Ok(term_meta_list) = parse_term_meta_bank(&path) {
                    let freq_dict = FrequencyDictionary::new(index.title.clone(), index.revision, term_meta_list);

                    let mut manager_guard = manager.lock().unwrap();
                    manager_guard.add_dictionary(index.title, freq_dict);

                }else{
                    println!("Failed to parse term meta bank at {:?}", path);
                }
            }else{
                println!("Skipping {:?} due to unsupported format version.", path);
            }
        });
        
    
    Ok(manager.into_inner().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_frequency(dict: &FrequencyDictionary, term: &str, reading: &str, is_kana: bool, expected: Option<u32>) {
        let result = dict.get_frequency(term, reading, is_kana).map(|f| f.value());
        assert_eq!(result, expected, "Failed for term: {}, reading: {}, is_kana: {}", term, reading, is_kana);
    }

    impl FrequencyManager {
        fn get_dictionary(&self, name: &str) -> Option<&FrequencyDictionary> {
            self.toggled_states
                .get(name)
                .and_then(|&enabled| if enabled { self.dictionaries.get(name) } else { None })
        }
    }

    #[test]
    fn test_frequency() {
        let frequency_manager = process_frequency_dictionaries().expect("Failed to load dictionaries");

        let dict = frequency_manager.get_dictionary("JPDBv2㋕").expect("JPDBv2㋕ dictionary not found");
           
        assert_frequency(&dict, "の", "の", true, Some(1));

        // Test kana frequency vs normal frequency
        assert_frequency(&dict, "溜まる", "タマル", true, Some(2701));
        assert_frequency(&dict, "溜まる", "タマル", false, Some(3885));

        // Test reading specificity
        assert_frequency(&dict, "市", "シ", false, Some(2466));
        assert_frequency(&dict, "市", "イチ", false, Some(17201));
    }
}
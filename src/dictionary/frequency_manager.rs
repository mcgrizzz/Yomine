use std::{
    collections::HashMap,
    fs::{
        self,
        File,
    },
    io::{
        BufReader,
        Read,
        Write,
    },
    path::Path,
    sync::Mutex,
    time::Instant,
    u32,
};

use rayon::iter::{
    ParallelBridge,
    ParallelIterator,
};
use regex::Regex;
use wana_kana::IsJapaneseStr;
use zip::ZipArchive;

use super::{
    frequency_dict::FrequencyDictionary,
    DictionaryIndex,
    FrequencyData,
};
use crate::{
    core::{
        utils::harmonic_frequency,
        YomineError,
    },
    dictionary::TermMetaBankV3,
    persistence::get_app_data_dir,
};

pub fn get_frequency_dict_dir() -> std::path::PathBuf {
    get_app_data_dir().join("dictionaries").join("frequency")
}

#[derive(Debug)]
pub struct DictionaryState {
    pub weight: f32,
    pub enabled: bool,
}

#[derive(Debug)]
pub struct FrequencyManager {
    dictionaries: HashMap<String, FrequencyDictionary>,
    states: HashMap<String, DictionaryState>,
}

impl FrequencyManager {
    fn new(states: Option<HashMap<String, DictionaryState>>) -> Self {
        let dict_states: HashMap<String, DictionaryState> = states.unwrap_or_default();
        FrequencyManager { dictionaries: HashMap::new(), states: dict_states }
    }

    fn add_dictionary(&mut self, name: String, dictionary: FrequencyDictionary) {
        self.dictionaries.insert(name.clone(), dictionary);

        if !self.states.contains_key(&name) {
            self.states.insert(name, DictionaryState { weight: 1.0, enabled: true });
        }
    }

    fn get_enabled_dictionaries(&self) -> Vec<&FrequencyDictionary> {
        self.dictionaries
            .iter()
            .filter_map(|(name, dictionary)| {
                self.states
                    .get(name)
                    .and_then(|state| (state.enabled && state.weight > 0.0).then_some(dictionary))
            })
            .collect()
    }

    pub fn build_freq_map(
        &self,
        lemma_form: &str,
        lemma_reading: &str,
        is_kana: bool,
    ) -> HashMap<String, u32> {
        let mut freq_map: HashMap<String, u32> = self
            .dictionaries
            .iter()
            .filter_map(|(_, dict)| {
                let freq = dict.get_frequency(&lemma_form, &lemma_reading, is_kana);
                if let Some(term_freq) = freq {
                    Some((dict.title.clone(), term_freq.value()))
                } else {
                    None
                }
            })
            .collect();

        freq_map.insert("HARMONIC".to_string(), self.get_weighted_harmonic(&freq_map));
        freq_map
    }

    //Return a boolean so we know whether to update the UI or not.
    pub fn set_dictionary_state(
        &mut self,
        name: &str,
        weight: f32,
        enabled: bool,
    ) -> Result<bool, YomineError> {
        if let Some(state) = self.states.get_mut(name) {
            if state.weight != weight || state.enabled != enabled {
                *state = DictionaryState { weight, enabled };
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err(YomineError::Custom(format!("Dictionary '{}' not found", name)))
        }
    }

    pub fn get_weighted_harmonic(&self, freq_map: &HashMap<String, u32>) -> u32 {
        let weighted_freqs: Vec<u32> = freq_map
            .iter()
            .filter_map(|(name, &freq)| {
                self.states.get(name).and_then(|state| {
                    if state.enabled && freq > 0 && state.weight > 0.0 {
                        let weighted_freq = (freq as f32 / state.weight).round() as u32;
                        Some(weighted_freq.max(1))
                    } else {
                        None
                    }
                })
            })
            .collect();

        harmonic_frequency(&weighted_freqs).unwrap_or(u32::MAX)
    }

    pub fn get_dictionary_state(&self, name: &str) -> Option<&DictionaryState> {
        self.states.get(name)
    }

    pub fn get_dictionary_names(&self) -> Vec<String> {
        self.dictionaries.keys().cloned().collect()
    }

    /// Retrieves all frequency data entries for the exact term from enabled dictionaries,
    /// without requiring a reading. For non-kana terms, excludes kana-specific frequencies.
    // Used for anki term filtering, not affected by weighting or toggling dictionaries.
    pub fn get_frequency_data_by_term(&self, input: &str) -> Vec<&FrequencyData> {
        let mut freqs = Vec::new();
        for (_, dictionary) in &self.dictionaries {
            if let Some(freq_data) = dictionary.get_frequencies_by_key(input) {
                if !input.is_kana() {
                    //Filter the kana specific frequencies
                    freqs.extend(
                        freq_data
                            .iter()
                            .filter(|f| !f.has_special_marker())
                            .collect::<Vec<&FrequencyData>>(),
                    );
                } else {
                    //Don't filter otherwise since we matched on a kana key...
                    freqs.extend(freq_data);
                }
            }
        }

        freqs
    }

    //Used for deinflection sorting, not affected by weighting or toggling dictionaries.
    pub fn get_harmonic_frequency_for_pair(&self, word: &str, reading: &str) -> Option<u32> {
        let is_kana = word.is_kana();

        // Helper closure to get frequency for exact word/reading pair from a dictionary
        let get_exact_freq = |d: &FrequencyDictionary| -> Option<u32> {
            if let Some(entries) = d.get_frequencies_by_key(word) {
                let matching_entries: Vec<&FrequencyData> = entries
                    .iter()
                    .filter(|e| {
                        if let FrequencyData::Nested { reading: entry_reading, .. } = e {
                            let marker_condition = if is_kana {
                                e.has_special_marker()
                            } else {
                                !e.has_special_marker()
                            };
                            marker_condition && entry_reading.as_str() == reading
                        } else {
                            false
                        }
                    })
                    .collect();
                if !matching_entries.is_empty() {
                    // Select the smallest frequency among matching entries
                    matching_entries.into_iter().map(|e| e.value()).min()
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Helper closure to check if a dictionary has Nested entries with different readings
        let has_other = |d: &FrequencyDictionary| -> bool {
            if let Some(entries) = d.get_frequencies_by_key(word) {
                entries.iter().any(|e| {
                    if let FrequencyData::Nested { reading: entry_reading, .. } = e {
                        let marker_condition =
                            if is_kana { e.has_special_marker() } else { !e.has_special_marker() };
                        marker_condition && reading != entry_reading
                    } else {
                        false
                    }
                })
            } else {
                false
            }
        };

        // Helper closure to get Simple frequency from a dictionary
        let get_simple = |d: &FrequencyDictionary| -> Option<u32> {
            if let Some(entries) = d.get_frequencies_by_key(word) {
                entries.iter().find(|e| matches!(e, FrequencyData::Simple(_))).map(|e| e.value())
            } else {
                None
            }
        };

        let enabled_dicts = self.get_enabled_dictionaries();

        // Collect frequencies for exact word/reading pairs
        let exact_freqs: Vec<u32> =
            enabled_dicts.iter().filter_map(|d| get_exact_freq(d)).collect();

        if !exact_freqs.is_empty() {
            // Case 1: Exact matches found, calculate harmonic mean
            harmonic_frequency(&exact_freqs)
        } else if enabled_dicts.iter().any(|d| has_other(d)) {
            // Case 2: No exact match, but other readings exist, return None
            None
        } else {
            // Case 3 & 4: No Nested frequencies, use Simple frequencies or return None
            let simple_freqs: Vec<u32> =
                enabled_dicts.iter().filter_map(|d| get_simple(d)).collect();
            harmonic_frequency(&simple_freqs)
        }
    }
}

fn parse_index_json(folder_path: &Path) -> Result<Option<DictionaryIndex>, YomineError> {
    let index_path = folder_path.join("index.json");
    let index_data = fs::read_to_string(index_path)?;
    let index: DictionaryIndex = serde_json::from_str(&index_data)?;

    let version = index.format.or(index.version).ok_or(YomineError::MissingVersion)?;

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
            path.file_name().and_then(|n| n.to_str()).map_or(false, |name| re.is_match(name))
        })
        .flat_map(|path| {
            fs::read_to_string(&path)
                .ok() // Read file contents
                .and_then(|data| serde_json::from_str::<Vec<Vec<serde_json::Value>>>(&data).ok()) // Parse JSON as nested arrays
                .into_iter() // Handle Option
                .flat_map(|nested_array| nested_array.into_iter()) // Flatten nested arrays
                .filter_map(|raw_entry| {
                    serde_json::from_value::<TermMetaBankV3>(serde_json::Value::Array(raw_entry))
                        .ok()
                }) // Convert to TermMetaBankV3
                .filter(|meta| meta.data_type == "freq") // Filter by data type
        })
        .collect();

    println!("Parsed {} entries from term meta bank files.", term_meta_list.len());

    Ok(term_meta_list)
}

fn load_cached_dict(cache_path: &Path) -> Result<FrequencyDictionary, String> {
    let file = File::open(cache_path)
        .map_err(|e| format!("Failed to open cache file at {:?}: {}", cache_path, e))?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).map_err(|e| format!("Failed to read cache file: {}", e))?;

    // Replace bincode::deserialize with bincode::serde::decode_from_slice
    let (dict, _): (FrequencyDictionary, usize) =
        bincode::serde::decode_from_slice(&buffer, bincode::config::standard())
            .map_err(|e| format!("Failed to decode cache: {}", e))?;

    Ok(dict)
}

fn save_cached_dict(dict: &FrequencyDictionary, cache_path: &Path) -> Result<(), String> {
    // Replace bincode::serialize with bincode::serde::encode_to_vec
    let encoded = bincode::serde::encode_to_vec(dict, bincode::config::standard())
        .map_err(|e| format!("Failed to encode dictionary: {}", e))?;

    let mut file = File::create(cache_path)
        .map_err(|e| format!("Failed to create cache file at {:?}: {}", cache_path, e))?;
    file.write_all(&encoded).map_err(|e| format!("Failed to write cache file: {}", e))?;
    Ok(())
}

fn extract_zip(zip_path: &Path, extract_to: &Path) -> Result<(), YomineError> {
    let file = File::open(zip_path)
        .map_err(|e| YomineError::Custom(format!("Failed to open zip file: {}", e)))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| YomineError::Custom(format!("Failed to read zip archive: {}", e)))?;
    archive
        .extract(extract_to)
        .map_err(|e| YomineError::Custom(format!("Failed to extract zip: {}", e)))?;

    Ok(())
}

pub fn process_frequency_dictionaries() -> Result<FrequencyManager, YomineError> {
    let manager = Mutex::new(FrequencyManager::new(None));
    let start = Instant::now();

    println!("Loading frequency dictionaries...");
    let dir_path = get_frequency_dict_dir();
    fs::create_dir_all(&dir_path)?;
    for entry in fs::read_dir(&dir_path)
        .map_err(|e| YomineError::Custom(format!("Failed to read directory: {}", e)))?
    {
        let entry =
            entry.map_err(|e| YomineError::Custom(format!("Failed to read entry: {}", e)))?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("zip") {
            let extract_dir =
                dir_path.join(path.file_stem().unwrap_or_default().to_string_lossy().as_ref());
            if !extract_dir.exists() {
                fs::create_dir_all(&extract_dir).map_err(|e| {
                    YomineError::Custom(format!("Failed to create extraction directory: {}", e))
                })?;
                extract_zip(&path, &extract_dir)?;
            }
        }
    }

    fs::read_dir(&dir_path)?.filter_map(|e| e.ok()).par_bridge().for_each(|entry| {
        let path = entry.path();
        if !path.is_dir() {
            return;
        }

        let cache_path = path.join("cache.bin");

        // Parse index.json to get metadata
        if let Ok(Some(index)) = parse_index_json(&path) {
            let dict_name = index.title.clone();

            // Try loading from cache
            let load_start = Instant::now();
            match load_cached_dict(&cache_path) {
                Ok(cached_dict) => {
                    if cached_dict.revision == index.revision {
                        let duration = load_start.elapsed();
                        println!(
                            "Loaded '{}' from cache in {:?}: {} entries",
                            dict_name,
                            duration,
                            cached_dict.terms.len()
                        );
                        let mut manager_guard = manager.lock().unwrap();
                        manager_guard.add_dictionary(dict_name, cached_dict);
                        return;
                    } else {
                        println!(
                            "Revision mismatch for '{}': cache={}, index={}",
                            dict_name, cached_dict.revision, index.revision
                        );
                    }
                }
                Err(e) => {
                    println!(
                        "Failed to load cache for '{}': {}, rebuilding from JSON",
                        dict_name, e
                    );
                }
            }

            // Cache miss or invalid, build from JSON
            let build_start = Instant::now();
            if let Ok(term_meta_list) = parse_term_meta_bank(&path) {
                let freq_dict =
                    FrequencyDictionary::new(index.title.clone(), index.revision, term_meta_list);
                let build_duration = build_start.elapsed();
                println!("Built '{}' from JSON in {:?}", dict_name, build_duration);

                // Add to manager
                let mut manager_guard = manager.lock().unwrap();
                manager_guard.add_dictionary(dict_name.clone(), freq_dict.clone());

                // Save to cache
                if let Err(e) = save_cached_dict(&freq_dict, &cache_path) {
                    println!("Failed to save cache for '{}': {}", dict_name, e);
                }
            } else {
                println!("Failed to parse term meta bank for '{}'", dict_name);
            }
        } else {
            println!("Skipping {:?} due to unsupported format version.", path);
        }
    });

    let total_duration = start.elapsed();
    println!("Total processing time: {:?}", total_duration);

    Ok(manager.into_inner().unwrap())
}

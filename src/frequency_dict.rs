use std::{fs, iter::Rev, path::Path};

use regex::Regex;
use serde::Deserialize;

use crate::YomineError;

//https://github.com/yomidevs/yomitan/tree/master/ext/data/schemas

pub struct FrequencyDictionary {
    title: String,
    revision: String,
    terms: Vec<TermMetaBankV3>,
}


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

    #[serde(default="bool::default")]
    sequenced: bool,

    format: Option<u8>, //Must have one though
    version: Option<u8>,

    author: Option<String>,
    description: Option<String>,
    attribution: Option<String>,
    frequencyMode: Option<FrequencyMode>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)] 
enum Frequency {
    SimpleNumber(f64),      
    ComplexValue {
        value: f64,
        #[serde(rename = "displayValue")]    
        display_value: Option<String>,  
    },
}

#[derive(Deserialize, Debug)]
struct FreqData {
    #[serde(flatten)]
    frequency: Frequency, // Frequency information
}

impl From<Frequency> for f64 {
    fn from(value: Frequency) -> Self {
        match value {
            Frequency::SimpleNumber(num) => num,
            Frequency::ComplexValue {value, ..} => value,
        }
    }
}

#[derive(Deserialize, Debug)]
struct TermMetaBankV3 {
    term: String, // The text for the term
    #[serde(rename = "type")]
    data_type: String, // "freq", "pitch", or "ipa"
    #[serde(flatten)]
    data: Option<FreqData>, // Only populated for "freq" types
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
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.file_name().and_then(|n| n.to_str()).map_or(false, |name| re.is_match(name)))
        .flat_map(|path| {
            fs::read_to_string(&path)
                .ok()
                .and_then(|data| serde_json::from_str::<Vec<TermMetaBankV3>>(&data).ok())
                .unwrap_or_default()
        })
        .filter(|meta| meta.data_type == "freq")
        .collect();

    Ok(term_meta_list)
}

pub fn process_frequency_dictionaries() -> Result<Vec<FrequencyDictionary>, YomineError> {

    let mut dictionaries: Vec<FrequencyDictionary> = Vec::new();

    for entry in fs::read_dir("frequency_dict")?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Parse index.json and skip folders with unsupported formats
        let index = match parse_index_json(&path)? {
            Some(index) => index,
            None => {
                println!("Skipping {:?} due to unsupported format version.", path);
                continue;
            }
        };

        let freq_data = parse_term_meta_bank(&path)?;
        let freq_dict = FrequencyDictionary {
            title: index.title,
            revision: index.revision,
            terms: freq_data,
        };

        dictionaries.push(freq_dict);
    }
    Ok(dictionaries)
}
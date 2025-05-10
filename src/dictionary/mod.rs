pub mod token_dictionary;
pub mod frequency_dict;
pub mod frequency_manager;

type FrequencyData = CacheFrequencyData;
type Frequency = CacheFrequency;

use serde::{ Deserialize, Serialize };
use crate::core::utils::deserialize_number_or_numeric_string;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum JsonFrequency {
    #[serde(deserialize_with = "deserialize_number_or_numeric_string")] Number(u32),
    Complex {
        #[serde(deserialize_with = "deserialize_number_or_numeric_string")]
        value: u32,

        #[serde(rename = "displayValue")]
        display_value: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CacheFrequency {
    Number(u32),
    Complex {
        value: u32,
        display_value: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum JsonFrequencyData {
    Simple(JsonFrequency),
    Nested {
        reading: String,
        frequency: JsonFrequency,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CacheFrequencyData {
    Simple(CacheFrequency),
    Nested {
        reading: String,
        frequency: CacheFrequency,
    },
}

impl CacheFrequencyData {
    pub fn set_reading(&mut self, new_reading: String) {
        if let CacheFrequencyData::Nested { reading, .. } = self {
            *reading = new_reading;
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")] // Match the JSON naming convention
enum FrequencyMode {
    OccurrenceBased,
    RankBased,
}

#[derive(Deserialize, Debug)]
pub struct DictionaryIndex {
    pub title: String,
    pub revision: String,

    pub format: Option<u8>, //Must have one
    pub version: Option<u8>,
}

#[derive(Deserialize, Debug)]
pub struct TermMetaBankV3 {
    pub term: String, // The text for the term
    #[serde(rename = "type")]
    pub data_type: String, // "freq", "pitch", or "ipa"
    pub data: Option<JsonFrequencyData>, // Only populated for "freq" types
}

impl CacheFrequency {
    pub fn value(&self) -> u32 {
        match self {
            CacheFrequency::Number(num) => *num,
            CacheFrequency::Complex { value, .. } => *value,
        }
    }

    pub fn display_value(&self) -> Option<&str> {
        match self {
            CacheFrequency::Number(_) => None,
            CacheFrequency::Complex { display_value, .. } => display_value.as_deref(),
        }
    }
}

impl CacheFrequencyData {
    pub fn value(&self) -> u32 {
        match self {
            CacheFrequencyData::Simple(simple) => simple.value(),
            CacheFrequencyData::Nested { frequency, .. } => frequency.value(),
        }
    }

    pub fn display_value(&self) -> Option<&str> {
        match self {
            CacheFrequencyData::Simple(simple) => simple.display_value(),
            CacheFrequencyData::Nested { frequency, .. } => frequency.display_value(),
        }
    }

    pub fn reading(&self) -> Option<&str> {
        match self {
            CacheFrequencyData::Nested { reading, .. } => Some(reading.as_str()),
            CacheFrequencyData::Simple(_) => None,
        }
    }

    pub fn has_special_marker(&self) -> bool {
        self.display_value().map_or(false, |value| value.contains('㋕'))
    }
}

impl From<JsonFrequency> for CacheFrequency {
    fn from(json_freq: JsonFrequency) -> Self {
        match json_freq {
            JsonFrequency::Number(n) => CacheFrequency::Number(n),
            JsonFrequency::Complex { value, display_value } =>
                CacheFrequency::Complex {
                    value,
                    display_value,
                },
        }
    }
}

impl From<JsonFrequencyData> for CacheFrequencyData {
    fn from(json_data: JsonFrequencyData) -> Self {
        match json_data {
            JsonFrequencyData::Simple(freq) => CacheFrequencyData::Simple(freq.into()),
            JsonFrequencyData::Nested { reading, frequency } =>
                CacheFrequencyData::Nested {
                    reading,
                    frequency: frequency.into(),
                },
        }
    }
}

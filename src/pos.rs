use std::{collections::HashMap, fmt, fs};
use serde::Deserialize;
use serde_hjson::from_str;

use crate::core::{models::PartOfSpeech, YomineError};

#[derive(Debug, Deserialize)]
struct PosData {
    pos: Vec<PartOfSpeech>,
}

impl fmt::Display for PartOfSpeech {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} - {}]", self.key, self.english_name)
    }
}

#[derive(Debug)]
pub struct PosLookup {
    data: HashMap<String, PartOfSpeech>, // Hash map for fast lookups
}

impl PosLookup {
    /// Create a new empty PosLookup
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Insert a PartOfSpeech into the lookup
    pub fn insert(&mut self, key: &str, value: PartOfSpeech) {
        self.data.insert(key.to_string(), value);
    }

    /// Resolve a PartOfSpeech by its key, with fallback
    pub fn resolve(&self, pos_key: &str) -> PartOfSpeech {
        let parts: Vec<&str> = pos_key.split(" -> ").collect();

        // Try progressively shorter keys
        for i in (1..=parts.len()).rev() {
            let truncated_key = parts[..i].join(" -> ");

            if let Some(value) = self.data.get(&truncated_key) {
                return value.clone();
            }
        }

        // Fallback to "Unknown"
        let mut english_name = pos_key.to_string();

        print!("\nCould not find: POS{}", pos_key);

        english_name.insert_str(0, "Unknown: ");

        PartOfSpeech {
            key: pos_key.to_string(),
            english_name,
            hint: "".to_string(),
            examples: vec![],
        }
    }

}

pub fn load_pos_lookup() -> Result<PosLookup, YomineError> {
    let pos_file = fs::read_to_string("lib/pos.hjson")?;
    let pos_data: PosData = from_str(&pos_file)?;

    let mut pos_lookup = PosLookup::new();
    for p in pos_data.pos {
        pos_lookup.insert(&p.key, p.clone());
    }

    Ok(pos_lookup)
}
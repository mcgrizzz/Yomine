use std::collections::HashMap;

use serde::{
    Deserialize,
    Serialize,
};
use wana_kana::ConvertJapanese;

use crate::segmentation::word::POS;

const JLPT_JSON: &str = include_str!("../../assets/jlpt_vocab.json");

/// Declaration order is easiest → hardest, so `Ord::min` picks the easier level.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum JlptLevel {
    N5,
    N4,
    N3,
    N2,
    N1,
}

impl JlptLevel {
    /// Display order, easiest (N5) to hardest (N1).
    pub const ALL: [JlptLevel; 5] =
        [JlptLevel::N5, JlptLevel::N4, JlptLevel::N3, JlptLevel::N2, JlptLevel::N1];

    pub fn label(self) -> &'static str {
        match self {
            JlptLevel::N5 => "N5",
            JlptLevel::N4 => "N4",
            JlptLevel::N3 => "N3",
            JlptLevel::N2 => "N2",
            JlptLevel::N1 => "N1",
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct JlptEntry {
    /// Kanji form. May be empty for kana-only words.
    pub kanji: String,
    pub kana: String,
    pub level: JlptLevel,
    pub pos: POS,
}

pub struct JlptDatabase {
    entries: Vec<JlptEntry>,
    /// (form, hiragana reading) → level; forms are both kanji and kana
    /// spellings so kana-written lemmas of kanji entries still match.
    index: HashMap<(String, String), JlptLevel>,
}

impl JlptDatabase {
    pub fn load() -> Self {
        match serde_json::from_str::<Vec<JlptEntry>>(JLPT_JSON) {
            Ok(entries) => {
                let mut index = HashMap::new();
                for entry in &entries {
                    let kana_h = entry.kana.to_hiragana();
                    let mut insert = |form: &str| {
                        index
                            .entry((form.to_string(), kana_h.clone()))
                            .and_modify(|level: &mut JlptLevel| *level = (*level).min(entry.level))
                            .or_insert(entry.level);
                    };
                    insert(&entry.kana);
                    if !entry.kanji.is_empty() {
                        insert(&entry.kanji);
                    }
                }
                Self { entries, index }
            }
            Err(e) => {
                eprintln!("Failed to parse bundled jlpt_vocab.json: {e}");
                Self { entries: Vec::new(), index: HashMap::new() }
            }
        }
    }

    pub fn entries_for_level(&self, level: JlptLevel) -> impl Iterator<Item = &JlptEntry> {
        self.entries.iter().filter(move |e| e.level == level)
    }

    pub fn level_for(&self, form: &str, reading_hiragana: &str) -> Option<JlptLevel> {
        self.index.get(&(form.to_string(), reading_hiragana.to_string())).copied()
    }
}

use serde::{
    Deserialize,
    Serialize,
};

use crate::segmentation::word::POS;

const JLPT_JSON: &str = include_str!("../../assets/jlpt_vocab.json");

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
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
}

impl JlptDatabase {
    pub fn load() -> Self {
        match serde_json::from_str::<Vec<JlptEntry>>(JLPT_JSON) {
            Ok(entries) => Self { entries },
            Err(e) => {
                eprintln!("Failed to parse bundled jlpt_vocab.json: {e}");
                Self { entries: Vec::new() }
            }
        }
    }

    pub fn entries_for_level(&self, level: JlptLevel) -> impl Iterator<Item = &JlptEntry> {
        self.entries.iter().filter(move |e| e.level == level)
    }
}

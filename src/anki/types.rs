#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub id: u64,
    pub fields: Vec<String>,
    pub note_count: usize,
    pub sample_note: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldMapping {
    pub term_field: String,
    pub reading_field: String,
    /// Sentence field for already-mined detection (issue #3); optional so
    /// existing settings.json files load unchanged.
    #[serde(default)]
    pub sentence_field: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vocab {
    pub term: String,
    pub reading: String,
    pub card_id: Option<u64>,
    pub interval: Option<f32>, // Interval in days (can be fractional for learning cards)
}

//TODO: Comprehensibility via FSRS state
//Tsunagi integration to fetch FSRS parameters

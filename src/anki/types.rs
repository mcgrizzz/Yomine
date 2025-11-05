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
}

#[derive(Debug)]
pub struct Vocab {
    pub term: String,
    pub reading: String,
    pub card_id: Option<u64>,
    pub interval: Option<f32>, // Interval in days (can be fractional for learning cards)
}

//TODO: Comprehensibility via FSRS state
//Tsunagi integration to fetch FSRS parameters

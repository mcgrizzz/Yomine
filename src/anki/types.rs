#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub id: u64,
    pub fields: Vec<String>,
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
}

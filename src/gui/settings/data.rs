use std::collections::HashMap;

use crate::anki::FieldMapping;

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SettingsData {
    pub anki_model_mappings: HashMap<String, FieldMapping>,
}

impl SettingsData {
    pub fn new() -> Self {
        Self { anki_model_mappings: HashMap::new() }
    }
}

#[derive(Default, Clone)]
pub struct ModelMappingEditor {
    pub model_name: String,
    pub term_field: String,
    pub reading_field: String,
    pub is_editing: bool,
    pub original_model_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AnkiModelInfo {
    pub name: String,
    pub fields: Vec<String>,
    pub sample_note: Option<HashMap<String, String>>,
}

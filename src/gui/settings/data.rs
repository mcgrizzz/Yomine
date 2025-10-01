use std::collections::HashMap;

use crate::anki::FieldMapping;

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct FrequencyDictionarySetting {
    pub weight: f32,
    pub enabled: bool,
}

impl Default for FrequencyDictionarySetting {
    fn default() -> Self {
        Self { weight: 1.0, enabled: true }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct WebSocketSettings {
    pub port: u16,
}

impl Default for WebSocketSettings {
    fn default() -> Self {
        Self { port: 8766 }
    }
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SettingsData {
    pub anki_model_mappings: HashMap<String, FieldMapping>,
    #[serde(default)]
    pub websocket_settings: WebSocketSettings,
    #[serde(default)]
    pub frequency_weights: HashMap<String, FrequencyDictionarySetting>,
    #[serde(default)]
    pub pos_filters: HashMap<String, bool>,
}

impl SettingsData {
    pub fn new() -> Self {
        Self {
            anki_model_mappings: HashMap::new(),
            websocket_settings: WebSocketSettings::default(),
            frequency_weights: HashMap::new(),
            pos_filters: HashMap::new(),
        }
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

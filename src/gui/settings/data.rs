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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SettingsData {
    pub anki_model_mappings: HashMap<String, FieldMapping>,
    #[serde(default = "default_interval")]
    pub anki_interval: u32,
    #[serde(default)]
    pub websocket_settings: WebSocketSettings,
    #[serde(default)]
    pub frequency_weights: HashMap<String, FrequencyDictionarySetting>,
    #[serde(default)]
    pub pos_filters: HashMap<String, bool>,
    #[serde(default)]
    pub use_serif_font: bool,
    #[serde(default = "default_true")]
    pub dark_mode: bool,
}

const fn default_interval() -> u32 {
    30
}

const fn default_true() -> bool {
    true
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            anki_model_mappings: HashMap::new(),
            anki_interval: default_interval(),
            websocket_settings: WebSocketSettings::default(),
            frequency_weights: HashMap::new(),
            pos_filters: HashMap::new(),
            use_serif_font: false,
            dark_mode: true,
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

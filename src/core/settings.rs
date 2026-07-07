//! Persisted application settings and Anki model metadata.
//!
//! These types are UI-neutral (Constitution IV) so both the egui app and the
//! Tauri backend can read/write them. They cross the Tauri IPC boundary, so they
//! derive serde (Constitution III). The on-disk format (`settings.json`) is
//! unchanged, so existing users' settings load in both apps.

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
    /// Follow mode (issue #105): after a load from asbplayer, automatically load
    /// NEW videos asbplayer binds (with subtitles). Opt-in, persisted.
    #[serde(default)]
    pub asbplayer_follow_new_media: bool,
    /// Follow mode (issue #105): after a load from asbplayer, switch to the
    /// active tab's video when it isn't the loaded one. Opt-in, persisted.
    #[serde(default)]
    pub asbplayer_follow_active_tab: bool,
    /// How often follow mode polls asbplayer's bound-media list, in seconds.
    #[serde(default = "default_asbplayer_poll_secs")]
    pub asbplayer_poll_secs: u32,
}

const fn default_asbplayer_poll_secs() -> u32 {
    3
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
            asbplayer_follow_new_media: false,
            asbplayer_follow_active_tab: false,
            asbplayer_poll_secs: default_asbplayer_poll_secs(),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AnkiModelInfo {
    pub name: String,
    pub fields: Vec<String>,
    pub sample_note: Option<HashMap<String, String>>,
}

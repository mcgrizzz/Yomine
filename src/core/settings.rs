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

/// How sentence segments are marked in the term table (issue #94).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SentenceColoring {
    #[default]
    Knowledge,
    None,
}

// Manual so an unrecognized value (e.g. the removed "pos") falls back to the
// default instead of failing the whole settings.json load.
impl<'de> serde::Deserialize<'de> for SentenceColoring {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer)?.as_str() {
            "none" => Self::None,
            _ => Self::Knowledge,
        })
    }
}

/// Per-state visibility of the knowledge underlines (issue #94).
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct UnderlineToggles {
    #[serde(default = "default_true")]
    pub unknown: bool,
    #[serde(default = "default_true")]
    pub new: bool,
    #[serde(default = "default_true")]
    pub young: bool,
    #[serde(default = "default_true")]
    pub mature: bool,
}

impl Default for UnderlineToggles {
    fn default() -> Self {
        Self { unknown: true, new: true, young: true, mature: true }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TableColumn {
    pub id: String,
    pub visible: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TextFilterSetting {
    pub pattern: String,
    pub replacement: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    /// Follow mode (issue #105): auto-load NEW subtitled videos asbplayer binds.
    #[serde(default)]
    pub asbplayer_follow_new_media: bool,
    /// Follow mode (issue #105): switch to asbplayer's active subtitled tab.
    #[serde(default)]
    pub asbplayer_follow_active_tab: bool,
    /// How often follow mode polls asbplayer's bound-media list, in seconds.
    #[serde(default = "default_asbplayer_poll_secs")]
    pub asbplayer_poll_secs: u32,
    /// Whole-UI scale factor (Tauri app only; 1.0 = 100%). The egui app ignores it.
    #[serde(default = "default_font_scale")]
    pub font_scale: f32,
    /// Definition popover scale factor (issue #113), independent of `font_scale`.
    #[serde(default = "default_font_scale")]
    pub definition_scale: f32,
    /// yomitan-api base URL (one-click mining, issue #105).
    #[serde(default = "default_yomitan_url")]
    pub yomitan_url: String,
    /// Path or command name of the mpv executable (issue #89).
    #[serde(default = "default_mpv_path")]
    pub mpv_path: String,
    #[serde(default)]
    pub sentence_coloring: SentenceColoring,
    #[serde(default)]
    pub sentence_underlines: UnderlineToggles,
    /// JLPT level tags in the term table (issue #112); filtering is unaffected.
    #[serde(default = "default_true")]
    pub show_jlpt_tags: bool,
    /// Term-table column order/visibility (issue #122); empty = built-in layout.
    #[serde(default)]
    pub table_columns: Vec<TableColumn>,
    /// Custom regex text filters (issue #92), applied in order.
    #[serde(default)]
    pub text_filters: Vec<TextFilterSetting>,
    /// Preset id → enabled (`text_filter::presets`); missing = off.
    #[serde(default)]
    pub text_filter_presets: HashMap<String, bool>,
}

const fn default_font_scale() -> f32 {
    1.0
}

fn default_yomitan_url() -> String {
    "http://127.0.0.1:19633".to_string()
}

fn default_mpv_path() -> String {
    "mpv".to_string()
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
            font_scale: default_font_scale(),
            definition_scale: default_font_scale(),
            yomitan_url: default_yomitan_url(),
            mpv_path: default_mpv_path(),
            sentence_coloring: SentenceColoring::default(),
            sentence_underlines: UnderlineToggles::default(),
            show_jlpt_tags: true,
            table_columns: Vec::new(),
            text_filters: Vec::new(),
            text_filter_presets: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AnkiModelInfo {
    pub name: String,
    pub fields: Vec<String>,
    pub sample_note: Option<HashMap<String, String>>,
}

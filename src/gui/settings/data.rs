// The persisted settings data types now live in `core::settings` (UI-neutral, so
// the Tauri backend can use them too). Re-exported here so existing
// `gui::settings::data::*` references keep resolving.
pub use crate::core::settings::{
    AnkiModelInfo,
    FrequencyDictionarySetting,
    SettingsData,
    WebSocketSettings,
};

/// egui-only editor state for the Anki model-mapping form. Stays in `gui`.
#[derive(Default, Clone)]
pub struct ModelMappingEditor {
    pub model_name: String,
    pub term_field: String,
    pub reading_field: String,
    pub is_editing: bool,
    pub original_model_name: Option<String>,
}

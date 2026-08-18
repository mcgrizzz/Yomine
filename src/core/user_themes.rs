//! The user's custom theme library, stored apart from `settings.json` so it can
//! be shared while the active-theme slots stay per-settings.

use crate::{
    core::settings::UserTheme,
    persistence,
};

pub const USER_THEMES_FILE: &str = "user_themes.json";

pub fn load() -> Vec<UserTheme> {
    persistence::load_json_at_or_default(&persistence::get_shared_file_path(USER_THEMES_FILE))
}

pub fn save(themes: &[UserTheme]) -> Result<(), Box<dyn std::error::Error>> {
    persistence::save_json_at(
        &themes.to_vec(),
        &persistence::get_shared_file_path(USER_THEMES_FILE),
    )
}

/// Reads the raw JSON because `SettingsData` no longer carries the field.
fn themes_in_settings(raw: &str) -> Option<Vec<UserTheme>> {
    let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;
    let themes =
        serde_json::from_value::<Vec<UserTheme>>(value.get("user_themes")?.clone()).ok()?;
    (!themes.is_empty()).then_some(themes)
}

pub fn migrate_from_settings() {
    if persistence::get_shared_file_path(USER_THEMES_FILE).exists() {
        return;
    }
    let Ok(raw) = std::fs::read_to_string(persistence::get_data_file_path("settings.json")) else {
        return;
    };
    if let Some(themes) = themes_in_settings(&raw) {
        if let Err(e) = save(&themes) {
            eprintln!("Failed to migrate user themes: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_pre_split_theme_library() {
        let raw = r##"{"dark_mode":true,"user_themes":[
            {"name":"Mine","dark":true,"colors":{"bg":"#111111"}}
        ]}"##;
        let themes = themes_in_settings(raw).expect("themes");
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "Mine");
        assert_eq!(themes[0].colors.get("bg").map(String::as_str), Some("#111111"));
    }

    #[test]
    fn nothing_to_migrate_is_none() {
        assert!(themes_in_settings(r#"{"dark_mode":true}"#).is_none());
        assert!(themes_in_settings(r#"{"user_themes":[]}"#).is_none());
        assert!(themes_in_settings("not json").is_none());
    }
}

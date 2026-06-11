//! Anki commands (T028, contracts/commands.md "Anki"). Connectivity and the note
//! model list the Anki settings modal needs for field mapping. Live connectivity
//! is also pushed ambiently via the `anki-status` event (background task, R5).

use yomine::{
    anki,
    core::settings::AnkiModelInfo,
};

use crate::events::AnkiStatus;

/// Point-in-time Anki connectivity probe (mirrors egui's `update_anki_status`).
/// `fetching` is always `false` here — it is a connectivity check, not a query;
/// the background poll emits the same shape on the `anki-status` event.
#[tauri::command]
pub async fn get_anki_status() -> AnkiStatus {
    let connected = anki::api::get_version().await.is_ok();
    AnkiStatus { connected, fetching: false }
}

/// Note types (with their fields) that have at least one note, for the mapping UI
/// and field guessing. Mirrors egui's `AnkiService::fetch_models` (errors when
/// Anki is offline so the UI can surface "Anki Offline").
#[tauri::command]
pub async fn list_anki_models() -> Result<Vec<AnkiModelInfo>, String> {
    anki::api::get_version().await.map_err(|_| "Anki Offline".to_string())?;

    let models = anki::get_models().await.map_err(|e| format!("Failed to fetch models: {}", e))?;

    Ok(models
        .into_iter()
        .map(|model| AnkiModelInfo {
            name: model.name,
            fields: model.fields,
            sample_note: model.sample_note,
        })
        .collect())
}

//! Anki commands (T028, contracts/commands.md "Anki"). Connectivity and the note
//! model list the Anki settings modal needs for field mapping. Live connectivity
//! is also pushed ambiently via the `anki-status` event (background task, R5).

use std::collections::HashMap;

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

/// A model's sample note plus the engine's term/reading field guesses (T040).
#[derive(serde::Serialize)]
pub struct SampleNote {
    pub sample_note: Option<HashMap<String, String>>,
    pub guessed_term: Option<String>,
    pub guessed_reading: Option<String>,
}

/// Fetch a sample note for one note type and run the shared field-guessing
/// heuristic over it (`anki::guess_field_mappings` — the same code egui's
/// `trigger_field_guessing` uses). Mirrors egui's `fetch_sample_note`, which
/// swallows errors into "no sample" (`.unwrap_or(None)`), so this never errors.
#[tauri::command]
pub async fn get_anki_sample_note(model_name: String, fields: Vec<String>) -> SampleNote {
    let sample_note = anki::get_sample_note_for_model(&model_name).await.unwrap_or(None);
    let (guessed_term, guessed_reading) = sample_note
        .as_ref()
        .map(|note| anki::guess_field_mappings(note, &fields))
        .unwrap_or((None, None));

    SampleNote { sample_note, guessed_term, guessed_reading }
}

//! Anki commands (contracts/commands.md "Anki").

use std::collections::HashMap;

use yomine::{
    anki,
    core::settings::AnkiModelInfo,
};

use crate::events::AnkiStatus;

/// Point-in-time connectivity probe; `fetching` is always `false` here.
#[tauri::command]
pub async fn get_anki_status() -> AnkiStatus {
    let connected = anki::api::get_version().await.is_ok();
    AnkiStatus { connected, fetching: false }
}

/// Note types (with fields) that have at least one note. Errors when Anki is
/// offline so the UI can say so.
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

/// A model's sample note plus the engine's term/reading field guesses.
#[derive(serde::Serialize)]
pub struct SampleNote {
    pub sample_note: Option<HashMap<String, String>>,
    pub guessed_term: Option<String>,
    pub guessed_reading: Option<String>,
}

/// Sample note + engine-side field guessing for one note type. Errors are
/// swallowed into "no sample", so this never rejects.
#[tauri::command]
pub async fn get_anki_sample_note(model_name: String, fields: Vec<String>) -> SampleNote {
    let sample_note = anki::get_sample_note_for_model(&model_name).await.unwrap_or(None);
    let (guessed_term, guessed_reading) = sample_note
        .as_ref()
        .map(|note| anki::guess_field_mappings(note, &fields))
        .unwrap_or((None, None));

    SampleNote { sample_note, guessed_term, guessed_reading }
}

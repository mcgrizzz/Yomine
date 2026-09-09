//! Anki commands (contracts/commands.md "Anki").

use std::collections::HashMap;

use yomine::{
    anki,
    core::{
        errors::YomineError,
        settings::{
            AnkiConnectionSettings,
            AnkiModelInfo,
        },
    },
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
pub async fn list_anki_models(
    connection: AnkiConnectionSettings,
) -> Result<Vec<AnkiModelInfo>, String> {
    let client = anki::api::AnkiClient::new(connection);
    let mut models = anki::get_models(&client).await.map_err(|e| e.to_string())?;
    models.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(models
        .into_iter()
        .map(|model| AnkiModelInfo {
            name: model.name,
            fields: model.fields,
            sample_note: model.sample_note,
        })
        .collect())
}

#[derive(serde::Serialize)]
pub struct ConnectionError {
    message: String,
    detail: String,
}

#[tauri::command]
pub async fn test_anki_connection(
    connection: AnkiConnectionSettings,
) -> Result<u32, ConnectionError> {
    anki::api::AnkiClient::new(connection.clone()).get_version().await.map_err(|error| {
        let message = match &error {
            YomineError::Reqwest(e) if e.is_connect() || e.is_timeout() => format!(
                "Cannot reach Anki on port {}. Open Anki and check that the add-on is running.",
                connection.port
            ),
            YomineError::Custom(message)
                if message.to_lowercase().contains("key")
                    || message.to_lowercase().contains("auth") =>
            {
                "Anki rejected the API key. Check Authentication settings.".into()
            }
            YomineError::Custom(_) => {
                "Anki rejected the request. Check the add-on configuration.".into()
            }
            _ => "Anki returned an unexpected response. Check the port and add-on configuration."
                .into(),
        };
        let mut detail = error.to_string();
        if !connection.api_key.is_empty() {
            detail = detail.replace(&connection.api_key, "[redacted]");
        }
        ConnectionError { message, detail }
    })
}

/// A model's sample note plus the engine's term/reading/sentence field guesses.
#[derive(serde::Serialize)]
pub struct SampleNote {
    pub sample_note: Option<HashMap<String, String>>,
    pub guessed_term: Option<String>,
    pub guessed_reading: Option<String>,
    pub guessed_sentence: Option<String>,
}

/// Sample note and field guesses for the supplied connection.
#[tauri::command]
pub async fn get_anki_sample_note(
    connection: AnkiConnectionSettings,
    model_name: String,
    fields: Vec<String>,
) -> Result<SampleNote, String> {
    let client = anki::api::AnkiClient::new(connection);
    let sample_note =
        anki::get_sample_note_for_model(&client, &model_name).await.map_err(|e| e.to_string())?;
    let (guessed_term, guessed_reading) = sample_note
        .as_ref()
        .map(|note| anki::guess_field_mappings(note, &fields))
        .unwrap_or((None, None));
    let guessed_sentence =
        sample_note.as_ref().and_then(|note| anki::guess_sentence_field(note, &fields));

    Ok(SampleNote { sample_note, guessed_term, guessed_reading, guessed_sentence })
}

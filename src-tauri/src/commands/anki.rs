//! Anki commands (contracts/commands.md "Anki").

use std::collections::HashMap;

use yomine::{
    anki,
    anki::AnkiError,
    core::settings::{
        AnkiConnectionSettings,
        AnkiModelInfo,
    },
    yomitan,
};

use crate::events::AnkiStatus;

/// Point-in-time connectivity probe; `fetching` is always `false` here.
#[tauri::command]
pub async fn get_anki_status() -> AnkiStatus {
    AnkiStatus { connected: anki::reachable().await, fetching: false }
}

/// Syncs Yomine's copy of the collection in the background, for moments Anki may have
/// changed (the window regaining focus, a batch ending).
#[tauri::command]
pub fn sync_anki(app: tauri::AppHandle) {
    crate::anki_sync::hint(&app);
}

/// All note types and their fields, including empty types.
#[tauri::command]
pub async fn list_anki_models(
    connection: AnkiConnectionSettings,
) -> Result<Vec<AnkiModelInfo>, String> {
    let anki = anki::probe(connection).detected().await;
    let mut models = anki.note_types().await.map_err(|e| e.to_string())?;
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
) -> Result<anki::Backend, ConnectionError> {
    anki::probe(connection.clone()).detect().await.map_err(|error| {
        let message = match &error {
            AnkiError::NotSent(_) | AnkiError::TimedOut(_) => format!(
                "Cannot reach Anki at {} on port {}. Check the address and that the add-on is running.",
                connection.host, connection.port
            ),
            AnkiError::Rejected(message)
                if message.to_lowercase().contains("key")
                    || message.to_lowercase().contains("auth") =>
            {
                "Anki rejected the API key. Check Authentication settings.".into()
            }
            AnkiError::Rejected(_) => {
                "Anki rejected the request. Check the add-on configuration.".into()
            }
            AnkiError::Unconfirmed(_) => {
                "Anki returned an unexpected response. Check the host, port and add-on configuration."
                    .into()
            }
        };
        ConnectionError { message, detail: error.to_string() }
    })
}

/// A model's sample note plus the engine's field guesses.
#[derive(serde::Serialize)]
pub struct SampleNote {
    pub sample_note: Option<HashMap<String, String>>,
    pub guessed_term: Option<String>,
    pub guessed_reading: Option<String>,
    pub guessed_sentence: Option<String>,
    pub guessed_sentence_audio: Option<String>,
    pub guessed_picture: Option<String>,
    /// A known note type ("Lapis") the guesses came from.
    pub detected: Option<String>,
    pub fields_differ: bool,
}

/// Sample note and field guesses for the supplied connection. Yomitan's card formats
/// for the note type show which field it fills with a screenshot or word audio.
#[tauri::command]
pub async fn get_anki_sample_note(
    connection: AnkiConnectionSettings,
    yomitan_url: String,
    model_name: String,
    fields: Vec<String>,
) -> Result<SampleNote, String> {
    let anki = anki::probe(connection).detected().await;
    let sample_note = anki.sample_note(&model_name).await.map_err(|e| e.to_string())?;
    let templates: HashMap<String, String> = yomitan::get_term_card_formats(&yomitan_url)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|format| format.model == model_name)
        .flat_map(|format| format.fields.into_iter().map(|(name, field)| (name, field.value)))
        .collect();
    let guess = anki::guess_mapping(&model_name, &fields, sample_note.as_ref(), &templates);

    Ok(SampleNote {
        sample_note,
        guessed_term: guess.term,
        guessed_reading: guess.reading,
        guessed_sentence: guess.sentence,
        guessed_sentence_audio: guess.sentence_audio,
        guessed_picture: guess.picture,
        detected: guess.detected.map(str::to_string),
        fields_differ: guess.fields_differ,
    })
}

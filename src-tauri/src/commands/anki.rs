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
    yomitan,
};

use crate::events::AnkiStatus;

/// Point-in-time connectivity probe; `fetching` is always `false` here.
#[tauri::command]
pub async fn get_anki_status() -> AnkiStatus {
    let connected = anki::api::get_version().await.is_ok();
    AnkiStatus { connected, fetching: false }
}

/// All note types and their fields, including empty types.
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
                "Cannot reach Anki at {} on port {}. Check the address and that the add-on is running.",
                connection.host, connection.port
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
            _ => "Anki returned an unexpected response. Check the host, port and add-on configuration."
                .into(),
        };
        let mut detail = error.to_string();
        if !connection.api_key.is_empty() {
            detail = detail.replace(&connection.api_key, "[redacted]");
        }
        ConnectionError { message, detail }
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
    let client = anki::api::AnkiClient::new(connection);
    let sample_note =
        anki::get_sample_note_for_model(&client, &model_name).await.map_err(|e| e.to_string())?;
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

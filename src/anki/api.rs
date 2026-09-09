use std::{
    collections::HashMap,
    sync::{
        LazyLock,
        RwLock,
    },
};

use reqwest::Client;
use serde::{
    Deserialize,
    Serialize,
};

use crate::core::{
    errors::YomineError,
    settings::AnkiConnectionSettings,
};

static CONNECTION: LazyLock<RwLock<AnkiConnectionSettings>> =
    LazyLock::new(|| RwLock::new(AnkiConnectionSettings::default()));

pub fn configure_connection(settings: AnkiConnectionSettings) {
    *CONNECTION.write().unwrap() = settings;
}

#[derive(Debug)]
pub struct Deck {
    pub name: String,
    pub id: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Field {
    pub value: String,
    order: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub note_id: u64,
    profile: String,
    tags: Vec<String>,
    pub fields: HashMap<String, Field>,
    pub model_name: String,
    #[serde(rename = "mod")]
    modified: u64,
    pub cards: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    answer: String,
    question: String,
    deck_name: String,
    model_name: String,
    field_order: u32,
    fields: HashMap<String, Field>,
    css: String,
    pub card_id: u64,
    pub interval: u32,
    note: u64,
    ord: u32,
    #[serde(rename = "type")]
    _type: u32,
    queue: u32,
    due: u32,
    reps: u32,
    lapses: u32,
    left: u32,
    #[serde(rename = "mod")]
    modified: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub result: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn unwrap_result(self) -> Option<T> {
        if self.error.is_some() {
            eprintln!("API error: {:?}", self.error);
        }
        self.result
    }
}

async fn make_request<T: for<'de> Deserialize<'de>>(
    action: &str,
    params: Option<serde_json::Value>,
) -> Result<ApiResponse<T>, reqwest::Error> {
    let request = build_request(&CONNECTION.read().unwrap(), action, params);
    request.send().await?.json().await
}

fn build_request(
    connection: &AnkiConnectionSettings,
    action: &str,
    params: Option<serde_json::Value>,
) -> reqwest::RequestBuilder {
    let mut body = serde_json::Map::new();
    body.insert("action".to_string(), serde_json::Value::String(action.to_string()));
    body.insert("version".to_string(), serde_json::Value::Number((6).into()));

    if let Some(params) = params {
        body.insert("params".to_string(), params);
    }

    if !connection.api_key.is_empty() {
        body.insert("key".to_string(), serde_json::Value::String(connection.api_key.clone()));
    }
    Client::new().post(format!("http://localhost:{}/", connection.port)).json(&body)
}

pub async fn get_version() -> Result<u32, YomineError> {
    let response: ApiResponse<u32> = make_request("version", None).await?;
    match (response.result, response.error) {
        (Some(version), None) => Ok(version),
        (_, error) => {
            Err(YomineError::Custom(format!("Not an AnkiConnect version response: {error:?}")))
        }
    }
}

pub async fn get_deck_ids() -> Result<Vec<Deck>, reqwest::Error> {
    let response: ApiResponse<HashMap<String, u64>> = make_request("deckNamesAndIds", None).await?;

    Ok(response
        .unwrap_result()
        .unwrap_or_default()
        .into_iter()
        .map(|(name, id)| Deck { name, id })
        .collect())
}

pub async fn get_note_ids(query: &str) -> Result<Vec<u64>, reqwest::Error> {
    let params = serde_json::json!({ "query": query });
    let response: ApiResponse<Vec<u64>> = make_request("findNotes", Some(params)).await?;
    Ok(response.unwrap_result().unwrap_or_default())
}

pub async fn get_notes(note_ids: Vec<u64>) -> Result<Vec<Note>, reqwest::Error> {
    let params = serde_json::json!({ "notes": note_ids });
    let response: ApiResponse<Vec<Note>> = make_request("notesInfo", Some(params)).await?;
    Ok(response.unwrap_result().unwrap_or_default())
}

pub async fn get_cards(card_ids: Vec<u64>) -> Result<Vec<Card>, reqwest::Error> {
    let params = serde_json::json!({ "cards": card_ids });
    let response: ApiResponse<Vec<Card>> = make_request("cardsInfo", Some(params)).await?;
    Ok(response.unwrap_result().unwrap_or_default())
}

pub async fn get_intervals(card_ids: Vec<u64>) -> Result<Vec<i32>, reqwest::Error> {
    let params = serde_json::json!({ "cards": card_ids });
    let response: ApiResponse<Vec<i32>> = make_request("getIntervals", Some(params)).await?;
    Ok(response.unwrap_result().unwrap_or_default())
}

/// Create a note (one-click mining, issue #105). Returns the raw `ApiResponse`
/// so callers can tell a duplicate rejection apart from other errors.
pub async fn add_note(
    deck_name: &str,
    model_name: &str,
    fields: &HashMap<String, String>,
    tags: &[String],
) -> Result<ApiResponse<u64>, reqwest::Error> {
    let params = serde_json::json!({
        "note": {
            "deckName": deck_name,
            "modelName": model_name,
            "fields": fields,
            "tags": tags,
            "options": { "allowDuplicate": false }
        }
    });
    make_request("addNote", Some(params)).await
}

/// Open Anki's card browser on a search (e.g. `nid:123`) — the "open the card
/// I just mined" affordance.
pub async fn gui_browse(query: &str) -> Result<ApiResponse<Vec<u64>>, reqwest::Error> {
    let params = serde_json::json!({ "query": query });
    make_request("guiBrowse", Some(params)).await
}

/// Select a card in the open browser (`guiSelectCard`); returns false when no
/// browser is open.
pub async fn gui_select_card(card_id: u64) -> Result<ApiResponse<bool>, reqwest::Error> {
    let params = serde_json::json!({ "card": card_id });
    make_request("guiSelectCard", Some(params)).await
}

/// Store a base64 media payload in Anki's collection (`storeMediaFile`).
pub async fn store_media_file(
    filename: &str,
    base64_data: &str,
) -> Result<ApiResponse<String>, reqwest::Error> {
    let params = serde_json::json!({ "filename": filename, "data": base64_data });
    make_request("storeMediaFile", Some(params)).await
}

pub async fn get_model_ids() -> Result<HashMap<String, u64>, reqwest::Error> {
    let response: ApiResponse<HashMap<String, u64>> =
        make_request("modelNamesAndIds", None).await?;
    Ok(response.unwrap_result().unwrap_or_default())
}

pub async fn get_field_names(model_name: &str) -> Result<Vec<String>, reqwest::Error> {
    let params = serde_json::json!({ "modelName": model_name });
    let response: ApiResponse<Vec<String>> = make_request("modelFieldNames", Some(params)).await?;
    Ok(response.unwrap_result().unwrap_or_default())
}

pub async fn get_sample_note_for_model(model_name: &str) -> Result<Option<Note>, reqwest::Error> {
    let query = if model_name.contains(' ') || model_name.contains(':') || model_name.contains('"')
    {
        format!("note:\"{}\"", model_name.replace('"', "\\\""))
    } else {
        format!("note:{}", model_name)
    };
    let note_ids = get_note_ids(&query).await?;

    if !note_ids.is_empty() {
        let mid_index = note_ids.len() / 2;
        let mid_note_id = note_ids[mid_index];
        let notes = get_notes(vec![mid_note_id]).await?;
        Ok(notes.into_iter().next())
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn default_requests_keep_the_unauthenticated_protocol() {
        let request =
            build_request(&AnkiConnectionSettings::default(), "version", None).build().unwrap();
        assert_eq!(request.url().as_str(), "http://localhost:8765/");
        assert_eq!(request.method(), reqwest::Method::POST);
        let body: serde_json::Value =
            serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(body, json!({ "action": "version", "version": 6 }));
    }

    #[test]
    fn custom_connection_sends_key_at_the_top_level() {
        let connection = AnkiConnectionSettings {
            port: std::num::NonZeroU16::new(18765).unwrap(),
            api_key: " key-\"with\\escapes ".into(),
        };
        for (action, params) in
            [("version", None), ("findNotes", Some(json!({ "query": "deck:Default" })))]
        {
            let request = build_request(&connection, action, params.clone()).build().unwrap();
            assert_eq!(request.url().as_str(), "http://localhost:18765/");
            let body: serde_json::Value =
                serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
            assert_eq!(body["action"], action);
            assert_eq!(body["version"], 6);
            assert_eq!(body["key"], connection.api_key);
            assert_eq!(body.get("params"), params.as_ref());
        }
    }
}

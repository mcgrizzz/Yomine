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
    let client = AnkiClient::new(CONNECTION.read().unwrap().clone());
    client.get_version().await
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

pub struct AnkiClient {
    connection: AnkiConnectionSettings,
}

impl AnkiClient {
    pub fn new(connection: AnkiConnectionSettings) -> Self {
        Self { connection }
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        action: &str,
        params: Option<serde_json::Value>,
    ) -> Result<T, YomineError> {
        let response: ApiResponse<T> = build_request(&self.connection, action, params)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .json()
            .await?;
        if let Some(mut error) = response.error {
            if !self.connection.api_key.is_empty() {
                error = error.replace(&self.connection.api_key, "[redacted]");
            }
            return Err(YomineError::Custom(error));
        }
        response.result.ok_or_else(|| YomineError::Custom("Anki returned no result".into()))
    }

    pub async fn get_version(&self) -> Result<u32, YomineError> {
        self.request("version", None).await
    }

    pub async fn get_model_ids(&self) -> Result<HashMap<String, u64>, YomineError> {
        self.request("modelNamesAndIds", None).await
    }

    pub async fn get_field_names(&self, model_name: &str) -> Result<Vec<String>, YomineError> {
        self.request("modelFieldNames", Some(serde_json::json!({ "modelName": model_name }))).await
    }

    pub async fn get_model_note_ids(&self, model_name: &str) -> Result<Vec<u64>, YomineError> {
        let query = format!("note:\"{}\"", model_name.replace('"', "\\\""));
        self.request("findNotes", Some(serde_json::json!({ "query": query }))).await
    }

    pub async fn get_sample_note_for_model(
        &self,
        model_name: &str,
    ) -> Result<Option<Note>, YomineError> {
        let ids = self.get_model_note_ids(model_name).await?;
        let Some(id) = ids.get(ids.len() / 2) else { return Ok(None) };
        let notes: Vec<Note> =
            self.request("notesInfo", Some(serde_json::json!({ "notes": [id] }))).await?;
        Ok(notes.into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    async fn server(
        response: serde_json::Value,
    ) -> (AnkiClient, tokio::task::JoinHandle<serde_json::Value>) {
        use tokio::io::{
            AsyncBufReadExt,
            AsyncReadExt,
            AsyncWriteExt,
            BufReader,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert!(stream.read_line(&mut line).await.unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            stream.read_exact(&mut body).await.unwrap();
            let body = serde_json::from_slice(&body).unwrap();
            let response = response.to_string();
            stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).as_bytes()).await.unwrap();
            body
        });
        (
            AnkiClient::new(AnkiConnectionSettings {
                port: std::num::NonZeroU16::new(port).unwrap(),
                api_key: "draft-key".into(),
            }),
            task,
        )
    }

    #[tokio::test]
    async fn draft_probe_uses_its_own_connection_without_changing_the_active_one() {
        let original = CONNECTION.read().unwrap().clone();
        let (client, request) = server(json!({ "result": 6, "error": null })).await;
        assert_eq!(client.get_version().await.unwrap(), 6);
        assert_eq!(
            request.await.unwrap(),
            json!({ "action": "version", "version": 6, "key": "draft-key" })
        );
        assert!(original == *CONNECTION.read().unwrap());
    }

    #[tokio::test]
    async fn authentication_and_missing_results_fail_without_exposing_the_key() {
        for response in [
            json!({ "result": null, "error": "invalid key: draft-key" }),
            json!({ "result": null, "error": null }),
        ] {
            let (client, request) = server(response).await;
            let error = client.get_version().await.unwrap_err().to_string();
            assert!(!error.contains("draft-key"));
            request.await.unwrap();
        }
    }

    #[tokio::test]
    async fn draft_model_lookup_escapes_the_note_type_name() {
        let (client, request) = server(json!({ "result": [], "error": null })).await;
        assert!(client.get_model_note_ids("Model \"A\"").await.unwrap().is_empty());
        let request = request.await.unwrap();
        assert_eq!(request["action"], "findNotes");
        assert_eq!(request["params"]["query"], "note:\"Model \\\"A\\\"\"");
    }

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

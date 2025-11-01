use std::collections::HashMap;

use reqwest::Client;
use serde::{
    Deserialize,
    Serialize,
};

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
    note_id: u64,
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
    let mut body = serde_json::Map::new();
    body.insert("action".to_string(), serde_json::Value::String(action.to_string()));
    body.insert("version".to_string(), serde_json::Value::Number((6).into()));

    if let Some(params) = params {
        body.insert("params".to_string(), params);
    }

    let response: ApiResponse<T> =
        Client::new().post("http://localhost:8765/").json(&body).send().await?.json().await?;

    Ok(response)
}

//Will just use to check if ankiconnect is online
pub async fn get_version() -> Result<u32, reqwest::Error> {
    let response: ApiResponse<u32> = make_request("version", None).await?;

    Ok(response.unwrap_result().unwrap_or_default())
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

//! AnkiConnect's protocol, which Tsunagi's shim also answers.

use std::{
    collections::HashMap,
    net::{
        Ipv4Addr,
        Ipv6Addr,
        SocketAddr,
    },
    sync::LazyLock,
    time::Duration,
};

use futures::{
    stream,
    StreamExt,
    TryStreamExt,
};
use reqwest::Client;
use serde::{
    de::DeserializeOwned,
    Deserialize,
};
use serde_json::json;

use super::{
    client::{
        setup_problem,
        Anki,
        AnkiError,
        CreateOutcome,
        NewNote,
        NoteInfo,
        PROBE_TIMEOUT,
    },
    connection::base_url,
    types::Model,
};
use crate::core::settings::AnkiConnectionSettings;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    let loopback =
        [SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), SocketAddr::from((Ipv6Addr::LOCALHOST, 0))];
    Client::builder()
        // AnkiConnect closes each response without a Connection: close header.
        .pool_max_idle_per_host(0)
        // AnkiConnect binds IPv4 only; trying ::1 first stalls each request 300ms on Windows.
        .resolve_to_addrs("localhost", &loopback)
        .build()
        .expect("failed to create Anki HTTP client")
});

#[derive(Deserialize)]
struct Field {
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Note {
    note_id: u64,
    fields: HashMap<String, Field>,
    model_name: String,
    cards: Vec<u64>,
}

impl From<Note> for NoteInfo {
    fn from(note: Note) -> Self {
        NoteInfo {
            id: note.note_id,
            note_type: note.model_name,
            fields: note.fields.into_iter().map(|(name, field)| (name, field.value)).collect(),
            cards: note.cards,
        }
    }
}

#[derive(Deserialize)]
struct Response<T> {
    result: Option<T>,
    error: Option<String>,
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
    HTTP_CLIENT.post(format!("{}/", base_url(connection))).json(&body)
}

/// The result, which may be absent without an error (`deleteNotes` answers null).
async fn send<T: DeserializeOwned>(
    anki: &Anki,
    timeout: Option<Duration>,
    action: &str,
    params: Option<serde_json::Value>,
) -> Result<Option<T>, AnkiError> {
    let transport = |error: reqwest::Error| {
        let message = anki.redact(error.to_string());
        if error.is_connect() {
            AnkiError::NotSent(message)
        } else if error.is_timeout() {
            AnkiError::TimedOut(message)
        } else {
            AnkiError::Unconfirmed(message)
        }
    };
    let mut request = build_request(&anki.connection, action, params);
    if let Some(timeout) = timeout.or(anki.timeout) {
        request = request.timeout(timeout);
    }
    let response: Response<T> =
        request.send().await.map_err(transport)?.json().await.map_err(transport)?;
    match response.error {
        Some(error) => Err(AnkiError::Rejected(anki.redact(error))),
        None => Ok(response.result),
    }
}

async fn request<T: DeserializeOwned>(
    anki: &Anki,
    action: &str,
    params: Option<serde_json::Value>,
) -> Result<T, AnkiError> {
    send(anki, None, action, params)
        .await?
        .ok_or_else(|| AnkiError::Unconfirmed("Anki returned no result".into()))
}

async fn request_empty(
    anki: &Anki,
    action: &str,
    params: Option<serde_json::Value>,
) -> Result<(), AnkiError> {
    send::<serde_json::Value>(anki, None, action, params).await.map(|_| ())
}

pub(super) async fn version(anki: &Anki) -> Result<u32, AnkiError> {
    send(anki, Some(PROBE_TIMEOUT), "version", None)
        .await?
        .ok_or_else(|| AnkiError::Unconfirmed("Anki returned no result".into()))
}

pub(super) async fn active_profile(anki: &Anki) -> Result<String, AnkiError> {
    request(anki, "getActiveProfile", None).await
}

pub(super) async fn note_types(anki: &Anki) -> Result<Vec<Model>, AnkiError> {
    let model_ids: HashMap<String, u64> = request(anki, "modelNamesAndIds", None).await?;
    stream::iter(model_ids)
        .map(|(name, id)| async move {
            let fields =
                request(anki, "modelFieldNames", Some(json!({ "modelName": name }))).await?;
            Ok(Model { name, id, fields, sample_note: None })
        })
        .buffer_unordered(8)
        .try_collect()
        .await
}

pub(super) async fn find_notes(anki: &Anki, query: &str) -> Result<Vec<u64>, AnkiError> {
    request(anki, "findNotes", Some(json!({ "query": query }))).await
}

pub(super) async fn notes(anki: &Anki, ids: &[u64]) -> Result<Vec<NoteInfo>, AnkiError> {
    let notes: Vec<Note> = request(anki, "notesInfo", Some(json!({ "notes": ids }))).await?;
    Ok(notes.into_iter().map(NoteInfo::from).collect())
}

pub(super) async fn intervals(anki: &Anki, card_ids: &[u64]) -> Result<Vec<i32>, AnkiError> {
    request(anki, "getIntervals", Some(json!({ "cards": card_ids }))).await
}

pub(super) async fn existing(anki: &Anki, ids: &[u64]) -> Result<Vec<u64>, AnkiError> {
    let mut found = Vec::new();
    for chunk in ids.chunks(500) {
        let query =
            format!("nid:{}", chunk.iter().map(u64::to_string).collect::<Vec<_>>().join(","));
        found.extend(checked_note_ids(find_notes(anki, &query).await?, chunk)?);
    }
    found.sort_unstable();
    found.dedup();
    Ok(found)
}

fn checked_note_ids(result: Vec<u64>, requested: &[u64]) -> Result<Vec<u64>, AnkiError> {
    if result.iter().any(|id| !requested.contains(id)) {
        return Err(AnkiError::Unconfirmed("Anki returned unexpected note IDs".into()));
    }
    Ok(result)
}

pub(super) async fn delete(anki: &Anki, ids: &[u64]) -> Result<(), AnkiError> {
    request_empty(anki, "deleteNotes", Some(json!({ "notes": ids }))).await
}

pub(super) async fn create_note(anki: &Anki, note: &NewNote) -> Result<CreateOutcome, AnkiError> {
    let params = json!({
        "note": {
            "deckName": note.deck,
            "modelName": note.note_type,
            "fields": note.fields,
            "tags": note.tags,
            "options": { "allowDuplicate": false }
        }
    });
    match send(anki, None, "addNote", Some(params)).await {
        Ok(Some(id)) => Ok(CreateOutcome::Created(id)),
        Ok(None) => Err(AnkiError::Unconfirmed("Anki returned no note ID".into())),
        Err(AnkiError::Rejected(reason)) if reason.contains("duplicate") => {
            Ok(CreateOutcome::Duplicate)
        }
        Err(AnkiError::Rejected(reason)) => {
            Ok(CreateOutcome::Rejected { setup: setup_problem(&reason), reason })
        }
        Err(error) => Err(error),
    }
}

pub(super) async fn update_fields(
    anki: &Anki,
    note_id: u64,
    fields: &HashMap<String, String>,
) -> Result<(), AnkiError> {
    request_empty(
        anki,
        "updateNoteFields",
        Some(json!({ "note": { "id": note_id, "fields": fields } })),
    )
    .await
}

pub(super) async fn store_media(
    anki: &Anki,
    filename: &str,
    base64_data: &str,
) -> Result<String, AnkiError> {
    request(anki, "storeMediaFile", Some(json!({ "filename": filename, "data": base64_data })))
        .await
}

pub(super) async fn media(anki: &Anki, filename: &str) -> Result<Option<String>, AnkiError> {
    // A missing file answers `false`.
    let result: Option<serde_json::Value> =
        send(anki, None, "retrieveMediaFile", Some(json!({ "filename": filename }))).await?;
    Ok(result.and_then(|v| v.as_str().map(str::to_string)))
}

pub(super) async fn browse(anki: &Anki, query: &str) -> Result<(), AnkiError> {
    request_empty(anki, "guiBrowse", Some(json!({ "query": query }))).await
}

pub(super) async fn select_card(anki: &Anki, card_id: u64) -> Result<(), AnkiError> {
    request_empty(anki, "guiSelectCard", Some(json!({ "card": card_id }))).await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn undo_lookup_does_not_confuse_errors_with_deleted_notes() {
        assert!(checked_note_ids(vec![99], &[42]).is_err());
        assert!(checked_note_ids(vec![], &[42]).unwrap().is_empty());
    }

    async fn server(
        response: serde_json::Value,
    ) -> (Anki, tokio::task::JoinHandle<serde_json::Value>) {
        use tokio::io::{
            AsyncBufReadExt,
            AsyncReadExt,
            AsyncWriteExt,
            BufReader,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.2:0").await.unwrap();
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
            super::super::probe(AnkiConnectionSettings {
                host: "127.0.0.2".into(),
                port: std::num::NonZeroU16::new(port).unwrap(),
                api_key: "draft-key".into(),
            }),
            task,
        )
    }

    #[tokio::test]
    async fn draft_probe_uses_its_own_connection_without_changing_the_active_one() {
        let original = super::super::connection::active();
        let (anki, request) = server(json!({ "result": 6, "error": null })).await;
        assert_eq!(anki.version().await.unwrap(), 6);
        assert_eq!(
            request.await.unwrap(),
            json!({ "action": "version", "version": 6, "key": "draft-key" })
        );
        assert!(original == super::super::connection::active());
    }

    #[tokio::test]
    async fn authentication_and_missing_results_fail_without_exposing_the_key() {
        for response in [
            json!({ "result": null, "error": "invalid key: draft-key" }),
            json!({ "result": null, "error": null }),
        ] {
            let (anki, request) = server(response).await;
            let error = anki.version().await.unwrap_err().to_string();
            assert!(!error.contains("draft-key"));
            request.await.unwrap();
        }
    }

    #[tokio::test]
    async fn draft_model_lookup_escapes_the_note_type_name() {
        let (anki, request) = server(json!({ "result": [], "error": null })).await;
        assert!(anki.sample_note("Model \"A\"").await.unwrap().is_none());
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
            host: "192.168.1.20".into(),
            port: std::num::NonZeroU16::new(18765).unwrap(),
            api_key: " key-\"with\\escapes ".into(),
        };
        for (action, params) in
            [("version", None), ("findNotes", Some(json!({ "query": "deck:Default" })))]
        {
            let request = build_request(&connection, action, params.clone()).build().unwrap();
            assert_eq!(request.url().as_str(), "http://192.168.1.20:18765/");
            let body: serde_json::Value =
                serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
            assert_eq!(body["action"], action);
            assert_eq!(body["version"], 6);
            assert_eq!(body["key"], connection.api_key);
            assert_eq!(body.get("params"), params.as_ref());
        }
    }

    #[test]
    fn requests_support_hostnames_and_ipv6() {
        for (host, expected) in [
            ("anki.local", "http://anki.local:8765/"),
            ("::1", "http://[::1]:8765/"),
            ("[2001:db8::1]", "http://[2001:db8::1]:8765/"),
        ] {
            let connection = AnkiConnectionSettings { host: host.into(), ..Default::default() };
            let request = build_request(&connection, "version", None).build().unwrap();
            assert_eq!(request.url().as_str(), expected);
        }
    }

    #[tokio::test]
    async fn localhost_reaches_an_ipv4_server_that_closes_every_connection() {
        use tokio::io::{
            AsyncBufReadExt,
            AsyncReadExt,
            AsyncWriteExt,
            BufReader,
        };
        const REQUESTS: usize = 16;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            for _ in 0..REQUESTS {
                let (stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut stream = BufReader::new(stream);
                    let mut length = 0;
                    loop {
                        let mut line = String::new();
                        stream.read_line(&mut line).await.unwrap();
                        if line == "\r\n" {
                            break;
                        }
                        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                            length = value.trim().parse().unwrap();
                        }
                    }
                    stream.read_exact(&mut vec![0; length]).await.unwrap();
                    let body = r#"{"result":6,"error":null}"#;
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                    stream.write_all(format!("{head}{body}").as_bytes()).await.unwrap();
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                });
            }
        });
        let connection = AnkiConnectionSettings {
            host: "localhost".into(),
            port: std::num::NonZeroU16::new(port).unwrap(),
            api_key: String::new(),
        };
        let responses: Vec<Response<u32>> = stream::iter(0..REQUESTS)
            .map(|_| async {
                build_request(&connection, "version", None).send().await?.json().await
            })
            .buffer_unordered(8)
            .try_collect()
            .await
            .unwrap();
        assert!(responses.iter().all(|r| r.result == Some(6)));
        tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .expect("every request must open its own connection")
            .unwrap();
    }
}

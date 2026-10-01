//! Tsunagi's own API (`/v1`), next to the AnkiConnect shim on the same port.

use std::{
    collections::{
        BTreeSet,
        HashMap,
    },
    net::{
        Ipv4Addr,
        Ipv6Addr,
        SocketAddr,
    },
    sync::LazyLock,
    time::Duration,
};

use base64::Engine;
use reqwest::{
    Client,
    Method,
    RequestBuilder,
    StatusCode,
};
use serde::{
    de::DeserializeOwned,
    Deserialize,
};
use serde_json::json;

use super::{
    client::{
        files,
        setup_problem,
        Anki,
        AnkiError,
        Attachment,
        AttachmentKind,
        CreateOutcome,
        MissingFeature,
        MissingPermissions,
        NewNote,
        NoteEvent,
        NoteInfo,
        PROBE_TIMEOUT,
    },
    connection::base_url,
    types::{
        FieldMapping,
        Model,
    },
};

pub(super) const DEFAULT_PORT: u16 = 7777;

/// Keeps its connections open, unlike AnkiConnect's client: Tsunagi's server reuses them,
/// and a new one per request can cost more than the request.
static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    let loopback =
        [SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), SocketAddr::from((Ipv6Addr::LOCALHOST, 0))];
    Client::builder()
        // Tsunagi binds IPv4 by default; trying ::1 first stalls each request on Windows.
        .resolve_to_addrs("localhost", &loopback)
        // Tsunagi's server closes a connection idle for 5 s; reusing one near that fails
        // the request as the server closes it.
        .pool_idle_timeout(Duration::from_secs(2))
        .build()
        .expect("failed to create Tsunagi HTTP client")
});

pub(super) struct Health {
    pub(super) version: String,
    pub(super) caller: Caller,
}

/// The app a request counts as: the key's, or the no-key row when the key is missing or
/// matches no app.
#[derive(Deserialize)]
pub(super) struct Caller {
    pub(super) app: String,
    pub(super) enabled: bool,
}

/// Tsunagi's health when its `v1` API answers here. It needs no profile open.
pub(super) async fn health(anki: &Anki) -> Option<Health> {
    #[derive(Deserialize)]
    struct Response {
        server: String,
        versions: Versions,
        caller: Caller,
    }
    #[derive(Deserialize)]
    struct Versions {
        api: String,
        addon: String,
    }
    let request = request(anki, Method::GET, "/v1/health").timeout(PROBE_TIMEOUT);
    let response: Response = request.send().await.ok()?.json().await.ok()?;
    (response.server == "tsunagi" && response.versions.api == "v1")
        .then_some(Health { version: response.versions.addon, caller: response.caller })
}

/// The operations Yomine uses, by what the user can't do without them.
const FEATURES: [(&str, &[&str]); 7] = [
    ("Read your cards", &["POST /v1/notes/query", "GET /v1/models"]),
    ("See changes in Anki as they happen", &["GET /v1/events"]),
    ("Save mined notes", &["POST /v1/notes", "POST /v1/media"]),
    ("Add audio and screenshots later", &["PATCH /v1/notes/{id}"]),
    ("Undo a batch", &["POST /v1/notes:delete"]),
    ("Preview asbplayer screenshots", &["GET /v1/media/{filename}"]),
    ("Open cards in Anki", &["POST /v1/gui:browse", "POST /v1/gui:select-card"]),
];

pub(super) async fn missing_permissions(anki: &Anki) -> Result<Option<MissingPermissions>, String> {
    #[derive(Deserialize)]
    struct Report {
        caller: App,
        operations: HashMap<String, Operation>,
    }
    #[derive(Deserialize)]
    struct App {
        name: String,
        role: String,
    }
    #[derive(Deserialize)]
    struct Operation {
        status: String,
        setting: Option<String>,
        reason: Option<String>,
    }
    let Ok(response) = request(anki, Method::GET, "/v1/capabilities").send().await else {
        return Ok(None);
    };
    // The report itself needs `read:collection`.
    if response.status() == StatusCode::FORBIDDEN {
        return Err(anki.redact(refusal(response).await));
    }
    let Ok(response) = success(anki, response).await else { return Ok(None) };
    let Ok(report) = response.json::<Report>().await else { return Ok(None) };
    let features: Vec<MissingFeature> = FEATURES
        .iter()
        .filter_map(|(feature, operations)| {
            let mut needs: Vec<String> = Vec::new();
            for key in *operations {
                let Some(operation) = report.operations.get(*key) else { continue };
                let need = match operation {
                    Operation { status, .. } if status == "available" => continue,
                    Operation { setting: Some(setting), .. } => {
                        setting.strip_prefix("permissions.").unwrap_or(setting).to_string()
                    }
                    Operation { reason, .. } => {
                        reason.clone().unwrap_or_else(|| "not supported here".into())
                    }
                };
                if !needs.contains(&need) {
                    needs.push(need);
                }
            }
            (!needs.is_empty())
                .then(|| MissingFeature { feature: (*feature).into(), needs: needs.join(", ") })
        })
        .collect();
    Ok((!features.is_empty()).then_some(MissingPermissions {
        app: report.caller.name,
        role: report.caller.role,
        features,
    }))
}

/// The open profile, which health reports along with the collection's state.
pub(super) async fn profile(anki: &Anki) -> Result<String, AnkiError> {
    #[derive(Deserialize)]
    struct Response {
        collection: Collection,
    }
    #[derive(Deserialize)]
    struct Collection {
        profile: Option<String>,
        state: String,
    }
    let response: Response = send(anki, request(anki, Method::GET, "/v1/health")).await?;
    let Collection { profile, state } = response.collection;
    profile.ok_or_else(|| AnkiError::Rejected(format!("No Anki profile is open ({state})")))
}

fn request(anki: &Anki, method: Method, path: &str) -> RequestBuilder {
    let mut request = CLIENT.request(method, format!("{}{path}", base_url(&anki.connection)));
    if !anki.connection.api_key.is_empty() {
        request = request.header("X-Api-Key", &anki.connection.api_key);
    }
    if let Some(timeout) = anki.timeout {
        request = request.timeout(timeout);
    }
    request
}

fn transport(anki: &Anki, error: reqwest::Error) -> AnkiError {
    let message = anki.redact(error.to_string());
    if error.is_connect() {
        AnkiError::NotSent(message)
    } else if error.is_timeout() {
        AnkiError::TimedOut(message)
    } else {
        AnkiError::Unconfirmed(message)
    }
}

async fn send<T: DeserializeOwned>(anki: &Anki, request: RequestBuilder) -> Result<T, AnkiError> {
    let response = accepted(anki, request).await?;
    response.json().await.map_err(|e| transport(anki, e))
}

/// The response, or Tsunagi's reason for refusing the request.
async fn accepted(anki: &Anki, request: RequestBuilder) -> Result<reqwest::Response, AnkiError> {
    let response = request.send().await.map_err(|e| transport(anki, e))?;
    success(anki, response).await
}

async fn success(anki: &Anki, response: reqwest::Response) -> Result<reqwest::Response, AnkiError> {
    let status = response.status();
    if !status.is_success() {
        let detail = refusal(response).await;
        return Err(AnkiError::Rejected(anki.redact(format!("{status}: {detail}"))));
    }
    Ok(response)
}

async fn refusal(response: reqwest::Response) -> String {
    #[derive(Deserialize)]
    struct Refusal {
        detail: String,
    }
    let body = response.text().await.unwrap_or_default();
    serde_json::from_str::<Refusal>(&body).map_or(body, |r| r.detail)
}

/// A query answered in full: without a `limit`, one page holds every match.
#[derive(Deserialize)]
struct Page<T> {
    items: Vec<T>,
}

async fn query<T: DeserializeOwned>(
    anki: &Anki,
    path: &str,
    body: serde_json::Value,
) -> Result<Vec<T>, AnkiError> {
    let page: Page<T> = send(anki, request(anki, Method::POST, path).json(&body)).await?;
    Ok(page.items)
}

pub(super) async fn find_notes(anki: &Anki, search: &str) -> Result<Vec<u64>, AnkiError> {
    let body = json!({ "search": search, "select": "id", "shape": "scalar" });
    query(anki, "/v1/notes/query", body).await
}

/// `search` narrows by day; a note's `mod` (epoch seconds) then makes it exact.
pub(super) async fn edited_since(
    anki: &Anki,
    search: &str,
    since: i64,
) -> Result<Vec<u64>, AnkiError> {
    let body = json!({
        "search": search,
        "where": [format!("mod >= {}", since / 1000)],
        "select": "id",
        "shape": "scalar",
    });
    query(anki, "/v1/notes/query", body).await
}

#[derive(Deserialize)]
struct Field {
    name: String,
    value: String,
}

pub(super) async fn existing(anki: &Anki, ids: &[u64]) -> Result<Vec<u64>, AnkiError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let body = json!({
        "where": [format!("id in {}", json!(ids))],
        "select": "id",
        "shape": "scalar",
    });
    let mut found: Vec<u64> = query(anki, "/v1/notes/query", body).await?;
    found.sort_unstable();
    Ok(found)
}

#[derive(Deserialize)]
struct Note {
    id: u64,
    model_name: String,
    cards: Vec<u64>,
    fields: Vec<Field>,
}

impl From<Note> for NoteInfo {
    fn from(note: Note) -> Self {
        NoteInfo {
            id: note.id,
            note_type: note.model_name,
            fields: note.fields.into_iter().map(|f| (f.name, f.value)).collect(),
            cards: note.cards,
        }
    }
}

pub(super) async fn notes(anki: &Anki, ids: &[u64]) -> Result<Vec<NoteInfo>, AnkiError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let body = json!({
        "where": [format!("id in {}", json!(ids))],
        "select": "id,model_name,cards,fields[].(name,value)",
    });
    let notes: Vec<Note> = query(anki, "/v1/notes/query", body).await?;
    Ok(notes.into_iter().map(NoteInfo::from).collect())
}

/// Only the mapped note types, and only the fields the mapping reads.
pub(super) async fn vocab_notes(
    anki: &Anki,
    ids: &[u64],
    mapping: &HashMap<String, FieldMapping>,
) -> Result<Vec<NoteInfo>, AnkiError> {
    if ids.is_empty() || mapping.is_empty() {
        return Ok(Vec::new());
    }
    let types: Vec<&String> = mapping.keys().collect();
    let fields: BTreeSet<&String> = mapping
        .values()
        .flat_map(|m| [Some(&m.term_field), Some(&m.reading_field), m.sentence_field.as_ref()])
        .flatten()
        .collect();
    let list = |values: serde_json::Value| values.to_string();
    let body = json!({
        "where": [format!("id in {}", list(json!(ids))), format!("model_name in {}", list(json!(types)))],
        "select": format!("id,model_name,cards,fields[name in {}].(name,value)", list(json!(fields))),
    });
    let notes: Vec<Note> = query(anki, "/v1/notes/query", body).await?;
    Ok(notes.into_iter().map(NoteInfo::from).collect())
}

pub(super) async fn create_note(
    anki: &Anki,
    note: &NewNote,
    key: &str,
) -> Result<CreateOutcome, AnkiError> {
    #[derive(Deserialize)]
    struct Response {
        created: Vec<Created>,
        failed: Vec<Failed>,
    }
    #[derive(Deserialize)]
    struct Created {
        id: u64,
    }
    #[derive(Deserialize)]
    struct Failed {
        code: String,
        message: String,
    }
    let body = json!({
        "deckName": note.deck,
        "modelName": note.note_type,
        "fields": note.fields,
        "tags": note.tags,
        "audio": files(&note.attachments, AttachmentKind::Audio),
        "picture": files(&note.attachments, AttachmentKind::Picture),
    });
    let request =
        request(anki, Method::POST, "/v1/notes").header("Idempotency-Key", key).json(&body);
    let response = request.send().await.map_err(|e| transport(anki, e))?;
    let status = response.status();
    let response = match success(anki, response).await {
        Ok(response) => response,
        // A 503 from an operation timeout doesn't cancel the write, and after any other
        // server error the note may exist too.
        Err(AnkiError::Rejected(reason)) if status.is_server_error() => {
            return Err(AnkiError::Unconfirmed(reason))
        }
        Err(AnkiError::Rejected(reason)) => {
            let setup = matches!(status.as_u16(), 401 | 403 | 422) || setup_problem(&reason);
            return Ok(CreateOutcome::Rejected { reason, setup });
        }
        Err(e) => return Err(e),
    };
    let response: Response = response.json().await.map_err(|e| transport(anki, e))?;
    if let Some(created) = response.created.into_iter().next() {
        return Ok(CreateOutcome::Created(created.id));
    }
    match response.failed.into_iter().next() {
        Some(failed) if failed.code == "duplicate" => Ok(CreateOutcome::Duplicate),
        Some(Failed { message, .. }) => {
            Ok(CreateOutcome::Rejected { setup: setup_problem(&message), reason: message })
        }
        None => Err(AnkiError::Unconfirmed("Tsunagi reported no result".into())),
    }
}

pub(super) async fn delete(anki: &Anki, ids: &[u64]) -> Result<(), AnkiError> {
    let request = request(anki, Method::POST, "/v1/notes:delete");
    accepted(anki, request.json(&json!({ "note_ids": ids }))).await?;
    Ok(())
}

pub(super) async fn attach(
    anki: &Anki,
    note_id: u64,
    attachments: &[Attachment],
) -> Result<(), AnkiError> {
    let body = json!({
        "audio": files(attachments, AttachmentKind::Audio),
        "picture": files(attachments, AttachmentKind::Picture),
    });
    let request = request(anki, Method::PATCH, &format!("/v1/notes/{note_id}"));
    accepted(anki, request.json(&body)).await?;
    Ok(())
}

pub(super) async fn store_media(
    anki: &Anki,
    filename: &str,
    base64_data: &str,
) -> Result<String, AnkiError> {
    #[derive(Deserialize)]
    struct Response {
        created: Vec<Stored>,
        failed: Vec<Failed>,
    }
    #[derive(Deserialize)]
    struct Stored {
        filename: String,
    }
    #[derive(Deserialize)]
    struct Failed {
        message: String,
    }
    let body = json!({ "filename": filename, "data": base64_data });
    let response: Response =
        send(anki, request(anki, Method::POST, "/v1/media").json(&body)).await?;
    if let Some(failed) = response.failed.into_iter().next() {
        return Err(AnkiError::Rejected(failed.message));
    }
    let stored = response.created.into_iter().next();
    stored
        .map(|s| s.filename)
        .ok_or_else(|| AnkiError::Unconfirmed("Tsunagi stored no file".into()))
}

pub(super) async fn media(anki: &Anki, filename: &str) -> Result<Option<String>, AnkiError> {
    let mut url = reqwest::Url::parse("http://localhost/v1/media").expect("valid URL");
    url.path_segments_mut().expect("base URL").push(filename);
    let response =
        request(anki, Method::GET, url.path()).send().await.map_err(|e| transport(anki, e))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let bytes = success(anki, response).await?.bytes().await.map_err(|e| transport(anki, e))?;
    Ok(Some(base64::engine::general_purpose::STANDARD.encode(bytes)))
}

pub(super) async fn browse(anki: &Anki, query: &str) -> Result<(), AnkiError> {
    let request = request(anki, Method::POST, "/v1/gui:browse");
    accepted(anki, request.json(&json!({ "query": query }))).await?;
    Ok(())
}

pub(super) async fn select_card(anki: &Anki, card_id: u64) -> Result<(), AnkiError> {
    let request = request(anki, Method::POST, "/v1/gui:select-card");
    accepted(anki, request.json(&json!({ "card_id": card_id }))).await?;
    Ok(())
}

pub(super) async fn note_types(anki: &Anki) -> Result<Vec<Model>, AnkiError> {
    #[derive(Deserialize)]
    struct NoteType {
        id: u64,
        name: String,
        fields: Vec<String>,
    }
    let request =
        request(anki, Method::GET, "/v1/models").query(&[("select", "id,name,fields[].name")]);
    let page: Page<NoteType> = send(anki, request).await?;
    Ok(page
        .items
        .into_iter()
        .map(|t| Model { name: t.name, id: t.id, fields: t.fields, sample_note: None })
        .collect())
}

/// Without a heartbeat interval from Tsunagi, three of its 15-second pings.
const DEFAULT_IDLE_LIMIT: Duration = Duration::from_secs(45);

/// Follows note changes until the stream ends, handing each one to `on`, and returns the
/// reason Tsunagi gave for closing it. A stream silent for three heartbeats is taken as
/// dead, since a killed Anki can leave it open.
pub(super) async fn follow_notes(
    anki: &Anki,
    mut on: impl FnMut(NoteEvent),
) -> Result<Option<String>, AnkiError> {
    // Only notes: card events would keep intervals current, which mining doesn't need.
    let request = request(anki, Method::GET, "/v1/events")
        .query(&[("resources", "notes")])
        .header(reqwest::header::ACCEPT, "text/event-stream");
    let mut response = accepted(anki, request).await?;
    let mut stream = EventStream::default();
    let mut idle_limit = DEFAULT_IDLE_LIMIT;
    loop {
        let chunk = tokio::time::timeout(idle_limit, response.chunk())
            .await
            .map_err(|_| AnkiError::TimedOut("Tsunagi's event stream went silent".into()))?
            .map_err(|e| transport(anki, e))?;
        let Some(chunk) = chunk else { return Ok(None) };
        for message in stream.feed(&chunk) {
            let Ok(data) = serde_json::from_str::<Data>(&message.data) else { continue };
            if message.event == "close" {
                return Ok(data.reason);
            }
            if let Some(heartbeat) = data.heartbeat_ms {
                idle_limit = Duration::from_millis(heartbeat) * 3;
            }
            if let Some(event) = note_event(&message.event, data.ids) {
                on(event);
            }
        }
    }
}

#[derive(Deserialize)]
struct Data {
    #[serde(default)]
    ids: Vec<u64>,
    /// On `ready`, how often Tsunagi sends a heartbeat when nothing changes.
    #[serde(default)]
    heartbeat_ms: Option<u64>,
    /// On `close`: `profile_closed`, `auth`, `shutdown`, `timeout` or `max_events`.
    #[serde(default)]
    reason: Option<String>,
}

fn note_event(event: &str, ids: Vec<u64>) -> Option<NoteEvent> {
    Some(match event {
        "ready" => NoteEvent::Ready,
        "notes.created" | "notes.updated" => NoteEvent::Changed(ids),
        "notes.deleted" => NoteEvent::Deleted(ids),
        "notes.stale" | "gap" => NoteEvent::Stale,
        _ => return None,
    })
}

struct Message {
    event: String,
    data: String,
}

/// Server-sent events framed as the HTML standard defines them: a line ends at CRLF, LF or
/// CR, a blank line ends a message, `data:` lines join with newlines, and lines starting
/// with `:` are comments.
#[derive(Default)]
struct EventStream {
    line: Vec<u8>,
    after_cr: bool,
    event: String,
    data: Vec<String>,
}

impl EventStream {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Message> {
        let mut messages = Vec::new();
        for &byte in bytes {
            let after_cr = std::mem::take(&mut self.after_cr);
            match byte {
                b'\n' if after_cr => {}
                b'\r' | b'\n' => {
                    self.after_cr = byte == b'\r';
                    let line =
                        String::from_utf8_lossy(&std::mem::take(&mut self.line)).into_owned();
                    messages.extend(self.end_line(&line));
                }
                _ => self.line.push(byte),
            }
        }
        messages
    }

    fn end_line(&mut self, line: &str) -> Option<Message> {
        if line.is_empty() {
            let event = std::mem::take(&mut self.event);
            let data = std::mem::take(&mut self.data);
            return (!data.is_empty()).then(|| Message {
                event: if event.is_empty() { "message".into() } else { event },
                data: data.join("\n"),
            });
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = value.to_string(),
            "data" => self.data.push(value.to_string()),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(chunks: &[&[u8]]) -> Vec<(String, String)> {
        let mut stream = EventStream::default();
        chunks.iter().flat_map(|c| stream.feed(c)).map(|m| (m.event, m.data)).collect()
    }

    /// Answers one request with `status` and a JSON `body`.
    async fn server(status: &'static str, body: &'static str) -> Anki {
        use tokio::io::{
            AsyncBufReadExt,
            AsyncReadExt,
            AsyncWriteExt,
            BufReader,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.2:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
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
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        super::super::probe(crate::core::settings::AnkiConnectionSettings {
            host: "127.0.0.2".into(),
            port: std::num::NonZeroU16::new(port).unwrap(),
            api_key: String::new(),
        })
    }

    #[tokio::test]
    async fn a_create_that_may_have_run_is_not_reported_as_rejected() {
        let note = NewNote {
            deck: "Mining".into(),
            note_type: "Basic".into(),
            fields: HashMap::new(),
            tags: Vec::new(),
            attachments: Vec::new(),
        };
        let anki = server("503 Service Unavailable", r#"{"detail":"Anki took too long"}"#).await;
        assert!(matches!(create_note(&anki, &note, "k").await, Err(AnkiError::Unconfirmed(_))));
        let duplicate = r#"{"created":[],"failed":[{"index":0,"code":"duplicate","message":"Note duplicates an existing note"}]}"#;
        let anki = server("200 OK", duplicate).await;
        assert!(matches!(create_note(&anki, &note, "k").await, Ok(CreateOutcome::Duplicate)));
    }

    #[test]
    fn messages_are_framed_as_the_standard_defines() {
        let expected = vec![("notes.updated".to_string(), "{\"ids\":[1]}".to_string())];
        for ending in ["\n", "\r\n", "\r"] {
            let text = format!(": ping{ending}{ending}event: notes.updated{ending}data: {{\"ids\":[1]}}{ending}{ending}");
            assert_eq!(feed(&[text.as_bytes()]), expected, "{ending:?}");
        }
        // A CRLF split across chunks is one line ending, not two.
        assert_eq!(feed(&[b"event: notes.updated\r", b"\ndata: {\"ids\":[1]}\r\n\r\n"]), expected);
        assert_eq!(
            feed(&[b"data:a\ndata:  b\n\n"]),
            [("message".to_string(), "a\n b".to_string())]
        );
        // A character split across chunks decodes once its line is complete.
        let bytes = "data: 漢\n\n".as_bytes();
        assert_eq!(feed(&[&bytes[..7], &bytes[7..]]), [("message".to_string(), "漢".to_string())]);
    }
}

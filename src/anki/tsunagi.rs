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

use reqwest::{
    Client,
    Method,
    RequestBuilder,
};
use serde::{
    de::DeserializeOwned,
    Deserialize,
};
use serde_json::json;

use super::{
    client::{
        Anki,
        AnkiError,
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
    /// Who Tsunagi took the request to be; `None` from a Tsunagi too old to say.
    pub(super) caller: Option<Caller>,
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
        #[serde(default)]
        caller: Option<Caller>,
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
    let status = response.status();
    if !status.is_success() {
        #[derive(Deserialize)]
        struct Refusal {
            detail: String,
        }
        let body = response.text().await.unwrap_or_default();
        let detail = serde_json::from_str::<Refusal>(&body).map_or(body, |r| r.detail);
        return Err(AnkiError::Rejected(anki.redact(format!("{status}: {detail}"))));
    }
    Ok(response)
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

#[derive(Deserialize)]
struct VocabNote {
    id: u64,
    model_name: String,
    cards: Vec<u64>,
    fields: Vec<Field>,
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
    let notes: Vec<VocabNote> = query(anki, "/v1/notes/query", body).await?;
    Ok(notes
        .into_iter()
        .map(|note| NoteInfo {
            id: note.id,
            note_type: note.model_name,
            fields: note.fields.into_iter().map(|f| (f.name, f.value)).collect(),
            cards: note.cards,
        })
        .collect())
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

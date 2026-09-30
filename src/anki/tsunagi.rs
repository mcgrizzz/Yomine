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

async fn send<T: DeserializeOwned>(anki: &Anki, request: RequestBuilder) -> Result<T, AnkiError> {
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
    let response = request.send().await.map_err(transport)?;
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
    response.json().await.map_err(transport)
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

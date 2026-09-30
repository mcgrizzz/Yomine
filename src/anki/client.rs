//! Yomine's Anki operations, in Yomine's types, whichever add-on answers.

use std::{
    collections::HashMap,
    time::Duration,
};

use serde::Serialize;

use super::{
    ankiconnect,
    connection,
    tsunagi,
    types::Model,
};
use crate::core::settings::AnkiConnectionSettings;

pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Backend {
    AnkiConnect,
    Tsunagi { version: String },
}

#[derive(Debug, thiserror::Error)]
pub enum AnkiError {
    /// Anki wasn't reached, so nothing happened.
    #[error("{0}")]
    NotSent(String),
    /// No answer in time; a write may still have happened.
    #[error("{0}")]
    TimedOut(String),
    /// Anki answered, but not with anything usable; a write may have happened.
    #[error("{0}")]
    Unconfirmed(String),
    #[error("{0}")]
    Rejected(String),
}

pub struct NoteInfo {
    pub id: u64,
    pub note_type: String,
    pub fields: HashMap<String, String>,
    pub cards: Vec<u64>,
}

pub struct NewNote<'a> {
    pub deck: &'a str,
    pub note_type: &'a str,
    pub fields: &'a HashMap<String, String>,
    pub tags: &'a [String],
}

pub enum CreateOutcome {
    Created(u64),
    /// Anki's duplicate check: a note of the same type with the same first field.
    Duplicate,
    /// `setup` when the cause is shared by every note (deck, note type, key, permission).
    Rejected {
        reason: String,
        setup: bool,
    },
}

/// A connection to Anki, fixed for as long as the handle is kept.
pub struct Anki {
    pub(super) connection: AnkiConnectionSettings,
    pub(super) timeout: Option<Duration>,
}

/// The configured connection.
pub fn current() -> Anki {
    Anki { connection: connection::active(), timeout: None }
}

/// A connection being set up, which doesn't replace the configured one.
pub fn probe(connection: AnkiConnectionSettings) -> Anki {
    Anki { connection, timeout: Some(PROBE_TIMEOUT) }
}

/// Whether the configured Anki answers. A failure makes the next harvests full, since
/// anything may have changed meanwhile.
pub async fn reachable() -> bool {
    let reached = current().version().await.is_ok();
    if !reached {
        super::state::anki_unreachable();
    }
    reached
}

impl Anki {
    pub(super) fn redact(&self, message: String) -> String {
        if self.connection.api_key.is_empty() {
            message
        } else {
            message.replace(&self.connection.api_key, "[redacted]")
        }
    }

    pub async fn version(&self) -> Result<u32, AnkiError> {
        ankiconnect::version(self).await
    }

    /// Both add-ons answer AnkiConnect's protocol, which also checks the key; Tsunagi's
    /// health endpoint then tells them apart.
    pub async fn detect(&self) -> Result<Backend, AnkiError> {
        self.version().await?;
        Ok(match tsunagi::version(self).await {
            Some(version) => Backend::Tsunagi { version },
            None => Backend::AnkiConnect,
        })
    }

    /// The open profile's name, which keys Yomine's copy of its collection.
    pub async fn profile(&self) -> Result<String, AnkiError> {
        ankiconnect::active_profile(self).await
    }

    pub async fn note_types(&self) -> Result<Vec<Model>, AnkiError> {
        ankiconnect::note_types(self).await
    }

    pub async fn sample_note(
        &self,
        note_type: &str,
    ) -> Result<Option<HashMap<String, String>>, AnkiError> {
        ankiconnect::sample_note(self, note_type).await
    }

    /// Notes matching an Anki search.
    pub(crate) async fn find_notes(&self, query: &str) -> Result<Vec<u64>, AnkiError> {
        ankiconnect::find_notes(self, query).await
    }

    pub async fn notes(&self, ids: &[u64]) -> Result<Vec<NoteInfo>, AnkiError> {
        ankiconnect::notes(self, ids).await
    }

    /// In days, or negative seconds while learning; in the order of `card_ids`.
    pub(crate) async fn intervals(&self, card_ids: &[u64]) -> Result<Vec<i32>, AnkiError> {
        ankiconnect::intervals(self, card_ids).await
    }

    /// Those of `ids` still in Anki; an error rather than a guess when Anki can't say.
    pub async fn existing(&self, ids: &[u64]) -> Result<Vec<u64>, AnkiError> {
        ankiconnect::existing(self, ids).await
    }

    pub async fn delete(&self, ids: &[u64]) -> Result<(), AnkiError> {
        ankiconnect::delete(self, ids).await
    }

    pub async fn create_note(&self, note: &NewNote<'_>) -> Result<CreateOutcome, AnkiError> {
        ankiconnect::create_note(self, note).await
    }

    /// Replaces only the named fields.
    pub async fn update_fields(
        &self,
        note_id: u64,
        fields: &HashMap<String, String>,
    ) -> Result<(), AnkiError> {
        ankiconnect::update_fields(self, note_id, fields).await
    }

    /// Stores a base64 file and returns the name Anki stored it under.
    pub async fn store_media(
        &self,
        filename: &str,
        base64_data: &str,
    ) -> Result<String, AnkiError> {
        ankiconnect::store_media(self, filename, base64_data).await
    }

    /// A media file as base64; `None` when Anki doesn't have it.
    pub async fn media(&self, filename: &str) -> Result<Option<String>, AnkiError> {
        ankiconnect::media(self, filename).await
    }

    /// Opens Anki's browser on recent adds with the note's card selected.
    pub async fn open_note(&self, note_id: u64) -> Result<(), AnkiError> {
        ankiconnect::open_note(self, note_id).await
    }

    pub async fn browse_notes(&self, ids: &[u64]) -> Result<(), AnkiError> {
        let ids = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
        ankiconnect::browse(self, &format!("nid:{ids}")).await
    }
}

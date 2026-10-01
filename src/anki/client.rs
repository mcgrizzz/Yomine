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
    types::{
        FieldMapping,
        Model,
    },
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

/// A message from Tsunagi's event stream about notes.
pub enum NoteEvent {
    /// Connected; changes made before it may have been missed.
    Ready,
    Changed(Vec<u64>),
    Deleted(Vec<u64>),
    /// Notes changed that Tsunagi can't name (an undo, a sync), or messages were dropped.
    Stale,
}

pub struct NoteInfo {
    pub id: u64,
    pub note_type: String,
    pub fields: HashMap<String, String>,
    pub cards: Vec<u64>,
}

pub struct NewNote {
    pub deck: String,
    pub note_type: String,
    pub fields: HashMap<String, String>,
    pub tags: Vec<String>,
    pub attachments: Vec<Attachment>,
}

/// Those of `attachments` of one kind, which the add-ons take in separate lists.
pub(super) fn files(attachments: &[Attachment], kind: AttachmentKind) -> Vec<&Attachment> {
    attachments.iter().filter(|a| a.kind == kind).collect()
}

/// A file stored with the note, its reference appended to `fields`.
#[derive(Serialize)]
pub struct Attachment {
    #[serde(skip)]
    pub kind: AttachmentKind,
    pub filename: String,
    /// Base64.
    pub data: String,
    pub fields: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AttachmentKind {
    Audio,
    Picture,
}

/// A new idempotency key for one create request.
pub fn request_key() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Whether a rejection would fail every note, not only this one.
pub(super) fn setup_problem(reason: &str) -> bool {
    let lower = reason.to_lowercase();
    ["deck", "model", "note type", "api key", "permission"].iter().any(|s| lower.contains(s))
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

/// What Tsunagi doesn't let this app do, as its settings name them.
#[derive(Serialize)]
pub struct MissingPermissions {
    pub app: String,
    pub role: String,
    pub features: Vec<MissingFeature>,
}

#[derive(Serialize)]
pub struct MissingFeature {
    pub feature: String,
    /// The permissions it needs, or what else stands in the way.
    pub needs: String,
}

/// A connection to Anki, fixed for as long as the handle is kept.
pub struct Anki {
    pub(super) connection: AnkiConnectionSettings,
    pub(super) timeout: Option<Duration>,
    /// Anything not detected as Tsunagi talks AnkiConnect, which Tsunagi's shim answers too.
    pub(super) backend: Backend,
}

/// The configured connection, as the add-on it last answered as.
pub fn current() -> Anki {
    Anki {
        connection: connection::active(),
        timeout: None,
        backend: connection::backend().unwrap_or(Backend::AnkiConnect),
    }
}

/// A connection being set up, which doesn't replace the configured one.
pub fn probe(connection: AnkiConnectionSettings) -> Anki {
    Anki { connection, timeout: Some(PROBE_TIMEOUT), backend: Backend::AnkiConnect }
}

/// Whether the configured Anki answers, noting which add-on did. A failure makes the next
/// harvests full, since anything may have changed meanwhile.
pub async fn reachable() -> bool {
    let anki = current();
    let detected = anki.detect().await.ok();
    connection::set_backend(&anki.connection, detected.clone());
    if detected.is_none() {
        super::sync::anki_unreachable();
    }
    detected.is_some()
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

    /// Tsunagi's health endpoint names it and the app the request counts as; anything else
    /// is checked through AnkiConnect's protocol. What the app may do shows on the requests
    /// themselves.
    pub async fn detect(&self) -> Result<Backend, AnkiError> {
        let Some(health) = tsunagi::health(self).await else {
            self.version().await?;
            return Ok(Backend::AnkiConnect);
        };
        if !health.caller.enabled {
            let message =
                format!("The app \"{}\" is turned off in Tsunagi's settings", health.caller.app);
            return Err(AnkiError::Rejected(message));
        }
        Ok(Backend::Tsunagi { version: health.version })
    }

    /// Uses Tsunagi's own API from here on if Tsunagi answers.
    pub async fn detected(mut self) -> Self {
        if let Some(health) = tsunagi::health(&self).await {
            self.backend = Backend::Tsunagi { version: health.version };
        }
        self
    }

    fn tsunagi(&self) -> bool {
        matches!(self.backend, Backend::Tsunagi { .. })
    }

    /// Whether this handle is still the configured connection, answering as Tsunagi.
    pub fn is_configured_tsunagi(&self) -> bool {
        self.tsunagi()
            && self.connection == connection::active()
            && connection::backend().as_ref() == Some(&self.backend)
    }

    /// Follows note changes until the stream ends, returning the reason Tsunagi gave for
    /// closing it; only Tsunagi has one.
    pub async fn follow_notes(
        &self,
        on: impl FnMut(NoteEvent),
    ) -> Result<Option<String>, AnkiError> {
        if !self.tsunagi() {
            return Err(AnkiError::Rejected("Only Tsunagi announces changes".into()));
        }
        tsunagi::follow_notes(self, on).await
    }

    /// Yomine's features Tsunagi doesn't allow this app: `None` when all are allowed, or
    /// when Tsunagi can't report because no profile is open; `Err` with Tsunagi's reason
    /// when it won't report to this app.
    pub async fn missing_permissions(&self) -> Result<Option<MissingPermissions>, String> {
        tsunagi::missing_permissions(self).await
    }

    /// The open profile's name, which keys Yomine's copy of its collection.
    pub async fn profile(&self) -> Result<String, AnkiError> {
        if self.tsunagi() {
            tsunagi::profile(self).await
        } else {
            ankiconnect::active_profile(self).await
        }
    }

    pub async fn note_types(&self) -> Result<Vec<Model>, AnkiError> {
        if self.tsunagi() {
            tsunagi::note_types(self).await
        } else {
            ankiconnect::note_types(self).await
        }
    }

    pub async fn sample_note(
        &self,
        note_type: &str,
    ) -> Result<Option<HashMap<String, String>>, AnkiError> {
        let query = format!("note:\"{}\"", note_type.replace('"', "\\\""));
        let ids = self.find_notes(&query).await?;
        let Some(id) = ids.get(ids.len() / 2) else { return Ok(None) };
        Ok(self.notes(&[*id]).await?.into_iter().next().map(|note| note.fields))
    }

    /// Notes matching an Anki search.
    pub(crate) async fn find_notes(&self, query: &str) -> Result<Vec<u64>, AnkiError> {
        if self.tsunagi() {
            tsunagi::find_notes(self, query).await
        } else {
            ankiconnect::find_notes(self, query).await
        }
    }

    /// Notes edited since `since` (epoch ms); on AnkiConnect, also the others edited that
    /// day.
    pub(crate) async fn edited_since(&self, since: i64, now: i64) -> Result<Vec<u64>, AnkiError> {
        // edited:N counts back from the start of today, so one extra day covers `since`
        // late yesterday.
        let search = format!("edited:{}", (now - since) / 86_400_000 + 1);
        if self.tsunagi() {
            tsunagi::edited_since(self, &search, since).await
        } else {
            ankiconnect::find_notes(self, &search).await
        }
    }

    /// Those of `ids` whose note type is mapped, with the mapped fields at least.
    pub(crate) async fn vocab_notes(
        &self,
        ids: &[u64],
        mapping: &HashMap<String, FieldMapping>,
    ) -> Result<Vec<NoteInfo>, AnkiError> {
        if self.tsunagi() {
            tsunagi::vocab_notes(self, ids, mapping).await
        } else {
            ankiconnect::notes(self, ids).await
        }
    }

    pub async fn notes(&self, ids: &[u64]) -> Result<Vec<NoteInfo>, AnkiError> {
        if self.tsunagi() {
            tsunagi::notes(self, ids).await
        } else {
            ankiconnect::notes(self, ids).await
        }
    }

    /// Each card's latest interval: in days, or negative seconds while learning, and 0 for a
    /// new card; in the order of `card_ids`.
    pub(crate) async fn intervals(&self, card_ids: &[u64]) -> Result<Vec<i32>, AnkiError> {
        if self.tsunagi() {
            tsunagi::intervals(self, card_ids).await
        } else {
            ankiconnect::intervals(self, card_ids).await
        }
    }

    /// Those of `ids` still in Anki; an error rather than a guess when Anki can't say.
    pub async fn existing(&self, ids: &[u64]) -> Result<Vec<u64>, AnkiError> {
        if self.tsunagi() {
            tsunagi::existing(self, ids).await
        } else {
            ankiconnect::existing(self, ids).await
        }
    }

    pub async fn delete(&self, ids: &[u64]) -> Result<(), AnkiError> {
        if self.tsunagi() {
            tsunagi::delete(self, ids).await
        } else {
            ankiconnect::delete(self, ids).await
        }
    }

    pub async fn create_note(&self, note: &NewNote, key: &str) -> Result<CreateOutcome, AnkiError> {
        if self.tsunagi() {
            tsunagi::create_note(self, note, key).await
        } else {
            ankiconnect::create_note(self, note).await
        }
    }

    /// Whether a create sent again with its key returns the first attempt's result instead
    /// of adding the note twice.
    pub fn can_resend_create(&self) -> bool {
        self.tsunagi()
    }

    /// Stores the files and appends each reference to its fields.
    pub async fn attach(&self, note_id: u64, attachments: &[Attachment]) -> Result<(), AnkiError> {
        if self.tsunagi() {
            tsunagi::attach(self, note_id, attachments).await
        } else {
            ankiconnect::attach(self, note_id, attachments).await
        }
    }

    /// Stores a base64 file and returns the name Anki stored it under.
    pub async fn store_media(
        &self,
        filename: &str,
        base64_data: &str,
    ) -> Result<String, AnkiError> {
        if self.tsunagi() {
            tsunagi::store_media(self, filename, base64_data).await
        } else {
            ankiconnect::store_media(self, filename, base64_data).await
        }
    }

    /// A media file as base64; `None` when Anki doesn't have it.
    pub async fn media(&self, filename: &str) -> Result<Option<String>, AnkiError> {
        if self.tsunagi() {
            tsunagi::media(self, filename).await
        } else {
            ankiconnect::media(self, filename).await
        }
    }

    /// Opens Anki's browser on recent adds with the note's card selected.
    pub async fn open_note(&self, note_id: u64) -> Result<(), AnkiError> {
        self.browse(&format!("added:1 OR nid:{note_id}")).await?;
        if let Ok(notes) = self.notes(&[note_id]).await {
            if let Some(&card) = notes.first().and_then(|n| n.cards.first()) {
                let _ = self.select_card(card).await;
            }
        }
        Ok(())
    }

    pub async fn browse_notes(&self, ids: &[u64]) -> Result<(), AnkiError> {
        let ids = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
        self.browse(&format!("nid:{ids}")).await
    }

    async fn browse(&self, query: &str) -> Result<(), AnkiError> {
        if self.tsunagi() {
            tsunagi::browse(self, query).await
        } else {
            ankiconnect::browse(self, query).await
        }
    }

    async fn select_card(&self, card_id: u64) -> Result<(), AnkiError> {
        if self.tsunagi() {
            tsunagi::select_card(self, card_id).await
        } else {
            ankiconnect::select_card(self, card_id).await
        }
    }
}

//! Global event names + payload structs (contracts/events.md). These push
//! ambient state changes the background task detects (research R5), replacing
//! egui's per-frame `poll_results()`. Per-operation progress uses
//! `tauri::ipc::Channel<LoadingMessage>` / `Channel<AnalysisProgressDto>` instead.

use serde::{
    Deserialize,
    Serialize,
};

/// Event-name constants. Frontend `listen`s to these; backend `app.emit`s them.
pub mod names {
    pub const LANGUAGE_TOOLS_STATUS: &str = "language-tools-status";
    pub const ANKI_STATUS: &str = "anki-status";
    pub const PLAYER_STATUS: &str = "player-status";
    pub const TERMS_REFRESHED: &str = "terms-refreshed";
    pub const DICTIONARIES_CHANGED: &str = "dictionaries-changed";
    pub const KNOWLEDGE_SUMMARY: &str = "knowledge-summary";
    pub const ANALYSIS_COMPLETE: &str = "analysis-complete";
    pub const ANALYSIS_CANCELLED: &str = "analysis-cancelled";
    pub const EXPORT_COMPLETE: &str = "export-complete";
    pub const ERROR: &str = "error";
}

/// Progress payload for long-running commands. A `null` message clears the
/// overlay (mirrors `MessageOverlay::clear_message`).
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LoadingMessage {
    pub message: Option<String>,
}

impl LoadingMessage {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: Some(message.into()) }
    }

    pub fn clear() -> Self {
        Self { message: None }
    }
}

/// Tools load lifecycle. Serializes to `"loading"` / `"ready"` / `{ "error": "…" }`
/// (externally-tagged + lowercase variant names match the TS union in data-model.md).
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum LanguageToolsStatus {
    Loading,
    Ready,
    Error(String),
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct AnkiStatus {
    pub connected: bool,
    pub fetching: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct PlayerStatus {
    pub mpv_connected: bool,
    pub ws_clients: usize,
    /// `"mpv"` | `"asbplayer"` | `"none"`.
    pub mode: String,
}

/// `export-complete` payload.
#[derive(Serialize, Deserialize, Clone)]
pub struct ExportComplete {
    pub ok: bool,
    pub message: String,
}

/// `error` payload — a failure the UI should surface as a modal (was `ErrorModal`).
#[derive(Serialize, Deserialize, Clone)]
pub struct ErrorPayload {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
}

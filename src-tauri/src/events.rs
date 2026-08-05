//! Global event names + payload structs (contracts/events.md). Ambient state
//! changes only — per-operation progress uses a `tauri::ipc::Channel` instead.

use serde::{
    Deserialize,
    Serialize,
};

/// Event-name constants. Frontend `listen`s to these; backend `app.emit`s them.
pub mod names {
    pub const LANGUAGE_TOOLS_STATUS: &str = "language-tools-status";
    pub const ANKI_STATUS: &str = "anki-status";
    pub const YOMITAN_STATUS: &str = "yomitan-status";
    pub const PLAYER_STATUS: &str = "player-status";
    pub const TERMS_REFRESHED: &str = "terms-refreshed";
    pub const DICTIONARIES_CHANGED: &str = "dictionaries-changed";
    pub const KNOWLEDGE_SUMMARY: &str = "knowledge-summary";
    pub const ASBPLAYER_MEDIA_LOADED: &str = "asbplayer-media-loaded";
    pub const ASBPLAYER_CONTEXT: &str = "asbplayer-context";
    pub const ANALYSIS_COMPLETE: &str = "analysis-complete";
    pub const ANALYSIS_CANCELLED: &str = "analysis-cancelled";
    pub const EXPORT_COMPLETE: &str = "export-complete";
    pub const SETTINGS_CHANGED: &str = "settings-changed";
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
    /// WebSocket server state: `"running"` | `"starting"` | `"error"` | `"stopped"`.
    /// Lets the asbplayer dot distinguish a bind failure from "waiting".
    pub server_state: String,
    /// Error message when `server_state == "error"` (e.g. a bind failure), else `None`.
    pub server_error: Option<String>,
    /// Start-seconds of player-acknowledged seeks (drives the 👁 button state).
    /// Stable insertion order, so `PartialEq` change-detection fires exactly on
    /// new confirmations.
    pub confirmed_timestamps: Vec<f32>,
}

/// asbplayer active-tab awareness — mining/seek target the active tab.
#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct AsbplayerContext {
    pub has_active_tab: bool,
    pub active_title: Option<String>,
    pub active_has_subtitles: bool,
    pub loaded_is_active: bool,
    pub loaded_from_asbplayer: bool,
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

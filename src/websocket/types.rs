use serde::{
    Deserialize,
    Serialize,
};
use time::Time;
use tokio::sync::mpsc;

use crate::core::models::{
    Sentence,
    TimeStamp,
};

#[derive(Clone, Debug)]
pub enum ServerState {
    Running,
    Stopped,
    Error(String),
    Starting,
}

impl Default for ServerState {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Debug, Clone)]
pub enum ServerCommand {
    SendToClients { json: String, clients: Vec<mpsc::Sender<String>> },
    /// An inbound `"response"` message: routed to a pending request by
    /// `messageId`, else treated as a seek confirmation (the original protocol).
    ProcessResponse { message_id: String, body: Option<serde_json::Value> },
    Shutdown,
}

#[derive(Debug, Serialize)]
pub(crate) struct SeekCommand {
    pub command: String,
    #[serde(rename = "messageId")]
    pub message_id: String,
    pub body: SeekBody,
}

#[derive(Debug, Serialize)]
pub(crate) struct SeekBody {
    pub timestamp: f32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommandResponse {
    pub command: String,
    #[serde(rename = "messageId")]
    pub message_id: String,
    /// Present for request/response commands (get-bound-media, get-subtitles);
    /// absent for bare seek confirmations.
    #[serde(default)]
    pub body: Option<serde_json::Value>,
}

/// Outbound request in asbplayer's external-API shape (docs/reference/external-api.md).
#[derive(Debug, Serialize)]
pub(crate) struct RequestCommand {
    pub command: String,
    #[serde(rename = "messageId")]
    pub message_id: String,
    pub body: serde_json::Value,
}

/// One subtitle track loaded for a bound media (`get-bound-media`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub track_number: u32,
    pub file_name: String,
}

/// Media asbplayer is currently tracking (`get-bound-media`, extension v1.20+).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundMedia {
    pub id: String,
    /// `"streaming"` | `"local"`.
    #[serde(rename = "type")]
    pub media_type: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub favicon_url: Option<String>,
    /// Empty when no subtitles are loaded for the media.
    #[serde(default)]
    pub loaded_subtitles: Vec<SubtitleTrack>,
    /// Whether the media's tab is the active tab of its window.
    #[serde(default)]
    pub active: bool,
}

/// One subtitle cue from `get-subtitles`. `start`/`end` are milliseconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSubtitle {
    pub text: String,
    pub start: u64,
    pub end: u64,
    #[serde(default)]
    pub track: u32,
}

impl RemoteSubtitle {
    /// Convert a cue into a pipeline `Sentence` — the same text cleaning as the
    /// SRT parser, with the cue timing as the seekable timestamp. `None` when
    /// nothing minable is left after cleanup.
    pub fn to_sentence(&self, id: usize, source_id: u32) -> Option<Sentence> {
        let text = crate::parser::clean_subtitle_text(&self.text);
        if text.is_empty() {
            return None;
        }
        Some(Sentence {
            id,
            source_id,
            text,
            segments: Vec::new(),
            timestamp: Some(TimeStamp {
                start: ms_to_time(self.start),
                end: ms_to_time(self.end),
            }),
            comprehension: 0.0,
        })
    }
}

/// Serialize cues as an `.srt` document (raw cue text — the parsers do their own
/// cleaning on read, matching what the live pipeline applied). Lets an asbplayer
/// session be saved to disk and reopened like any subtitle file (issue #105).
pub fn subtitles_to_srt(subtitles: &[RemoteSubtitle]) -> String {
    let mut out = String::new();
    for (i, cue) in subtitles.iter().enumerate() {
        let ts = |ms: u64| {
            format!(
                "{:02}:{:02}:{:02},{:03}",
                ms / 3_600_000,
                (ms / 60_000) % 60,
                (ms / 1000) % 60,
                ms % 1000
            )
        };
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            ts(cue.start),
            ts(cue.end),
            cue.text.trim()
        ));
    }
    out
}

/// Milliseconds-from-start → `time::Time` (wraps at 24h, like subtitle formats).
fn ms_to_time(ms: u64) -> Time {
    let h = ((ms / 3_600_000) % 24) as u8;
    let m = ((ms / 60_000) % 60) as u8;
    let s = ((ms / 1000) % 60) as u8;
    let milli = (ms % 1000) as u16;
    Time::from_hms_milli(h, m, s, milli).unwrap_or(Time::MIDNIGHT)
}

#[derive(Clone, Debug)]
pub struct SeekStatus {
    pub message_id: String,
    pub timestamp: f32,
    pub timestamp_str: String, // Original timestamp string for display
    pub confirmed: bool,
    pub sent_time: std::time::Instant,
}

#[derive(Clone)]
pub struct ConnectedClient {
    pub tx: mpsc::Sender<String>,
}

impl ConnectedClient {
    pub fn is_valid(&self) -> bool {
        !self.tx.is_closed() && self.tx.capacity() > 0
    }
}

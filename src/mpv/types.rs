use std::time::Instant;

use serde::{
    Deserialize,
    Serialize,
};

pub const MPV_SOCKET_TIMEOUT_MS: u64 = 800;
pub const MPV_REQUEST_TIMEOUT_SECS: u64 = 5;
pub const MPV_DETECTION_INTERVAL_MS: u64 = 1000;
pub const MPV_BUFFER_SIZE: usize = 2048;

pub const fn default_mpv_endpoint() -> &'static str {
    #[cfg(windows)]
    return r"\\.\pipe\tmp\mpv-socket";
    #[cfg(not(windows))]
    return "/tmp/mpv-socket";
}

#[derive(Debug, Serialize)]
pub struct MpvCommand {
    pub command: Vec<serde_json::Value>,
    pub request_id: u32,
}

#[derive(Debug, Deserialize)]
pub struct MpvResponse {
    pub error: String,
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub request_id: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct PendingRequest {
    pub request_id: u32,
    pub timestamp_str: String,
    pub sent_time: Instant,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connected,
}

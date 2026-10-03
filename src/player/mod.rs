use std::sync::Arc;

use crate::{
    core::{
        errors::YomineError,
        settings::MiningMode,
    },
    mpv::MpvManager,
    websocket::{
        WebSocketManager,
        WebSocketServer,
    },
};

/// Uses only the mining mode's player: asbplayer over the WebSocket server, or mpv for
/// local mining, where mpv is only for watching. The server keeps running in Local mode,
/// unused: asbplayer retries a closed connection only every 10 s, so stopping it would
/// delay switching back.
pub struct PlayerManager {
    pub mpv: MpvManager,
    pub ws: WebSocketManager,
    pub mode: MiningMode,
}

impl PlayerManager {
    pub fn new(mpv: MpvManager, ws: WebSocketManager, mode: MiningMode) -> Self {
        Self { mpv, ws, mode }
    }

    pub fn update(&mut self, websocket_port: u16) {
        self.ws.update();
        if self.ws.server.is_none() {
            if let Err(e) = self.ws.restart_server(websocket_port) {
                eprintln!("[Player] Failed to start WebSocket server: {}", e);
            }
        }
        if self.mode == MiningMode::Local {
            self.mpv.update();
        }
    }

    /// The server asbplayer is reached through, in asbplayer mode only.
    pub fn asbplayer(&self) -> Option<&Arc<WebSocketServer>> {
        self.ws.server.as_ref().filter(|_| self.mode == MiningMode::Asbplayer)
    }

    pub fn asbplayer_connected(&self) -> bool {
        self.mode == MiningMode::Asbplayer && self.ws.has_clients()
    }

    pub fn mpv_connected(&self) -> bool {
        self.mode == MiningMode::Local && self.mpv.is_connected()
    }

    pub fn seek_timestamp(
        &self,
        seconds: f32,
        timestamp_str: &str,
        media_id: Option<&str>,
    ) -> Result<(), YomineError> {
        if self.mpv_connected() {
            self.mpv.seek_timestamp(seconds, timestamp_str)
        } else if let Some(server) = self.asbplayer() {
            server.seek_timestamp(seconds, timestamp_str, media_id)
        } else {
            Err(YomineError::Custom(
                "No player available (MPV disconnected and WebSocket server unavailable)".into(),
            ))
        }
    }

    pub fn is_connected(&self) -> bool {
        self.mpv_connected() || self.asbplayer_connected()
    }

    pub fn get_confirmed_timestamps(&self) -> Vec<f32> {
        let ws_timestamps: &[f32] =
            if self.asbplayer_connected() { self.ws.get_confirmed_timestamps() } else { &[] };
        let mpv_timestamps = self.mpv.get_confirmed_timestamps();

        let mut combined = Vec::with_capacity(ws_timestamps.len() + mpv_timestamps.len());
        combined.extend_from_slice(ws_timestamps);
        combined.extend(mpv_timestamps);
        combined
    }
}

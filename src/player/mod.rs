use crate::{
    core::errors::YomineError,
    gui::websocket_manager::WebSocketManager,
    mpv::MpvManager,
};

pub struct PlayerManager {
    pub mpv: MpvManager,
    pub ws: WebSocketManager,
}

impl PlayerManager {
    pub fn new(mpv: MpvManager, ws: WebSocketManager) -> Self {
        Self { mpv, ws }
    }

    pub fn update(&mut self, websocket_port: u16) {
        self.mpv.update(&mut self.ws, websocket_port);
        self.ws.update();
    }

    pub fn seek_timestamp(&self, seconds: f64, timestamp_str: &str) -> Result<(), YomineError> {
        if self.mpv.is_connected() {
            self.mpv.seek_timestamp(seconds, timestamp_str)
        } else if let Some(server) = &self.ws.server {
            server.seek_timestamp(seconds, timestamp_str)
        } else {
            Err(YomineError::Custom(
                "No player available (MPV disconnected and WebSocket server unavailable)".into(),
            ))
        }
    }

    pub fn is_connected(&self) -> bool {
        self.mpv.is_connected() || self.ws.has_clients()
    }

    pub fn get_confirmed_timestamps(&self) -> Vec<String> {
        let ws_timestamps = self.ws.get_confirmed_timestamps();
        let mpv_timestamps = self.mpv.get_confirmed_timestamps();

        let mut combined = Vec::with_capacity(ws_timestamps.len() + mpv_timestamps.len());
        combined.extend_from_slice(ws_timestamps);
        combined.extend(mpv_timestamps);
        combined
    }
}

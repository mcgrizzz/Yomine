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
        self.mpv.update();
        self.ws.update();

        match (self.mpv.is_connected(), self.ws.server.is_some()) {
            (true, true) => {
                //Prefer mpv to websocket server
                if let Err(e) = self.ws.shutdown_server() {
                    eprintln!("[Player] Failed to shutdown WebSocket server: {}", e);
                } else {
                    println!(
                        "[Player] MPV detected. WebSocket server stopped; switched to MPV mode."
                    );
                }
            }
            // Both disconnected -> start WebSocket
            (false, false) => {
                //Mpv is not connected so make sure we have websocket server
                if let Err(e) = self.ws.restart_server(websocket_port) {
                    eprintln!("[Player] Failed to restart WebSocket server: {}", e);
                } else {
                    println!("[Player] MPV not detected. WebSocket server restarted; switched to asbplayer mode.");
                }
            }
            _ => {}
        }
    }

    pub fn seek_timestamp(&self, seconds: f32, timestamp_str: &str) -> Result<(), YomineError> {
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

    pub fn get_confirmed_timestamps(&self) -> Vec<f32> {
        let ws_timestamps = self.ws.get_confirmed_timestamps();
        let mpv_timestamps = self.mpv.get_confirmed_timestamps();

        let mut combined = Vec::with_capacity(ws_timestamps.len() + mpv_timestamps.len());
        combined.extend_from_slice(ws_timestamps);
        combined.extend(mpv_timestamps);
        combined
    }
}

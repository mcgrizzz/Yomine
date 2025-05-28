use std::sync::Arc;

use crate::websocket::WebSocketServer;

#[derive(Default)]
pub struct WebSocketState {
    pub has_clients: bool,
    pub confirmed_timestamps: Vec<String>,
}

pub struct WebSocketManager {
    pub state: WebSocketState,
    pub server: Option<Arc<WebSocketServer>>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        let websocket_server = WebSocketServer::start_server();
        Self { state: WebSocketState::default(), server: websocket_server }
    }

    pub fn update(&mut self) {
        if let Some(server) = &self.server {
            self.state.has_clients = server.has_clients();

            server.process_pending_confirmations();

            if self.state.has_clients {
                self.state.confirmed_timestamps = server.get_confirmed_timestamps();
            }
        }
    }

    pub fn has_clients(&self) -> bool {
        self.state.has_clients
    }

    pub fn get_confirmed_timestamps(&self) -> &Vec<String> {
        &self.state.confirmed_timestamps
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

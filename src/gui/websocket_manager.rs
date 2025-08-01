use std::sync::Arc;

use crate::{
    core::errors::YomineError,
    websocket::{
        ServerState,
        WebSocketServer,
    },
};

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
    pub fn new(port: u16) -> Self {
        let websocket_server = WebSocketServer::start_server(port);
        Self { state: WebSocketState::default(), server: websocket_server }
    }

    pub fn restart_server(&mut self, port: u16) -> Result<(), YomineError> {
        // Shutdown existing server if it exists
        if let Some(server) = &self.server {
            println!("[WS] Shutting down existing WebSocket server...");
            server.shutdown()?;
            println!("[WS] Server shutdown completed");
        }

        // Clear the old server reference
        self.server = None;

        println!("[WS] Starting new WebSocket server on port {}...", port);

        // Start new server with new port
        self.server = WebSocketServer::start_server(port);

        if self.server.is_none() {
            return Err(YomineError::Custom(format!(
                "Failed to start WebSocket server on port {}",
                port
            )));
        }

        println!("[WS] WebSocket server restart completed successfully");
        Ok(())
    }

    pub fn get_server_state(&self) -> ServerState {
        if let Some(server) = &self.server {
            server.get_server_state()
        } else {
            ServerState::Stopped
        }
    }

    pub fn get_server_port(&self) -> u16 {
        if let Some(server) = &self.server {
            server.get_server_port()
        } else {
            8766 // Default port
        }
    }

    pub fn update(&mut self) {
        if let Some(server) = &self.server {
            self.state.has_clients = server.has_clients();

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
        Self::new(8766) // Default port
    }
}

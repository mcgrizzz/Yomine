use std::{
    net::SocketAddr,
    sync::{
        Arc,
        Mutex,
    },
};

use futures_util::{
    SinkExt,
    StreamExt,
};
use serde::{
    Deserialize,
    Serialize,
};
use tokio::{
    net::TcpListener,
    runtime::Runtime,
    sync::{
        mpsc,
        oneshot,
    },
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::protocol::Message,
};
use uuid::Uuid;

use crate::core::errors::YomineError;

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

// Command to seek to a specific timestamp
#[derive(Debug, Serialize)]
struct SeekCommand {
    command: String,
    #[serde(rename = "messageId")]
    message_id: String,
    body: SeekBody,
}

#[derive(Debug, Serialize)]
struct SeekBody {
    timestamp: f64,
}

// Response from ASBPlayer
#[derive(Debug, Deserialize)]
struct CommandResponse {
    command: String,
    #[serde(rename = "messageId")]
    message_id: String,
    //body: serde_json::Value,
}

// Track timestamp seek status
#[derive(Clone, Debug)]
pub struct SeekStatus {
    pub message_id: String,
    pub timestamp: f64,
    pub timestamp_str: String, // Original timestamp string for display
    pub confirmed: bool,
    pub sent_time: std::time::Instant,
}

#[derive(Clone)]
struct ConnectedClient {
    sender: mpsc::Sender<String>,
}

impl ConnectedClient {
    fn is_valid(&self) -> bool {
        !self.sender.is_closed() && self.sender.capacity() > 0
    }
}

#[derive(Clone)]
pub struct WebSocketServer {
    connected_clients: Arc<Mutex<Vec<ConnectedClient>>>,
    server_running: Arc<Mutex<bool>>,
    seek_statuses: Arc<Mutex<Vec<SeekStatus>>>,
    server_state: Arc<Mutex<ServerState>>,
    server_port: Arc<Mutex<u16>>,
    confirmation_channel:
        Arc<(tokio::sync::mpsc::Sender<String>, Mutex<tokio::sync::mpsc::Receiver<String>>)>,
    shutdown_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    server_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl WebSocketServer {
    /// Clean up invalid clients and return count of removed clients
    fn cleanup_clients(&self) -> usize {
        let mut clients = self.connected_clients.lock().unwrap();
        let initial_count = clients.len();
        clients.retain(|client| client.is_valid());
        initial_count - clients.len()
    }

    pub fn start_server(port: u16) -> Option<Arc<Self>> {
        let rt = match Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("[WS] Failed to create Tokio runtime: {}", e);
                return None;
            }
        };

        let (confirmation_sender, confirmation_receiver) =
            tokio::sync::mpsc::channel::<String>(100);

        let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();

        let server = Self {
            connected_clients: Arc::new(Mutex::new(Vec::new())),
            server_running: Arc::new(Mutex::new(false)),
            seek_statuses: Arc::new(Mutex::new(Vec::new())),
            server_state: Arc::new(Mutex::new(ServerState::Starting)),
            server_port: Arc::new(Mutex::new(port)),
            confirmation_channel: Arc::new((
                confirmation_sender,
                Mutex::new(confirmation_receiver),
            )),
            shutdown_sender: Arc::new(Mutex::new(Some(shutdown_sender))),
            server_handle: Arc::new(Mutex::new(None)),
        };

        let server_arc = Arc::new(server);
        let server_clone = server_arc.clone();

        let start_future = async move {
            if let Err(e) = server_clone.run_server(shutdown_receiver).await {
                eprintln!("[WS] Failed to start WebSocket server: {:?}", e);
                *server_clone.server_state.lock().unwrap() = ServerState::Error(e.to_string());
                *server_clone.server_running.lock().unwrap() = false;
                return;
            }
        };

        let handle = std::thread::spawn(move || {
            rt.block_on(start_future);
        });

        *server_arc.server_handle.lock().unwrap() = Some(handle);

        Some(server_arc)
    }

    async fn run_server(
        &self,
        mut shutdown_receiver: oneshot::Receiver<()>,
    ) -> Result<(), YomineError> {
        let port = *self.server_port.lock().unwrap();
        let addr = format!("127.0.0.1:{}", port).parse::<SocketAddr>().unwrap();

        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => {
                *self.server_running.lock().unwrap() = true;
                *self.server_state.lock().unwrap() = ServerState::Running;
                listener
            }
            Err(e) => {
                let error_msg = format!("Failed to bind to address {}: {}", addr, e);
                *self.server_state.lock().unwrap() = ServerState::Error(error_msg.clone());
                *self.server_running.lock().unwrap() = false;
                return Err(YomineError::Custom(error_msg));
            }
        };

        println!("[WS] WebSocket server running on {}", addr);
        println!("[WS] ASBPlayer can connect to: ws://127.0.0.1:{}/ws", port);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            if !*self.server_running.lock().unwrap() {
                                break;
                            }

                            println!("[WS] New connection from: {}", addr);

                            let clients = self.connected_clients.clone();
                            let confirmation_sender = self.confirmation_channel.0.clone();

                            tokio::spawn(async move {
                                if let Err(e) =
                                    Self::handle_connection(stream, addr, clients, confirmation_sender).await
                                {
                                    eprintln!("[WS] Error handling connection from {}: {:?}", addr, e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("[WS] Error accepting connection: {}", e);
                            break;
                        }
                    }
                }
                _ = &mut shutdown_receiver => {
                    println!("[WS] Received shutdown signal, stopping server...");
                    break;
                }
            }
        }

        *self.server_running.lock().unwrap() = false;
        *self.server_state.lock().unwrap() = ServerState::Stopped;

        Ok(())
    }

    async fn handle_connection(
        stream: tokio::net::TcpStream,
        addr: SocketAddr,
        clients: Arc<Mutex<Vec<ConnectedClient>>>,
        confirmation_sender: tokio::sync::mpsc::Sender<String>,
    ) -> Result<(), YomineError> {
        let ws_stream = accept_async(stream)
            .await
            .map_err(|e| YomineError::Custom(format!("Error during WebSocket handshake: {}", e)))?;

        println!("[WS] WebSocket connection established with: {}", addr);

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        let (tx, mut rx) = mpsc::channel::<String>(32);

        {
            let mut clients_lock = clients.lock().unwrap();
            clients_lock.push(ConnectedClient { sender: tx.clone() });
            println!("[WS] Client registered. Total clients: {}", clients_lock.len());
        }

        let forward_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_sender.send(Message::text(msg)).await.is_err() {
                    break;
                }
            }
        });

        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(message)) => {
                    println!("[WS] Received message from client {}: {}", addr, message);

                    if message == "PING" {
                        println!("[WS] Received PING from client, sending PONG");
                        if let Err(e) = tx.send("PONG".to_string()).await {
                            eprintln!("[WS] Failed to send PONG: {}", e);
                        }
                    } else {
                        match serde_json::from_str::<CommandResponse>(&message) {
                            Ok(response) if response.command == "response" => {
                                println!(
                                    "[WS] Received confirmation from ASBPlayer for message ID: {}",
                                    response.message_id
                                );

                                let message_id = response.message_id.clone();
                                if let Err(e) = confirmation_sender.send(message_id).await {
                                    eprintln!(
                                        "[WS] Failed to send message ID for confirmation: {}",
                                        e
                                    );
                                } else {
                                    println!("[WS] Sent message ID for confirmation to server");
                                }
                            }
                            Err(e) => {
                                println!(
                                    "[WS] Received message that's not a valid response: {}",
                                    e
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("[WS] Client {} disconnected", addr);
                    break;
                }
                Err(e) => {
                    eprintln!("[WS] Error from client {}: {}", addr, e);
                    break;
                }
                _ => {}
            }
        }

        forward_task.abort();

        drop(tx);

        {
            let mut clients_lock = clients.lock().unwrap();
            let initial_count = clients_lock.len();

            clients_lock.retain(|client| {
                if client.sender.is_closed() {
                    return false;
                }

                client.sender.capacity() > 0
            });

            let removed = initial_count - clients_lock.len();
            println!(
                "[WS] Client {} disconnected. Removed {} clients. Total clients remaining: {}",
                addr,
                removed,
                clients_lock.len()
            );
        }

        Ok(())
    }

    pub fn has_clients(&self) -> bool {
        let server_running = *self.server_running.lock().unwrap();
        if !server_running {
            return false;
        }

        let removed = self.cleanup_clients();
        if removed > 0 {
            println!("[WS] Removed {} invalid clients during has_clients check", removed);
        }

        let clients = self.connected_clients.lock().unwrap();
        !clients.is_empty()
    }

    pub fn get_seek_statuses(&self) -> Vec<SeekStatus> {
        let statuses = self.seek_statuses.lock().unwrap();
        statuses.clone()
    }

    pub fn is_timestamp_confirmed(&self, timestamp_str: &str) -> bool {
        let statuses = self.seek_statuses.lock().unwrap();
        statuses.iter().any(|status| status.timestamp_str == timestamp_str && status.confirmed)
    }

    pub fn confirm_seek_status(&self, message_id: &str) -> Option<String> {
        println!("[WS] Confirming seek status for message ID: {}", message_id);
        let mut statuses = self.seek_statuses.lock().unwrap();

        for status in statuses.iter_mut() {
            if status.message_id == message_id {
                status.confirmed = true;
                println!(
                    "[WS] Confirmed timestamp: {} for message ID: {}",
                    status.timestamp_str, message_id
                );
                return Some(status.timestamp_str.clone());
            }
        }

        println!("[WS] No matching status found for message ID: {}", message_id);
        None
    }

    pub fn process_pending_confirmations(&self) {
        if let Ok(mut receiver) = self.confirmation_channel.1.try_lock() {
            loop {
                match receiver.try_recv() {
                    Ok(message_id) => {
                        println!(
                            "[WS] Received confirmation request for message ID: {}",
                            message_id
                        );

                        if let Some(timestamp) = self.confirm_seek_status(&message_id) {
                            println!("[WS] Processed confirmation for timestamp: {}", timestamp);
                        }
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        println!("[WS] Confirmation channel disconnected");
                        break;
                    }
                }
            }
        }
    }

    pub fn get_confirmed_timestamps(&self) -> Vec<String> {
        let statuses = self.seek_statuses.lock().unwrap();
        statuses.iter().filter(|s| s.confirmed).map(|s| s.timestamp_str.clone()).collect()
    }

    pub fn get_server_state(&self) -> ServerState {
        self.server_state.lock().unwrap().clone()
    }

    pub fn get_server_port(&self) -> u16 {
        *self.server_port.lock().unwrap()
    }

    pub fn shutdown(&self) -> Result<(), YomineError> {
        *self.server_state.lock().unwrap() = ServerState::Stopped;
        *self.server_running.lock().unwrap() = false;

        if let Some(sender) = self.shutdown_sender.lock().unwrap().take() {
            let _ = sender.send(());
        }

        {
            let mut clients = self.connected_clients.lock().unwrap();
            for client in clients.iter() {
                let _ = client.sender.try_send("CLOSE".to_string());
            }
            clients.clear();
        }

        if let Some(handle) = self.server_handle.lock().unwrap().take() {
            if let Err(e) = handle.join() {
                eprintln!("Error joining server thread: {:?}", e);
            }
        }

        Ok(())
    }

    pub fn seek_timestamp(&self, timestamp: f64, timestamp_str: &str) -> Result<(), YomineError> {
        println!(
            "[WS] Sending seek command for timestamp: {} seconds, str: {}",
            timestamp, timestamp_str
        );

        let message_id = Uuid::new_v4().to_string();
        println!("[WS] Generated message ID: {}", message_id);

        let command = SeekCommand {
            command: "seek-timestamp".to_string(),
            message_id: message_id.clone(),
            body: SeekBody { timestamp },
        };

        {
            let mut statuses = self.seek_statuses.lock().unwrap();
            statuses.push(SeekStatus {
                message_id: message_id.clone(),
                timestamp,
                timestamp_str: timestamp_str.to_string(),
                confirmed: false,
                sent_time: std::time::Instant::now(),
            });
            println!("[WS] Added seek status to tracking. Total tracked: {}", statuses.len());

            if statuses.len() > 100 {
                statuses.sort_by(|a, b| b.sent_time.cmp(&a.sent_time));
                statuses.truncate(50);
                println!("[WS] Trimmed tracked statuses to 100 entries");
            }
        }

        let json = serde_json::to_string(&command)?;
        println!("[WS] Sending seek command to all clients: {}", json);

        let client_senders = {
            let removed = self.cleanup_clients();
            if removed > 0 {
                println!("[WS] Removed {} invalid clients during seek operation", removed);
            }

            let clients = self.connected_clients.lock().unwrap();
            if clients.is_empty() {
                println!("[WS] No clients connected, can't send seek command");
                return Ok(());
            }

            println!("[WS] Found {} connected clients", clients.len());
            clients.iter().map(|client| client.sender.clone()).collect::<Vec<_>>()
        };
        let rt = Runtime::new()
            .map_err(|e| YomineError::Custom(format!("Failed to create runtime: {}", e)))?;

        for (index, sender) in client_senders.into_iter().enumerate() {
            let json = json.clone();
            let client_index = index + 1;

            rt.spawn(async move {
                println!("[WS] Sending to client #{}: starting...", client_index);
                match sender.send(json).await {
                    Ok(_) => println!("[WS] Successfully sent command to client #{}", client_index),
                    Err(e) => eprintln!("[WS] Failed to send to client #{}: {}", client_index, e),
                }
            });
        }

        Ok(())
    }

    pub fn convert_srt_timestamp_to_seconds(timestamp: &str) -> Result<f64, YomineError> {
        // SRT format: 00:01:47,733 -> 107.733 seconds
        let parts: Vec<&str> = timestamp.split(|c| c == ':' || c == ',' || c == '.').collect();
        if parts.len() < 4 {
            return Err(YomineError::InvalidTimestamp);
        }

        let hours: f64 = parts[0].parse().map_err(|_| YomineError::InvalidTimestamp)?;
        let minutes: f64 = parts[1].parse().map_err(|_| YomineError::InvalidTimestamp)?;
        let seconds: f64 = parts[2].parse().map_err(|_| YomineError::InvalidTimestamp)?;
        let milliseconds: f64 = parts[3].parse().map_err(|_| YomineError::InvalidTimestamp)?;

        Ok(hours * 3600.0 + minutes * 60.0 + seconds + milliseconds / 1000.0)
    }
}

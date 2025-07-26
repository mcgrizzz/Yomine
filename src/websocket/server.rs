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
    sync::mpsc,
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::protocol::Message,
};
use uuid::Uuid;

use crate::core::errors::YomineError;

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

// Connected client information
#[derive(Clone)]
struct ConnectedClient {
    sender: mpsc::Sender<String>,
}

// Our WebSocket server that ASBPlayer connects to
#[derive(Clone)]
pub struct WebSocketServer {
    connected_clients: Arc<Mutex<Vec<ConnectedClient>>>,
    server_running: Arc<Mutex<bool>>,
    seek_statuses: Arc<Mutex<Vec<SeekStatus>>>,
    // A channel for message ID confirmations from the connection handlers to the main server
    confirmation_channel:
        Arc<(tokio::sync::mpsc::Sender<String>, Mutex<tokio::sync::mpsc::Receiver<String>>)>,
}

impl WebSocketServer {
    pub fn start_server() -> Option<Arc<Self>> {
        let rt = match Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("Failed to create Tokio runtime: {}", e);
                return None;
            }
        };

        // Create a channel for passing message IDs back to the main server instance
        let (confirmation_sender, confirmation_receiver) =
            tokio::sync::mpsc::channel::<String>(100);

        // Create the server instance
        let server = Self {
            connected_clients: Arc::new(Mutex::new(Vec::new())),
            server_running: Arc::new(Mutex::new(false)),
            seek_statuses: Arc::new(Mutex::new(Vec::new())),
            confirmation_channel: Arc::new((
                confirmation_sender,
                Mutex::new(confirmation_receiver),
            )),
        };

        let server_arc = Arc::new(server);
        let server_clone = server_arc.clone();

        // Start the server in a separate task
        let start_future = async move {
            if let Err(e) = server_clone.run_server().await {
                eprintln!("Failed to start WebSocket server: {:?}", e);
                return;
            }
        };

        std::thread::spawn(move || {
            rt.block_on(start_future);
        });

        *server_arc.server_running.lock().unwrap() = true;
        Some(server_arc)
    }

    // Run the WebSocket server
    async fn run_server(&self) -> Result<(), YomineError> {
        let addr = "127.0.0.1:8766".parse::<SocketAddr>().unwrap();

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| YomineError::Custom(format!("Failed to bind to address: {}", e)))?;

        println!("WebSocket server running on {}", addr);
        println!("ASBPlayer can connect to: ws://127.0.0.1:8766/ws");

        while let Ok((stream, addr)) = listener.accept().await {
            println!("New connection from: {}", addr);

            let clients = self.connected_clients.clone();
            let confirmation_sender = self.confirmation_channel.0.clone();

            tokio::spawn(async move {
                if let Err(e) =
                    Self::handle_connection(stream, addr, clients, confirmation_sender).await
                {
                    eprintln!("Error handling connection from {}: {:?}", addr, e);
                }
            });
        }

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

        println!("WebSocket connection established with: {}", addr);

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        let (tx, mut rx) = mpsc::channel::<String>(32);

        {
            let mut clients_lock = clients.lock().unwrap();
            clients_lock.push(ConnectedClient { sender: tx.clone() });
            println!("Client registered. Total clients: {}", clients_lock.len());
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
                    println!("Received message from client {}: {}", addr, message);

                    if message == "PING" {
                        println!("Received PING from client, sending PONG");
                        if let Err(e) = tx.send("PONG".to_string()).await {
                            eprintln!("Failed to send PONG: {}", e);
                        }
                    } else {
                        match serde_json::from_str::<CommandResponse>(&message) {
                            Ok(response) if response.command == "response" => {
                                println!(
                                    "Received confirmation from ASBPlayer for message ID: {}",
                                    response.message_id
                                );

                                let message_id = response.message_id.clone();
                                if let Err(e) = confirmation_sender.send(message_id).await {
                                    eprintln!("Failed to send message ID for confirmation: {}", e);
                                } else {
                                    println!("Sent message ID for confirmation to server");
                                }
                            }
                            Err(e) => {
                                println!("Received message that's not a valid response: {}", e);
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("Client {} disconnected", addr);
                    break;
                }
                Err(e) => {
                    eprintln!("Error from client {}: {}", addr, e);
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
                "Client {} disconnected. Removed {} clients. Total clients remaining: {}",
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

        let mut clients = self.connected_clients.lock().unwrap();
        let initial_count = clients.len();

        clients.retain(|client| !client.sender.is_closed() && client.sender.capacity() > 0);

        let removed = initial_count - clients.len();
        if removed > 0 {
            println!("Removed {} invalid clients during has_clients check", removed);
        }

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
        println!("Confirming seek status for message ID: {}", message_id);
        let mut statuses = self.seek_statuses.lock().unwrap();

        for status in statuses.iter_mut() {
            if status.message_id == message_id {
                status.confirmed = true;
                println!(
                    "Confirmed timestamp: {} for message ID: {}",
                    status.timestamp_str, message_id
                );
                return Some(status.timestamp_str.clone());
            }
        }

        println!("No matching status found for message ID: {}", message_id);
        None
    }

    pub fn process_pending_confirmations(&self) {
        if let Ok(mut receiver) = self.confirmation_channel.1.try_lock() {
            loop {
                match receiver.try_recv() {
                    Ok(message_id) => {
                        println!("Received confirmation request for message ID: {}", message_id);

                        if let Some(timestamp) = self.confirm_seek_status(&message_id) {
                            println!("Processed confirmation for timestamp: {}", timestamp);
                        }
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        println!("Confirmation channel disconnected");
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

    pub fn seek_timestamp(&self, timestamp: f64, timestamp_str: &str) -> Result<(), YomineError> {
        println!(
            "Sending seek command for timestamp: {} seconds, str: {}",
            timestamp, timestamp_str
        );

        let message_id = Uuid::new_v4().to_string();
        println!("Generated message ID: {}", message_id);

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
            println!("Added seek status to tracking. Total tracked: {}", statuses.len());

            if statuses.len() > 100 {
                statuses.sort_by(|a, b| b.sent_time.cmp(&a.sent_time));
                statuses.truncate(50);
                println!("Trimmed tracked statuses to 100 entries");
            }
        }

        let json = serde_json::to_string(&command)?;
        println!("Sending seek command to all clients: {}", json);

        let client_senders = {
            let mut clients = self.connected_clients.lock().unwrap();

            let initial_count = clients.len();
            clients.retain(|client| !client.sender.is_closed() && client.sender.capacity() > 0);

            let removed = initial_count - clients.len();
            if removed > 0 {
                println!("Removed {} invalid clients during seek operation", removed);
            }

            if clients.is_empty() {
                println!("No clients connected, can't send seek command");
                return Ok(());
            }

            println!("Found {} connected clients", clients.len());

            clients.iter().map(|client| client.sender.clone()).collect::<Vec<_>>()
        };
        let rt = Runtime::new()
            .map_err(|e| YomineError::Custom(format!("Failed to create runtime: {}", e)))?;

        // Spawn a task for each client to send the message
        for (index, sender) in client_senders.into_iter().enumerate() {
            let json = json.clone();
            let client_index = index + 1;

            rt.spawn(async move {
                println!("Sending to client #{}: starting...", client_index);
                match sender.send(json).await {
                    Ok(_) => println!("Successfully sent command to client #{}", client_index),
                    Err(e) => eprintln!("Failed to send to client #{}: {}", client_index, e),
                }
            });
        }

        Ok(())
    }

    // Convert an SRT timestamp to seconds
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

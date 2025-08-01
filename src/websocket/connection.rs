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
use tokio::sync::mpsc::{
    self,
    UnboundedSender,
};
use tokio_tungstenite::tungstenite::protocol::Message;

use super::types::{
    CommandResponse,
    ConnectedClient,
    ServerCommand,
};
use crate::core::errors::YomineError;

pub async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    clients: Arc<Mutex<Vec<ConnectedClient>>>,
    command_sender: Arc<Mutex<Option<UnboundedSender<ServerCommand>>>>,
) -> Result<(), YomineError> {
    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(|e| YomineError::Custom(format!("Error during WebSocket handshake: {}", e)))?;

    println!("[WS] WebSocket connection established with: {}", addr);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let (tx, mut rx) = mpsc::channel::<String>(32);

    {
        let mut clients_lock = clients.lock().unwrap();
        clients_lock.push(ConnectedClient { tx: tx.clone() });
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

                            if let Some(sender) = command_sender.lock().unwrap().as_ref() {
                                let confirmation_command = ServerCommand::ProcessConfirmation {
                                    message_id: response.message_id.clone(),
                                };
                                if let Err(e) = sender.send(confirmation_command) {
                                    eprintln!("[WS] Failed to send confirmation command: {}", e);
                                }
                            } else {
                                eprintln!("[WS] Command sender not available for confirmation");
                            }
                        }
                        Err(e) => {
                            println!("[WS] Received message that's not a valid response: {}", e);
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
            if client.tx.is_closed() {
                return false;
            }

            client.tx.capacity() > 0
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

use serde::{
    Deserialize,
    Serialize,
};
use tokio::sync::mpsc;

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
    ProcessConfirmation { message_id: String },
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
    pub timestamp: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommandResponse {
    pub command: String,
    #[serde(rename = "messageId")]
    pub message_id: String,
}

#[derive(Clone, Debug)]
pub struct SeekStatus {
    pub message_id: String,
    pub timestamp: f64,
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

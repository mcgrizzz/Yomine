use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio_tungstenite::tungstenite;

#[derive(Error, Debug)]
pub enum YomineError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HJson error: {0}")]
    HJson(#[from] serde_hjson::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Vibrato error: {0}")]
    Vibrato(#[from] vibrato::errors::VibratoError),
        
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tungstenite::Error),
    
    #[error("WebSocket send error: {0}")]
    WebSocketSend(String),
    
    #[error("Invalid timestamp format")]
    InvalidTimestamp,

    #[error("index.json must have either 'format' or 'version'")]
    MissingVersion,

    #[error("Failed to load file: {0}")]
    FailedToLoadFile(String),

    #[error("Failed to load unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("YomineError: {0}")]
    Custom(String),
}

impl<T> From<SendError<T>> for YomineError {
    fn from(error: SendError<T>) -> Self {
        YomineError::WebSocketSend(error.to_string())
    }
}
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio_tungstenite::tungstenite;

#[derive(Error, Debug)]
pub enum YomineError {
    #[error("I/O error: {0}")]
    Io(Box<std::io::Error>),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HJson error: {0}")]
    HJson(#[from] serde_hjson::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Reqwest error: {0}")]
    Reqwest(Box<reqwest::Error>),

    #[error("Vibrato error: {0}")]
    Vibrato(Box<vibrato::errors::VibratoError>),

    #[error("WebSocket error: {0}")]
    WebSocket(Box<tungstenite::Error>),

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

impl From<std::io::Error> for YomineError {
    fn from(error: std::io::Error) -> Self {
        YomineError::Io(Box::new(error))
    }
}

impl From<reqwest::Error> for YomineError {
    fn from(error: reqwest::Error) -> Self {
        YomineError::Reqwest(Box::new(error))
    }
}

impl From<vibrato::errors::VibratoError> for YomineError {
    fn from(error: vibrato::errors::VibratoError) -> Self {
        YomineError::Vibrato(Box::new(error))
    }
}

impl From<tungstenite::Error> for YomineError {
    fn from(error: tungstenite::Error) -> Self {
        YomineError::WebSocket(Box::new(error))
    }
}

pub mod dictionary;
pub mod frequency_dict;
pub mod pos;
pub mod tokenizer;
pub mod parser;

use thiserror::Error;

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

    #[error("index.json must have either 'format' or 'version'")]
    MissingVersion,

    #[error("YomineError: {0}")]
    Custom(String),
}
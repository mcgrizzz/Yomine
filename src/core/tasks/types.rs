use std::collections::HashMap;

use crate::{
    anki::Model,
    core::{
        Sentence,
        Term,
    },
    gui::app::LanguageTools,
};

#[derive(Debug, Clone)]
pub enum TaskResult {
    AnkiConnection(bool),
    AnkiModels(Result<Vec<Model>, String>),
    AnkiSampleNote { model_name: String, result: Result<HashMap<String, String>, String> },

    FileProcessing(Result<(Vec<Term>, Vec<Sentence>), String>),

    LanguageToolsLoaded(Result<LanguageTools, String>),
    LoadingMessage(String),
}

impl TaskResult {
    pub fn task_type(&self) -> &'static str {
        match self {
            TaskResult::AnkiConnection(_) => "anki_connection",
            TaskResult::AnkiModels(_) => "anki_models",
            TaskResult::AnkiSampleNote { .. } => "anki_sample",
            TaskResult::FileProcessing(_) => "file_processing",
            TaskResult::LanguageToolsLoaded(_) => "language_tools",
            TaskResult::LoadingMessage(_) => "loading_message",
        }
    }
}

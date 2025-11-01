use std::collections::HashMap;

use crate::{
    anki::Model,
    core::{
        pipeline::FilterResult,
        Sentence,
        Term,
    },
    gui::app::LanguageTools,
};

pub type FileProcessingResult = Result<(Vec<Term>, FilterResult, Vec<Sentence>, f32), String>;

#[derive(Debug, Clone)]
pub enum TaskResult {
    AnkiConnection(bool),
    AnkiModels(Result<Vec<Model>, String>),
    AnkiSampleNote { model_name: String, result: Result<HashMap<String, String>, String> },

    FileProcessing(FileProcessingResult),
    RequestRefresh,
    RequestSaveSettings,
    TermsRefreshed(Result<(FilterResult, f32), String>),

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
            TaskResult::RequestRefresh => "request_refresh",
            TaskResult::RequestSaveSettings => "request_save_settings",
            TaskResult::TermsRefreshed(_) => "terms_refreshed",
            TaskResult::LanguageToolsLoaded(_) => "language_tools",
            TaskResult::LoadingMessage(_) => "loading_message",
        }
    }
}

use std::collections::HashMap;

pub use super::handle::{
    CancellableTask,
    TaskHandle,
};
use crate::{
    anki::Model,
    core::{
        pipeline::FilterResult,
        Sentence,
        Term,
    },
    gui::app::LanguageTools,
    tools::analysis::FrequencyAnalysisResult,
};

pub type FileProcessingResult = Result<(Vec<Term>, FilterResult, Vec<Sentence>, f32), String>;

#[derive(Debug, Clone)]
pub struct AnalysisProgress {
    pub current_file: usize,
    pub message: String,
    pub file_size: u64,
}

#[derive(Debug, Clone)]
pub enum FrequencyAnalysisUpdate {
    Progress(AnalysisProgress),
    Complete(Result<FrequencyAnalysisResult, String>),
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum TaskResult {
    AnkiConnection(bool),
    AnkiModels(Result<Vec<Model>, String>),
    AnkiSampleNote { model_name: String, result: Result<HashMap<String, String>, String> },

    FileProcessing(FileProcessingResult),
    RequestRefresh,
    RequestSaveSettings,
    TermsRefreshed(Result<(FilterResult, Vec<Sentence>, f32), String>),

    LanguageToolsLoaded(Result<LanguageTools, String>),
    LoadingMessage(String),

    FrequencyAnalysis(FrequencyAnalysisUpdate),
    FrequencyExport(Result<String, String>),
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
            TaskResult::FrequencyAnalysis(update) => match update {
                FrequencyAnalysisUpdate::Progress(_) => "frequency_analysis_progress",
                FrequencyAnalysisUpdate::Complete(_) => "frequency_analysis",
                FrequencyAnalysisUpdate::Cancelled => "frequency_analysis_cancelled",
            },
            TaskResult::FrequencyExport(_) => "frequency_export",
        }
    }
}

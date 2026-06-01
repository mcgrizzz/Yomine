//! Backend-owned application state (Constitution: one source of truth per datum).
//!
//! Heavy, non-serializable runtime handles (`LanguageTools` = tokenizer + freq
//! manager + ignore list, the analyzer result) live here in `tauri::State` and
//! never cross the IPC boundary (Constitution III). Commands lock this briefly,
//! clone what they need (the `Arc`-backed handles are cheap to clone), then
//! release the lock before doing async work — never holding the `Mutex` across an
//! `.await`.
//!
//! The player is deliberately **not** here: it is owned by a dedicated task (see
//! `player_task`) and reached over a channel, so its periodic blocking I/O never
//! contends with this lock.

use std::sync::{
    atomic::AtomicBool,
    Arc,
};

use yomine::{
    core::{
        models::{
            Sentence,
            SourceFile,
            Term,
        },
        settings::SettingsData,
        LanguageTools,
    },
    tools::analysis::FrequencyAnalysisResult,
};

/// The currently loaded file and its enriched analysis. Mirrors the egui
/// `gui::app::file_data::FileData`, but holds only what the backend needs to
/// answer `get_terms`/`refresh_terms` and build the wire DTOs.
#[derive(Default)]
pub struct FileData {
    pub source_file: Option<SourceFile>,
    /// All terms (visible + Anki-filtered + ignore-filtered), as the pipeline
    /// returns them. Filtering for display happens client-side (research R6).
    pub terms: Vec<Term>,
    pub sentences: Vec<Sentence>,
    pub file_comprehension: f32,
}

pub struct AppState {
    /// `None` until `load_language_tools` finishes (app start).
    pub language_tools: Option<LanguageTools>,
    pub settings: SettingsData,
    pub file: FileData,
    /// Flipped by `cancel_analysis`; read by the frequency analyzer (research R5).
    pub analysis_cancel: Arc<AtomicBool>,
    /// Kept for `export_analysis`; only a preview DTO is sent to the UI.
    pub last_analysis: Option<FrequencyAnalysisResult>,
}

impl AppState {
    pub fn new(settings: SettingsData) -> Self {
        Self {
            language_tools: None,
            settings,
            file: FileData::default(),
            analysis_cancel: Arc::new(AtomicBool::new(false)),
            last_analysis: None,
        }
    }
}

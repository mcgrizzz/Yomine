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

use std::{
    collections::HashSet,
    sync::{
        atomic::AtomicBool,
        Arc,
    },
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

use crate::dto::KnowledgeSummaryDto;

/// The currently loaded file and its enriched analysis. Mirrors the egui
/// `gui::app::file_data::FileData`, but holds only what the backend needs to
/// answer `get_terms`/`refresh_terms` and build the wire DTOs.
#[derive(Default)]
pub struct FileData {
    pub source_file: Option<SourceFile>,
    /// The minable (unknown) terms shown in the table — `filter_result.terms`.
    /// Display-side refinement (POS/search/sort/range) happens client-side (R6).
    pub terms: Vec<Term>,
    /// All terms (visible + Anki-filtered + ignore-filtered) as the pipeline
    /// returns them; kept so a live Anki refresh can re-partition without
    /// re-segmenting (mirrors egui's `FileData::original_terms`).
    pub base_terms: Vec<Term>,
    /// Lemma forms Anki already knew (from `filter_result.anki_filtered`). Kept so
    /// an ignore-list change can re-filter without re-querying Anki — passed as
    /// `AnkiFilter::KnownLemmas` so known terms stay filtered out (mirrors egui's
    /// `FileData::anki_filtered_terms`, used by `partial_refresh`).
    pub anki_known_lemmas: HashSet<String>,
    /// How many base terms the ignore list filtered out
    /// (`filter_result.ignore_filtered.len()`); display-only, for the file
    /// summary's known-count hover breakdown (egui `ui_current_file_summary`).
    pub ignored_count: usize,
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
    /// Set when an input to the knowledge summary changes (settings save, dict
    /// reload, live Anki refresh); the background task recomputes and clears it.
    /// Starts `true` so the first cached summary loads once tools are ready.
    /// Mirrors egui's `knowledge_summary_attempted` reset (R5).
    pub knowledge_dirty: Arc<AtomicBool>,
    /// Last summary the background task computed, cached so a (re)loaded webview
    /// can pull it via `get_knowledge_summary` — the `knowledge-summary` event
    /// fires only on change, so without this the widget sits blank after a reload.
    pub knowledge_summary: Option<KnowledgeSummaryDto>,
}

impl AppState {
    pub fn new(settings: SettingsData) -> Self {
        Self {
            language_tools: None,
            settings,
            file: FileData::default(),
            analysis_cancel: Arc::new(AtomicBool::new(false)),
            last_analysis: None,
            knowledge_dirty: Arc::new(AtomicBool::new(true)),
            knowledge_summary: None,
        }
    }
}

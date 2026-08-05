//! Backend-owned state. Lock discipline: lock briefly, clone the Arc-backed
//! handles, release before async work — never hold the `Mutex` across `.await`.
//! The player is deliberately NOT here (see `player_task`): its blocking I/O
//! must not contend with this lock.

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

pub const KNOWLEDGE_SUMMARY_CACHE: &str = "knowledge_summary_cache.json";

#[derive(Default)]
pub struct FileData {
    pub source_file: Option<SourceFile>,
    /// The minable (unknown) terms shown in the table.
    pub terms: Vec<Term>,
    /// All terms as the pipeline returned them — a live Anki refresh
    /// re-partitions these without re-segmenting.
    pub base_terms: Vec<Term>,
    /// Lemma forms Anki already knew — an ignore-list change re-filters against
    /// these without re-querying Anki.
    pub anki_known_lemmas: HashSet<String>,
    /// Base terms the ignore list filtered out (display-only breakdown).
    pub ignored_count: usize,
    pub sentences: Vec<Sentence>,
    pub file_comprehension: f32,
    /// `None` for regular files. Arms follow mode and tells the active-tab
    /// follow what is currently showing.
    pub asbplayer_media_id: Option<String>,
    /// Loaded track's file name — dedupes re-loads of the same subtitles.
    pub asbplayer_subtitle_file: Option<String>,
}

pub struct AppState {
    /// `None` until `load_language_tools` finishes.
    pub language_tools: Option<LanguageTools>,
    pub settings: SettingsData,
    pub file: FileData,
    pub analysis_cancel: Arc<AtomicBool>,
    /// Kept for `export_analysis`; only a preview DTO is sent to the UI.
    pub last_analysis: Option<FrequencyAnalysisResult>,
    /// Set when a knowledge-summary input changes; the background task
    /// recomputes and clears it. Starts `true` so the first cached summary
    /// loads once tools are ready.
    pub knowledge_dirty: Arc<AtomicBool>,
    /// Cached for `get_knowledge_summary` — the event fires only on change, so
    /// without this the widget sits blank after a webview reload.
    pub knowledge_summary: Option<KnowledgeSummaryDto>,
    /// From the last catalog fetch; installs resolve their download URL here so
    /// the frontend only ever passes a title.
    pub recommended_catalog: Vec<crate::recommended::RecommendedEntry>,
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
            knowledge_summary: {
                let cached: KnowledgeSummaryDto =
                    yomine::persistence::load_json_or_default(KNOWLEDGE_SUMMARY_CACHE);
                (!cached.jlpt.is_empty() || !cached.frequency.is_empty()).then_some(cached)
            },
            recommended_catalog: Vec::new(),
        }
    }
}

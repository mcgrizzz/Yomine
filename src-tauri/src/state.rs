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
    anki::AnkiState,
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
    /// Identity of this processed result, distinct even when reopening the same path.
    pub revision: Arc<()>,
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
    /// Changes when settings or dictionary evidence invalidate in-flight work.
    pub input_revision: Arc<()>,
    /// Prevent live Anki from filtering old segments while dictionaries are rebuilding them.
    pub dictionary_refresh_pending: Option<Arc<()>>,
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
    /// Reading-keyed duplicate lookup for the popover.
    pub known_entry_keys: Option<HashSet<String>>,
    cached_anki_state: Option<Arc<AnkiState>>,
}

impl AppState {
    pub fn new(settings: SettingsData) -> Self {
        Self {
            input_revision: Arc::new(()),
            dictionary_refresh_pending: None,
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
            known_entry_keys: None,
            cached_anki_state: None,
        }
    }

    pub fn anki_state(&mut self) -> Option<Arc<AnkiState>> {
        if self.cached_anki_state.is_none() {
            let tools = self.language_tools.as_ref()?;
            self.cached_anki_state =
                AnkiState::from_cache(tools.frequency_manager.clone(), tools.known_interval)
                    .map(Arc::new);
        }
        self.cached_anki_state.clone()
    }

    pub fn set_anki_state(&mut self, state: Arc<AnkiState>) {
        self.cached_anki_state = Some(state);
        self.known_entry_keys = None;
    }

    /// The snapshot bakes in the vocab cache, `known_interval` and `frequency_manager`.
    pub fn invalidate_anki_cache(&mut self) {
        self.input_revision = Arc::new(());
        self.cached_anki_state = None;
        self.known_entry_keys = None;
    }

    pub fn file_update_is_current(&self, file: &Arc<()>, inputs: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.file.revision, file) && Arc::ptr_eq(&self.input_revision, inputs)
    }

    pub fn invalidate_dictionary_evidence(&mut self) {
        self.invalidate_anki_cache();
        self.dictionary_refresh_pending = Some(self.input_revision.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_refresh_cannot_overwrite_a_new_file_or_newer_settings() {
        let mut state = AppState::new(SettingsData::default());
        let file = state.file.revision.clone();
        let inputs = state.input_revision.clone();
        assert!(state.file_update_is_current(&file, &inputs));

        // Even reopening the same path produces a distinct processed result.
        state.file = FileData::default();
        assert!(!state.file_update_is_current(&file, &inputs));
        let file = state.file.revision.clone();
        assert!(state.file_update_is_current(&file, &inputs));

        state.invalidate_anki_cache();
        assert!(!state.file_update_is_current(&file, &inputs));
    }

    #[test]
    fn later_dictionary_changes_supersede_pending_refreshes() {
        let mut state = AppState::new(SettingsData::default());
        state.invalidate_dictionary_evidence();
        let first = state.input_revision.clone();
        let file = state.file.revision.clone();
        state.invalidate_dictionary_evidence();
        assert!(!state.file_update_is_current(&file, &first));
        assert!(Arc::ptr_eq(
            state.dictionary_refresh_pending.as_ref().unwrap(),
            &state.input_revision,
        ));

        // A settings change must not leave a stale pending flag blocking Anki.
        state.invalidate_anki_cache();
        assert!(!Arc::ptr_eq(
            state.dictionary_refresh_pending.as_ref().unwrap(),
            &state.input_revision,
        ));
    }
}

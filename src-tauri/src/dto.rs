//! Wire DTOs that cross the IPC boundary (data-model.md). Domain types are
//! serialized directly when cheap; a DTO is introduced only where the domain
//! type is awkward on the wire — here, `Sentence` (its `TimeStamp` wraps
//! `time::Time`, which we do not serialize; research R4).

use serde::{
    Deserialize,
    Serialize,
};
use wana_kana::ConvertJapanese;
use yomine::{
    core::models::{
        Sentence,
        SourceFile,
        Term,
    },
    segmentation::word::POS,
    tools::knowledge_summary::{
        BandStats,
        KnowledgeSummary,
    },
};

/// One `<ruby>` span over the sentence text: the surface slice, its reading +
/// POS, and the original `[start,end)` byte offsets. `surface` is pre-sliced and
/// `reading` pre-converted to hiragana (egui's `.to_hiragana()`), so the UI never
/// has to slice the sentence by UTF-8 byte offsets in JS. `start`/`end` are kept
/// for the in-sentence term highlight (numeric overlap test on the frontend).
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentDto {
    pub surface: String,
    pub reading: String,
    pub pos: POS,
    pub start: usize,
    pub end: usize,
}

/// Seconds (for seeking, FR-008) + human-readable labels (for display). Replaces
/// the internal `TimeStamp` which wraps non-serializable `time::Time`.
#[derive(Serialize, Deserialize, Clone)]
pub struct TimeStampDto {
    pub start_secs: f32,
    pub end_secs: f32,
    pub start_label: String,
    pub end_label: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SentenceDto {
    pub id: usize,
    pub source_id: u32,
    pub text: String,
    pub segments: Vec<SegmentDto>,
    pub timestamp: Option<TimeStampDto>,
    pub comprehension: f32,
}

impl SentenceDto {
    pub fn from_sentence(s: &Sentence) -> Self {
        let text = &s.text;
        let segments = s
            .segments
            .iter()
            .map(|(reading, pos, start, end)| SegmentDto {
                surface: text[*start..*end].to_string(),
                reading: reading.to_hiragana(),
                pos: *pos,
                start: *start,
                end: *end,
            })
            .collect();

        let timestamp = s.timestamp.as_ref().map(|ts| {
            let (start_secs, end_secs) = ts.to_secs();
            let (start_label, end_label) = ts.to_human_readable();
            TimeStampDto { start_secs, end_secs, start_label, end_label }
        });

        Self {
            id: s.id,
            source_id: s.source_id,
            text: s.text.clone(),
            segments,
            timestamp,
            comprehension: s.comprehension,
        }
    }
}

/// The payload for `process_file`/`get_terms`/`terms-refreshed` (contracts/commands.md).
#[derive(Serialize, Deserialize, Clone)]
pub struct FileLoadResult {
    pub source_file: SourceFile,
    pub terms: Vec<Term>,
    pub sentences: Vec<SentenceDto>,
    pub file_comprehension: f32,
    /// Whether Anki filtering removed any terms — gates the per-sentence
    /// comprehension indicator (egui checks `anki_filtered_terms.is_empty()`).
    pub anki_filter_active: bool,
    /// Total terms before filtering (`base_terms`), for the file summary's
    /// "shown / known / total" counts (egui `ui_current_file_summary`).
    pub total_terms: usize,
    /// Terms hidden by the ignore list — the known-count hover breakdown.
    pub ignored_terms: usize,
}

/// One file pill in the ignore-list modal: the persisted `IgnoreFile` fields
/// (`path`, `enabled`) plus the display-only `exists` + `term_count` the modal
/// renders (contracts/commands.md `IgnoreFileView`).
#[derive(Serialize, Deserialize, Clone)]
pub struct IgnoreFileView {
    pub path: String,
    pub enabled: bool,
    pub exists: bool,
    pub term_count: usize,
}

/// Full ignore-list state that hydrates the modal (`IgnoreListView`): the manual
/// lemma terms plus the file pills. Mirrors egui's `IgnoreListModal::open_modal`.
#[derive(Serialize, Deserialize, Clone)]
pub struct IgnoreListView {
    pub terms: Vec<String>,
    pub files: Vec<IgnoreFileView>,
}

/// Static POS metadata for filter/label rendering (`get_pos_catalog`). `key` is
/// `POS::as_key()` (matches `settings.pos_filters` keys); `display_name` is the label.
#[derive(Serialize, Deserialize, Clone)]
pub struct PosInfo {
    pub key: String,
    pub display_name: String,
}

impl PosInfo {
    pub fn from_pos(pos: POS) -> Self {
        Self { key: pos.as_key().to_string(), display_name: pos.display_name().to_string() }
    }
}

/// One row of the frequency-dictionary list (`list_dictionaries`). The engine
/// `DictionaryState` holds only `weight`/`enabled`; the name is the map key, so
/// this DTO folds it in to give the modal a flat `{ name, weight, enabled }`
/// (data-model.md "Frequency dictionary state").
#[derive(Serialize, Deserialize, Clone)]
pub struct DictionaryStateDto {
    pub name: String,
    pub weight: f32,
    pub enabled: bool,
}

/// Per-file analysis progress streamed over a `Channel` while `start_analysis`
/// runs (data-model.md "Analysis progress"). `total_bytes`/`bytes_processed`
/// drive the progress bar; `eta_secs` is the smoothed remaining-time estimate
/// (alpha=0.3, `null` until the first byte lands) computed backend-side so the
/// UI just renders it. `current_file` is 1-based (mirrors the engine callback).
#[derive(Serialize, Deserialize, Clone)]
pub struct AnalysisProgressDto {
    pub total_files: usize,
    pub current_file: usize,
    pub message: String,
    pub total_bytes: u64,
    pub bytes_processed: u64,
    pub eta_secs: Option<f32>,
}

/// One row of the frequency-analysis preview table (`AnalysisPreview.entries`).
/// `term`/`reading` are the deinflected lemma + its reading (`None` for pure
/// kana); `frequency` is the corpus count; `count` is kept as an explicit alias
/// so the UI can label "occurrences" without re-deriving it (data-model.md).
#[derive(Serialize, Deserialize, Clone)]
pub struct AnalysisPreviewEntry {
    pub term: String,
    pub reading: Option<String>,
    pub frequency: u32,
    pub count: u32,
}

/// The results preview returned by `start_analysis` (and re-emitted on the
/// `analysis-complete` event). Only this lightweight view crosses the IPC
/// boundary — the full `FrequencyAnalysisResult` (needed for export) stays in
/// `AppState.last_analysis`. `entries` are sorted by frequency descending and
/// capped; `total` is the full unique-lemma count before the cap (data-model.md).
#[derive(Serialize, Deserialize, Clone)]
pub struct AnalysisPreview {
    pub entries: Vec<AnalysisPreviewEntry>,
    /// The lowest-frequency slice (last ≤`PREVIEW_LIMIT` of the same
    /// frequency-descending list) for the UI's Bottom 250 radio.
    pub bottom: Vec<AnalysisPreviewEntry>,
    pub total: usize,
}

/// Aggregated readiness for the setup checklist/banner (`get_setup_status`).
/// Each field mirrors the matching egui `check_*` in `setup_checklist_modal.rs`.
#[derive(Serialize, Deserialize, Clone)]
pub struct SetupStatus {
    pub tools_loaded: bool,
    pub anki_connected: bool,
    pub has_field_mapping: bool,
    pub has_frequency_dict: bool,
    /// Number of loaded frequency dictionaries. The bool above answers item 2
    /// ("default dict installed", count ≥ 1); this count answers item 6
    /// ("additional dicts installed", count > 1) — egui's `check_additional_freq_dicts`.
    pub frequency_dict_count: usize,
    pub player_connected: bool,
}

/// One JLPT band of the knowledge summary (`get_knowledge_summary` /
/// `knowledge-summary`). Flattens the engine's `(JlptLevel, BandStats)` tuple to
/// named `{ level, stats }` — `BandStats` already serializes cleanly, but the
/// positional tuple does not (it lands as a JS array), so the bands get a DTO.
/// `level` is the display label (egui `JlptLevel::label`).
#[derive(Serialize, Deserialize, Clone)]
pub struct JlptBand {
    pub level: String,
    pub stats: BandStats,
}

/// One frequency band of the knowledge summary; flattens `(String, BandStats)`
/// to `{ label, stats }` (the label is already the display string, e.g. "<1.5k").
#[derive(Serialize, Deserialize, Clone)]
pub struct FrequencyBand {
    pub label: String,
    pub stats: BandStats,
}

/// The knowledge summary as it crosses IPC. The engine `KnowledgeSummary` holds
/// `Vec<(_, BandStats)>` tuples (positional on the wire); this names the fields
/// so the TS `KnowledgeSummary` interface deserializes as objects, not arrays.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct KnowledgeSummaryDto {
    pub jlpt: Vec<JlptBand>,
    pub frequency: Vec<FrequencyBand>,
}

impl KnowledgeSummaryDto {
    pub fn from_summary(s: KnowledgeSummary) -> Self {
        Self {
            jlpt: s
                .jlpt
                .into_iter()
                .map(|(level, stats)| JlptBand { level: level.label().to_string(), stats })
                .collect(),
            frequency: s
                .frequency
                .into_iter()
                .map(|(label, stats)| FrequencyBand { label, stats })
                .collect(),
        }
    }
}

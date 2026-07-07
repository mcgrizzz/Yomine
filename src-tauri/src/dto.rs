//! Wire DTOs (data-model.md). Domain types serialize directly when cheap; a
//! DTO exists only where the domain type is awkward on the wire.

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

/// One `<ruby>` span. `surface` is pre-sliced and `reading` pre-converted to
/// hiragana so the UI never slices by UTF-8 byte offsets in JS; `start`/`end`
/// remain for the in-sentence term-highlight overlap test.
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
    /// comprehension indicator`).
    pub anki_filter_active: bool,
    /// Total terms before filtering (`base_terms`), for the file summary's
    /// "shown / known / total" counts.
    pub total_terms: usize,
    /// Terms hidden by the ignore list — the known-count hover breakdown.
    pub ignored_terms: usize,
}

/// Persisted `IgnoreFile` fields plus the display-only `exists` + `term_count`.
#[derive(Serialize, Deserialize, Clone)]
pub struct IgnoreFileView {
    pub path: String,
    pub enabled: bool,
    pub exists: bool,
    pub term_count: usize,
}

/// Full ignore-list state that hydrates the modal (`IgnoreListView`): the manual
/// lemma terms plus the file pills.
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

/// The engine keys `DictionaryState` by name; this folds the key in so the
/// modal gets a flat `{ name, weight, enabled }`.
#[derive(Serialize, Deserialize, Clone)]
pub struct DictionaryStateDto {
    pub name: String,
    pub weight: f32,
    pub enabled: bool,
}

/// `eta_secs` is smoothed backend-side (alpha=0.3, `null` until the first byte
/// lands); `current_file` is 1-based.
#[derive(Serialize, Deserialize, Clone)]
pub struct AnalysisProgressDto {
    pub total_files: usize,
    pub current_file: usize,
    pub message: String,
    pub total_bytes: u64,
    pub bytes_processed: u64,
    pub eta_secs: Option<f32>,
}

/// `reading` is `None` for pure kana; `count` aliases `frequency` so the UI
/// can label "occurrences" without re-deriving it.
#[derive(Serialize, Deserialize, Clone)]
pub struct AnalysisPreviewEntry {
    pub term: String,
    pub reading: Option<String>,
    pub frequency: u32,
    pub count: u32,
}

/// Only this lightweight view crosses IPC — the full result (for export) stays
/// in `AppState.last_analysis`. `total` is the unique-lemma count before the cap.
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
    /// ≥1 answers "default dict installed"; >1 answers "additional dicts installed".
    pub frequency_dict_count: usize,
    pub player_connected: bool,
}

/// Names the engine's positional `(JlptLevel, BandStats)` tuple — a bare tuple
/// lands as a JS array.
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

/// Names the engine's tuple vectors so TS deserializes objects, not arrays.
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

/// `status`: `"not-installed"` | `"installed"` (present, latest revision
/// unknown) | `"up-to-date"` | `"update-available"`.
#[derive(Serialize, Clone)]
pub struct RecommendedDictionaryDto {
    pub name: String,
    pub title: String,
    pub description: String,
    pub installed_revision: Option<String>,
    pub latest_revision: Option<String>,
    pub status: String,
}

/// One subtitle track of a bound media (issue #105, snake_case for the wire).
#[derive(Serialize, Clone)]
pub struct SubtitleTrackDto {
    pub track_number: u32,
    pub file_name: String,
}

/// Media asbplayer is tracking (`get-bound-media`), for the picker modal.
#[derive(Serialize, Clone)]
pub struct BoundMediaDto {
    pub id: String,
    /// `"streaming"` | `"local"`.
    pub media_type: String,
    pub title: Option<String>,
    pub favicon_url: Option<String>,
    pub loaded_subtitles: Vec<SubtitleTrackDto>,
    pub active: bool,
}

impl From<yomine::websocket::BoundMedia> for BoundMediaDto {
    fn from(m: yomine::websocket::BoundMedia) -> Self {
        Self {
            id: m.id,
            media_type: m.media_type,
            title: m.title,
            favicon_url: m.favicon_url,
            loaded_subtitles: m
                .loaded_subtitles
                .into_iter()
                .map(|t| SubtitleTrackDto { track_number: t.track_number, file_name: t.file_name })
                .collect(),
            active: m.active,
        }
    }
}

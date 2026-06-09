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

//! Wire DTOs (data-model.md). Domain types serialize directly when cheap; a
//! DTO exists only where the domain type is awkward on the wire.

use std::collections::{
    HashMap,
    HashSet,
};

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

/// `mine_term` outcome. `status`: `"created"` | `"duplicate"`; `warning` =
/// note created but asbplayer enrichment failed; `media_missing` = enrichment
/// verifiably didn't land (drives the retry chip).
#[derive(Serialize, Clone)]
pub struct MineResultDto {
    pub status: String,
    pub via: String,
    pub warning: Option<String>,
    pub note_id: Option<u64>,
    pub media_missing: bool,
}

/// Already-mined state (issue #3): `added:1` terms + normalized sentence keys.
#[derive(Serialize, Clone)]
pub struct MinedStateDto {
    pub added_terms: Vec<String>,
    pub mined_sentences: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct YomitanStatusDto {
    pub reachable: bool,
    pub version: Option<String>,
}

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
    /// Anki knowledge of the covering term (underline coloring, issue #94);
    /// `None` for segments no extracted term covers (particles, punctuation).
    pub knowledge: Option<SegmentKnowledge>,
}

/// Anki state of a term, least → most known (`Ord` picks the worst overlap).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SegmentKnowledge {
    Unknown,
    New,
    Young,
    Mature,
}

impl SegmentKnowledge {
    /// `in_anki` = the Anki filter matched the lemma; within that, comprehension
    /// encodes the card interval (0 = unreviewed, ≥1 = past the known interval).
    /// Outside Anki, comprehension 1.0 means ignored — user-declared known.
    fn classify(in_anki: bool, comprehension: f32) -> Self {
        match (in_anki, comprehension) {
            (true, c) if c >= 1.0 => Self::Mature,
            (true, c) if c <= 0.0 => Self::New,
            (true, _) => Self::Young,
            (false, c) if c >= 1.0 => Self::Mature,
            (false, _) => Self::Unknown,
        }
    }
}

/// One term occurrence as `(start, end, knowledge)` byte offsets.
pub type TermSpan = (usize, usize, SegmentKnowledge);

/// Group every term occurrence by sentence id. Expressions span their full
/// segment — must match `isTermSeg` in `SentenceView.svelte`.
pub fn term_spans_by_sentence(
    terms: &[Term],
    anki_lemmas: &HashSet<String>,
) -> HashMap<usize, Vec<TermSpan>> {
    let mut map: HashMap<usize, Vec<TermSpan>> = HashMap::new();
    for term in terms {
        let knowledge =
            SegmentKnowledge::classify(anki_lemmas.contains(&term.lemma_form), term.comprehension);
        let len = match term.part_of_speech {
            POS::Expression | POS::NounExpression => term.full_segment.len(),
            _ => term.surface_form.len(),
        };
        for (sentence_id, start) in &term.sentence_references {
            map.entry(*sentence_id).or_default().push((*start, start + len, knowledge));
        }
    }
    map
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
    pub fn from_sentence(s: &Sentence, term_spans: &[TermSpan]) -> Self {
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
                // Min over covering terms: an unknown compound outweighs a
                // known component word.
                knowledge: term_spans
                    .iter()
                    .filter(|(ts, te, _)| ts < end && te > start)
                    .map(|(_, _, k)| *k)
                    .min(),
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
    /// yomitan-api reachable (optional item — enables one-click mining).
    pub yomitan_connected: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn term(surface: &str, pos: POS, comprehension: f32, start: usize) -> Term {
        Term {
            id: 1,
            lemma_form: surface.to_string(),
            lemma_reading: String::new(),
            surface_form: surface.to_string(),
            surface_reading: String::new(),
            is_kana: false,
            part_of_speech: pos,
            frequencies: HashMap::new(),
            full_segment: surface.to_string(),
            full_segment_reading: String::new(),
            sentence_references: vec![(1, start)],
            comprehension,
        }
    }

    #[test]
    fn segment_knowledge_from_term_spans() {
        use SegmentKnowledge::*;

        // 気になる人がよく来る: expression 気になる (0..12) overlaps mature 気
        // (0..3); が (15..18) belongs to no term.
        let sentence = Sentence {
            id: 1,
            source_id: 3,
            text: "気になる人がよく来る".to_string(),
            segments: vec![
                ("キ".to_string(), POS::Noun, 0, 3),
                ("ニ".to_string(), POS::Postposition, 3, 6),
                ("ナル".to_string(), POS::Verb, 6, 12),
                ("ヒト".to_string(), POS::Noun, 12, 15),
                ("ガ".to_string(), POS::Postposition, 15, 18),
                ("ヨク".to_string(), POS::Adverb, 18, 24),
                ("クル".to_string(), POS::Verb, 24, 30),
            ],
            timestamp: None,
            comprehension: 0.0,
        };
        let terms = vec![
            term("気になる", POS::Expression, 0.2, 0), // not in Anki → Unknown
            term("気", POS::Noun, 1.0, 0),             // mature, masked by the expression
            term("人", POS::Noun, 0.0, 12),            // in Anki, unreviewed → New
            term("よく", POS::Adverb, 1.0, 18),        // ignored (not in Anki) → Mature
            term("来る", POS::Verb, 0.4, 24),          // in Anki, short interval → Young
        ];
        let anki: HashSet<String> = ["気", "人", "来る"].iter().map(|s| s.to_string()).collect();

        let spans = term_spans_by_sentence(&terms, &anki);
        let dto = SentenceDto::from_sentence(&sentence, &spans[&1]);

        let knowledge: Vec<Option<SegmentKnowledge>> =
            dto.segments.iter().map(|s| s.knowledge).collect();
        assert_eq!(
            knowledge,
            vec![
                Some(Unknown), // the unknown expression outweighs mature 気
                Some(Unknown),
                Some(Unknown),
                Some(New),
                None,
                Some(Mature),
                Some(Young),
            ]
        );
    }
}

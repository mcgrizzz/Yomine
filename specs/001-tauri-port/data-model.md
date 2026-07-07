# Data Model: IPC Boundary

Types that cross the Tauri boundary. Each MUST derive `serde::{Serialize, Deserialize}`
(Constitution III). Field names below are the JSON wire names (serde default = field name).
Source of truth for domain fields: `src/core/models.rs`, `src/segmentation/word.rs`,
`src/gui/settings/data.rs`, `src/tools/...`.

## Conventions

- Rust enums without data (e.g. `POS`) serialize to their variant name string by default
  (`"Noun"`, `"Verb"`, …). The frontend uses `POS::as_key()` values as stable keys and
  `POS::display_name()` for labels — expose both if the UI needs the human label, or ship a
  static POS metadata map once (see `PosInfo`).
- Floats `comprehension`, `coverage` are 0.0–1.0.
- `frequencies` maps a dictionary id (string) → rank (u32); the special key `"HARMONIC"` is the
  combined rank used for default ranking.
- Heavy runtime objects (`Tokenizer`, `FrequencyManager`, `IgnoreList`, `TaskManager`) are
  **never** serialized; they live in `tauri::State`.

## Term  (from `core::models::Term` — add serde)

| field | type | notes |
|-------|------|-------|
| id | u32 | |
| lemma_form | string | dictionary/base form |
| lemma_reading | string | hiragana |
| surface_form | string | as found in sentence |
| surface_reading | string | |
| is_kana | bool | |
| part_of_speech | POS (string) | |
| frequencies | map<string,u32> | dict id → rank; includes `"HARMONIC"` |
| full_segment | string | UI may omit |
| full_segment_reading | string | UI may omit |
| sentence_references | array<[usize, usize]> | (sentence_id, start_index) pairs |
| comprehension | f32 | 0–1 |

The frontend's term-table row is derived from this. `frequencies["HARMONIC"]` drives the
frequency column and band; `sentence_references.len()` gives the sentence-count column.

## Sentence  (from `core::models::Sentence` — add serde, but ship a DTO for the timestamp)

`Sentence.segments` is `Vec<(String /*reading*/, POS, usize /*start*/, usize /*end*/)>` over
`text`; the frontend maps each segment to a `<ruby>` span. The DTO ships each segment's
**pre-sliced `surface`** (so the UI never slices `text` by UTF-8 byte offset in JS) and its
`reading` **already converted to hiragana**; `start`/`end` (byte offsets) are retained for the
in-sentence term-highlight overlap test. `Sentence.timestamp` is `Option<TimeStamp>` wrapping
`time::Time` — do **not** serialize `time::Time`; expose a DTO:

```
SentenceDto {
  id: usize,
  source_id: u32,
  text: string,
  segments: array<{ surface: string, reading: string /* hiragana */, pos: POS, start: usize, end: usize }>,
  timestamp: { start_secs: f32, end_secs: f32, start_label: string, end_label: string } | null,
  comprehension: f32,
}
```

`start_secs/end_secs` come from `TimeStamp::to_secs`; labels from `TimeStamp::to_human_readable`.
`start_secs` is the value passed to `seek_timestamp` (FR-008).

## SourceFile  (from `core::models::SourceFile` — add serde)

| field | type | notes |
|-------|------|-------|
| id | u32 | |
| source | string \| null | e.g. "YouTube", "Jimaku" |
| file_type | SourceFileType (string-ish) | see below |
| title | string | |
| creator | string \| null | |
| original_file | string | absolute path |

`SourceFileType` is `SRT | SSA | TXT | Other(string)` — serialize as an externally-tagged enum
or flatten to `{ kind: "SRT" }` / `{ kind: "Other", value: "PDF" }`. UI only needs a label +
the supported flag (`is_supported`).

## POS metadata  (`PosInfo`, derived from `segmentation::word::POS`)

Ship once to the frontend so it can render filters/labels without hardcoding:

```
PosInfo { key: string /* as_key() */, display_name: string /* display_name() */ }
```

Provided by a `get_pos_catalog` command (static list). `pos_filters` in settings is keyed by
`as_key()`.

## Settings  (from `gui::settings::data` — already serde)

```
SettingsData {
  anki_model_mappings: map<string, FieldMapping>,   // note type → fields
  anki_interval: u32,                                // default 30
  websocket_settings: { port: u16 },                 // default 8766
  frequency_weights: map<string, { weight: f32, enabled: bool }>,
  pos_filters: map<string, bool>,                    // POS as_key() → shown
  use_serif_font: bool,
  dark_mode: bool,
  asbplayer_follow_new_media: bool,                  // issue #105 follow mode (default false)
  asbplayer_follow_active_tab: bool,                 // issue #105 follow mode (default false)
  asbplayer_poll_secs: u32,                          // follow-mode poll cadence (default 3)
  font_scale: f32,                                   // whole-UI scale, Tauri only (default 1.0)
}
FieldMapping { term_field: string, reading_field: string }   // see src/anki/types.rs
```

On-disk format is unchanged (`settings.json`), so existing users' settings load in both apps.

## Frequency dictionary state  (from `FrequencyManager::dictionary_states`)

```
DictionaryState { name: string, weight: f32, enabled: bool }
```

`list_dictionaries` returns `array<DictionaryState>`; `set_dictionary_state(name, weight,
enabled)` and `reload_dictionaries` mutate engine state and trigger a term/band recompute.

## Anki model info  (from `gui::settings::data::AnkiModelInfo` — add serde)

```
AnkiModelInfo { name: string, fields: array<string>, sample_note: map<string,string> | null }
```

Used by the Anki settings UI to populate note types/fields and guess mappings.

## Knowledge summary  (from `tools::knowledge_summary` — add serde)

```
BandStats { coverage: f32, comprehension: f32, total: usize }
KnowledgeSummary {
  jlpt: array<{ level: string /* "N5".. "N1" */, stats: BandStats }>,
  frequency: array<{ label: string /* "<1.5k".. */, stats: BandStats }>,
}
KnowledgeMode = "Coverage" | "Estimate"
```

`JlptLevel` serializes to its level label string. The UI toggle selects which `BandStats` field
to show.

## Frequency analyzer  (from `tools::analysis`)

```
AnalysisProgressDto {
  total_files: usize, current_file: usize, message: string,
  total_bytes: u64, bytes_processed: u64, eta_secs: f32 | null,
}
ExportOptions {
  dict_name: string, dict_author: string, dict_url: string, dict_description: string,
  revision_prefix: string, export_yomitan: bool, export_csv: bool,
  pretty_json: bool, exclude_hapax: bool,
}
FrequencyAnalysisResult { /* whatever export needs; ship a results-preview DTO:
  entries: array<{ term: string, reading: string, frequency: u32, count: u32 }>, total: usize */ }
```

`AnalysisProgressDto` is the streamed progress payload (R5). The full
`FrequencyAnalysisResult` stays in `AppState` for export; only a preview DTO goes to the UI.

## Ignore list  (modal DTOs, from `core::ignore_list` — see contracts/commands.md)

The modal stages edits locally and persists once via `save_ignore_list` (egui's
`temp_terms`/`temp_files` + "Save Settings"). `IgnoreFile` is the persisted shape; the
`*View` types add display-only metadata the modal renders.

```
IgnoreFile     { path: string, enabled: bool }                 // persisted (engine IgnoreFile)
IgnoreFileView { path, enabled, exists: bool, term_count: usize } // file pill (display-only)
IgnoreListView { terms: string[], files: IgnoreFileView[] }    // hydrates the modal
```

`term_count` is the file's line count (0 when missing/unreadable, matching egui's count map
which only records on a successful read). `save_ignore_list(terms, files: IgnoreFile[])` and
`export_ignore_list(terms)` take plain shapes; `import_ignore_file`/`refresh_ignore_file`
return an `IgnoreFileView`.

## Status payloads (events, see contracts/events.md)

```
AnkiStatus { connected: bool, fetching: bool }
PlayerStatus { mpv_connected: bool, ws_clients: usize, mode: "mpv" | "asbplayer" | "none", server_state: "running" | "starting" | "error" | "stopped", server_error: Option<String> }  // server_state/error added in T056
LoadingMessage { message: string | null }   // mirrors egui MessageOverlay
LanguageToolsStatus = "loading" | "ready" | { error: string }
```

## Notes on parity

- The egui `TableState` (`src/gui/table/state.rs`) defines the sortable/filterable derived
  fields. The frontend store mirrors those derivations from the `Term` payload above; no new
  fields are invented.
- Comprehension and frequency-band math are computed in Rust and arrive pre-computed; the
  frontend never recomputes them.

# Contract: Tauri Commands

The frontend↔backend command API. Each is a `#[tauri::command]` in `src-tauri/src/commands/`.
Types reference [../data-model.md](../data-model.md). This is the stable boundary
(Constitution III): adding fields/commands is additive; renaming/removing is breaking.

Conventions: commands are `async` and return `Result<T, String>` (error string surfaced to the
UI as today). Long-running commands accept a `tauri::ipc::Channel<…>` for progress and/or emit
events (see [events.md](./events.md)). Engine handles come from `tauri::State<AppState>`.

## Lifecycle / tools

| Command | Args | Returns | Maps to (egui) | Notes |
|---------|------|---------|----------------|-------|
| `load_language_tools` | `progress: Channel<LoadingMessage>` | `()` | `TaskManager::load_language_tools` | Loads tokenizer + freq dicts + ignore list into `AppState`; streams progress; emits `language-tools-status` ready/error. Call on app start. |
| `get_pos_catalog` | — | `array<PosInfo>` | `POS` static | Static POS key/label list for filters. |
| `get_settings` | — | `SettingsData` | `load_json` | From `AppState` (loaded at start). |
| `save_settings` | `settings: SettingsData` | `()` | `save_settings` | Persists via `persistence::save_json`; updates `AppState`; may trigger recompute (e.g. known-interval). |

## File / mining

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `open_file_dialog` | — | `string \| null` | `rfd`/FileModal | Via `tauri-plugin-dialog`; returns chosen path or null. |
| `open_video_dialog` | — | `string \| null` | issue #89 | Video-extension filter (+ All files); same dialog bridge as `open_file_dialog`. |
| `open_executable_dialog` | — | `string \| null` | issue #89 | "Locate mpv…" picker; `.exe` filter on Windows, unfiltered elsewhere. |
| `process_file` | `path: string`, `progress: Channel<LoadingMessage>` | `FileLoadResult` | `TaskManager::process_file` → `pipeline::process_source_file` | Parses, segments, filters (cached Anki), returns enriched terms + sentence DTOs + file comprehension. If Anki reachable, triggers background `refresh_terms` and emits `terms-refreshed`. |
| `get_terms` | — | `FileLoadResult \| null` | current `FileData` | Re-fetch current loaded state (e.g. on UI reload). |
| `refresh_terms` | — | `()` | `TaskManager::refresh_terms` | Live Anki re-filter + recompute comprehension; emits `terms-refreshed`. |
| `get_recent_files` | — | `array<RecentFile>` | `gui/recent_files.rs` | Reuse existing store/format (O3). |

`FileLoadResult = { source_file: SourceFile, terms: array<Term>, sentences: array<SentenceDto>,
file_comprehension: f32 }`.

`SegmentDto.knowledge: "unknown" | "new" | "young" | "mature" | null` (issue #94): the covering
term's Anki state for underline coloring — in-Anki membership from `anki_known_lemmas`, sub-state
from the interval-derived comprehension (0 = new, <1 = young, ≥1 = mature; ignored terms map to
mature). Worst state wins over overlapping terms (computed in `load_result` from `base_terms`, so
known/ignored words are covered too); `null` when no extracted term covers the segment. Paired
with `SettingsData.sentence_coloring: "knowledge" | "none"` (serde-defaulted to `knowledge`;
unknown values — e.g. the removed `"pos"` — deserialize to the default; Appearance modal) and
`SettingsData.sentence_underlines: { unknown, new, young, mature: bool }` (all default true;
per-state visibility checkboxes shown when knowledge mode is selected — filtering is
frontend-only, the DTO always carries the state).

`SettingsData.table_columns: array<{ id, visible }>` (issue #122): term-table column order and
visibility, edited via header right-click → "Edit columns…" (drag to reorder, untick to hide;
each change persists immediately); empty = built-in layout. The known id set lives
frontend-side (`normalizeColumns` in `table.ts`) — unknown ids drop, new columns append, Term is
always visible. Supersedes `show_jlpt_tags`, which now only seeds the jlpt default.

## Ignore list

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `get_ignore_list` | — | `array<string>` | `IgnoreList::get_all_terms` | Manual lemma forms only (not file-sourced). |
| `add_to_ignore_list` | `lemma: string` | `()` | row Ctrl+Click / context menu | Adds + persists only; **no re-filter** — the term stays visible-but-greyed until the next `refresh_terms` (T059, egui parity). |
| `remove_from_ignore_list` | `lemma: string` | `()` | row Ctrl+Click / context menu (un-ignore) | Removes + persists only; the un-ignored term stops being greyed (T059). Also retained for API completeness vs the modal's staged `save_ignore_list`. |
| `get_ignore_list_full` | — | `IgnoreListView` | `IgnoreListModal::open_modal` | Hydrates the modal: manual terms + file pills with per-file `exists` + `term_count`. |
| `import_ignore_file` | — | `IgnoreFileView \| null` | `FileAction::Add` | Opens a `.txt` open dialog (`tauri-plugin-dialog`), loads its terms, returns `{ path, enabled: true, exists, term_count }`; null if cancelled. Frontend pushes it to the staged file list. |
| `refresh_ignore_file` | `path: string` | `IgnoreFileView` | `FileAction::Refresh` | Re-reads a file's `term_count`/`exists` for display. The persisted cache reload happens on save. |
| `save_ignore_list` | `terms: array<string>`, `files: array<IgnoreFile>` | `FileLoadResult \| null` | "Save Settings" | `set_terms` + `set_files` + `reload_file_cache`, reapply filters, return updated terms (null if no file loaded). The modal's single commit point (staged Save/Cancel). |
| `get_default_ignored_terms` | — | `array<string>` | "Restore Default" | `DEFAULT_IGNORED_TERMS`. Frontend stages defaults + clears files; persisted on save. |
| `export_ignore_list` | `terms: array<string>` | `string \| null` | "Export…" / `export_terms` | Opens a `.txt` save dialog, writes the (possibly unsaved) staged terms newline-joined; returns the path or null if cancelled. |

`IgnoreFile = { path: string, enabled: boolean }` (the persisted shape; engine `IgnoreFile`).
`IgnoreFileView = IgnoreFile & { exists: boolean, term_count: number }` (display-only DTO).
`IgnoreListView = { terms: array<string>, files: array<IgnoreFileView> }`.

## Anki

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `get_anki_status` | — | `AnkiStatus` | `update_anki_status` | Also pushed periodically via `anki-status` event (R5). |
| `list_anki_models` | — | `array<AnkiModelInfo>` | anki settings modal | For mapping UI + field guessing. |

## Player

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `seek_timestamp` | `seconds: f32`, `label: string` | `()` | `PlayerManager::seek_timestamp` | Prefers MPV, else WebSocket; errors if no player. |
| `get_player_status` | — | `PlayerStatus` | `PlayerManager` | Also pushed via `player-status` event. |
| `set_websocket_port` | `port: u16` | `()` | websocket settings modal | Persists + restarts server. |
| `get_asbplayer_media` | — | `array<BoundMediaDto>` | issue #105 (T066) | asbplayer `get-bound-media` over the WS (extension v1.20+): id/type/title/favicon/tracks/active for the picker. Errors when not connected / no response (timeout hints at the version). |
| `load_asbplayer_media` | `media_id: string`, `track_numbers: array<u32> \| null`, `title: string`, `progress: Channel<LoadingMessage>` | `FileLoadResult` | issue #105 (T066) | asbplayer `get-subtitles` → cues become `Sentence`s (SRT-grade cleanup, cue timings preserved → seek/👁 work) → `process_sentences` (the same pipeline tail as `process_file`) → stored as the loaded file. NOT recorded in recent files. `null` tracks = all loaded tracks. |
| `launch_mpv` | `video_path: string` | `"launched" \| "not_found"` | issue #89 | Spawns `settings.mpv_path` with `--input-ipc-server=default_mpv_endpoint()`; detection auto-connects within ~1s. `not_found` (executable missing) drives the UI's "Locate mpv…" flow; errors while an mpv is already connected. |

## One-click mining (T077, issues #105/#3)

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `mine_term` | `term: string`, `surface: string` (the occurrence as tokenized — cloze/bold falls back to `term` when it isn't in the sentence), `sentence: string`, `timestamp_secs: f32 \| null`, `timestamp_label: string \| null`, `via: "asbplayer" \| "direct"`, `entry_index: usize \| null` (Yomitan entry to build the card from — the popover's per-definition mine; default first), `format_name: string \| null` (Yomitan term card format to render with; default first) | `MineResultDto { status: "created" \| "duplicate", via, warning }` | `commands/mining.rs` + `src/yomitan` | Card content from the user's Yomitan config (yomitan-api `/ankiCardFormats` + `/ankiFields`). The note is ALWAYS created by Yomine: `storeMediaFile` per returned media, then AnkiConnect `addNote` (tag `yomine`); duplicates return `status: "duplicate"` instead of erroring (and skip enrichment). `via: "asbplayer"` (frontend rule: player mode is asbplayer + client connected + row has a cue — same rule as seeking, NOT tied to how the file was loaded) then confirmed-seeks and sends WS `mine-subtitle` postMineAction 2 (update last card) so asbplayer attaches audio/screenshot to the fresh note; enrichment failure sets `warning` rather than failing the mine. |
| `get_mined_state` | — | `MinedStateDto { added_terms, mined_sentences }` | `anki::mined` | `added:1` note terms via the field mappings + the normalized sentence set (cache written during the `get_total_vocab` note pass, merged with fresh `added:1` sentences). Best-effort: Anki offline still returns cached sentences. |
| `get_yomitan_status` | `url: string \| null` | `YomitanStatusDto { reachable, version }` | `yomitan::get_version` | `url` overrides the saved setting so the modal can probe a staged value. |
| `get_card_formats` | — | `array<CardFormatDto { name, deck, model }>` | `yomitan::get_term_card_formats` | The user's Yomitan term card formats, Yomitan's order (first = default) — drives the popover's per-format mine/queue buttons. |
| `open_in_anki` | `note_id: u64` | `()` | `anki_api::gui_browse` | Opens Anki's card browser on `nid:<id>` — the mined ✓ chip's click action (session mines only; the note id comes from `mine_term`). |

`mine_term` also takes `progress: Channel<LoadingMessage>` and streams stage updates ("Rendering … with Yomitan…", "Creating Anki note…", "Adding audio & screenshot via asbplayer…") which the frontend surfaces as an updating toast. `get_anki_sample_note` gained `guessed_sentence` (engine `guess_sentence_field`: literal "Sentence" name → sentence-ish name that isn't audio/translation → first sample field whose content looks like a Japanese sentence).

Settings additions: `SettingsData.yomitan_url` (default `http://127.0.0.1:19633`), `FieldMapping.sentence_field: string \| null` (optional; enables sentence-level mined badges). Both serde-defaulted — existing `settings.json` loads unchanged. `SetupStatus` gains `yomitan_connected: bool` (optional checklist item; also gates the ⛏ button via the frontend's periodic `get_yomitan_status` probes).

## Frequency dictionaries

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `list_dictionaries` | — | `array<DictionaryState>` | `dictionary_states` | name/weight/enabled. |
| `set_dictionary_state` | `name: string`, `weight: f32`, `enabled: bool` | `()` | `apply_frequency_settings` | Updates engine; emits `dictionaries-changed`; UI re-fetches terms. |
| `get_recommended_dictionaries` | — | `array<RecommendedDictionaryDto>` | issue #100 (T064) | Repo-manifest catalog (baked fallback) + live `index_url` revision checks; resolves per-entry status (`not-installed`/`installed`/`up-to-date`/`update-available`) against loaded dicts; caches the catalog in AppState. |
| `install_recommended_dictionary` | `title: string`, `progress: Channel<LoadingMessage>` | `()` | issue #100 (T064) | Downloads to `.zip.part`, replaces same-title artifacts, then the shared `reload_and_swap` (weights + per-term re-bake + `dictionaries-changed`). |
| `remove_dictionary` | `title: string`, `progress: Channel<LoadingMessage>` | `()` | issue #100 (T064) | Deletes the extracted folder + source zip of any installed dict, drops its `frequency_weights` entry, then `reload_and_swap`. Removing the last dict re-downloads the engine default on reload. |
| `load_frequency_dictionaries` | `progress: Channel<LoadingMessage>` | `usize` | `frequency_utils::load_frequency_dictionaries` + `TaskManager::reload_frequency_dictionaries` | One command instead of the drafted import+reload pair (T060): native multi-`.zip` picker (backend-side, like `import_ignore_file`) → copy new archives → rebuild + swap the manager (weights reapplied; the loaded file's per-term frequencies re-baked via `build_freq_map`, so new dicts take effect immediately — deviation from egui, which needs the file reopened) → emit `dictionaries-changed`. Returns the newly-copied count; `0` = cancelled/nothing new (no reload, egui parity). |

## Frequency analyzer

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `start_analysis` | `paths: array<string>`, `progress: Channel<AnalysisProgressDto>` | `AnalysisPreview` | `TaskManager::analyze_frequency` | Runs analyzer; streams progress; returns a results preview (full result kept in `AppState`). |
| `cancel_analysis` | — | `()` | `cancel_task(FrequencyAnalysis)` | Flips the `AtomicBool` cancel token in `AppState`. |
| `export_analysis` | `output_dir: string`, `options: ExportOptions` | `string` | `TaskManager::export_frequency` | Writes Yomitan zip / CSV; returns a success message. |

## Knowledge summary

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `get_knowledge_summary` | — | `KnowledgeSummary \| null` | `compute_knowledge_summary` | Null until an Anki cache exists; recomputed in background and pushed via `knowledge-summary` event. |

## Misc

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `open_url` | `url: string` | `()` | `open::that` (setup checklist) | Via opener plugin. |
| `get_setup_status` | — | `SetupStatus` | `SetupBanner`/`SetupChecklist` | Aggregates anki/dict/player readiness for the checklist. |
| `check_for_update` | — | `UpdateInfo \| null` | — (new, T074) | Newest non-prerelease GitHub release vs the app version; `null` = up to date. `UpdateInfo = { current, latest, url }`. Called once at hydrate; failures are swallowed frontend-side. |

`SetupStatus = { tools_loaded: bool, anki_connected: bool, has_field_mapping: bool,
has_frequency_dict: bool, player_connected: bool }`.

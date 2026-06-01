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
| `process_file` | `path: string`, `progress: Channel<LoadingMessage>` | `FileLoadResult` | `TaskManager::process_file` → `pipeline::process_source_file` | Parses, segments, filters (cached Anki), returns enriched terms + sentence DTOs + file comprehension. If Anki reachable, triggers background `refresh_terms` and emits `terms-refreshed`. |
| `get_terms` | — | `FileLoadResult \| null` | current `FileData` | Re-fetch current loaded state (e.g. on UI reload). |
| `refresh_terms` | — | `()` | `TaskManager::refresh_terms` | Live Anki re-filter + recompute comprehension; emits `terms-refreshed`. |
| `get_recent_files` | — | `array<RecentFile>` | `gui/recent_files.rs` | Reuse existing store/format (O3). |

`FileLoadResult = { source_file: SourceFile, terms: array<Term>, sentences: array<SentenceDto>,
file_comprehension: f32 }`.

## Ignore list

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `get_ignore_list` | — | `array<string>` | `IgnoreList` | Lemma forms. |
| `add_to_ignore_list` | `lemma: string` | `FileLoadResult` | right-click action + `partial_refresh` | Adds + reapplies filters (no Anki connection) and returns updated terms. |
| `remove_from_ignore_list` | `lemma: string` | `FileLoadResult` | ignore-list modal | Removes + reapplies. |

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

## Frequency dictionaries

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `list_dictionaries` | — | `array<DictionaryState>` | `dictionary_states` | name/weight/enabled. |
| `set_dictionary_state` | `name: string`, `weight: f32`, `enabled: bool` | `()` | `apply_frequency_settings` | Updates engine; emits `dictionaries-changed`; UI re-fetches terms. |
| `load_frequency_dictionaries` | `paths: array<string>`, `progress: Channel<LoadingMessage>` | `()` | `frequency_utils::load_frequency_dictionaries` | Import zips. |
| `reload_dictionaries` | `progress: Channel<LoadingMessage>` | `array<DictionaryState>` | `TaskManager::reload_frequency_dictionaries` | Rebuilds manager; emits `dictionaries-changed`. |

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

`SetupStatus = { tools_loaded: bool, anki_connected: bool, has_field_mapping: bool,
has_frequency_dict: bool, player_connected: bool }`.

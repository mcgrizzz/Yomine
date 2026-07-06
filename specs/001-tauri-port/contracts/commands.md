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

## Frequency dictionaries

| Command | Args | Returns | Maps to | Notes |
|---------|------|---------|---------|-------|
| `list_dictionaries` | — | `array<DictionaryState>` | `dictionary_states` | name/weight/enabled. |
| `set_dictionary_state` | `name: string`, `weight: f32`, `enabled: bool` | `()` | `apply_frequency_settings` | Updates engine; emits `dictionaries-changed`; UI re-fetches terms. |
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

`SetupStatus = { tools_loaded: bool, anki_connected: bool, has_field_mapping: bool,
has_frequency_dict: bool, player_connected: bool }`.

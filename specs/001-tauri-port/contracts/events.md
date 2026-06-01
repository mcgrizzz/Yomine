# Contract: Events & Channels

Push messages from backend → frontend. These replace egui's per-frame `poll_results()` +
`update()` polling (research R5). Two mechanisms:

- **Channels** (`tauri::ipc::Channel<T>`): per-call progress streams, passed as a command arg.
  Used for operations the caller initiated and wants progress on.
- **Global events** (`app.emit(name, payload)` / frontend `listen(name, cb)`): ambient state
  changes any view may care about, driven by the background task.

## Channels (per-operation progress)

| Channel arg | Payload | On command | Replaces |
|-------------|---------|------------|----------|
| `progress: Channel<LoadingMessage>` | `{ message: string \| null }` | `load_language_tools`, `process_file`, `reload_dictionaries`, `load_frequency_dictionaries` | `TaskResult::LoadingMessage` + `MessageOverlay` |
| `progress: Channel<AnalysisProgressDto>` | see data-model | `start_analysis` | `FrequencyAnalysisUpdate::Progress` |

A `null` `message` clears the overlay (mirrors `MessageOverlay::clear_message`).

## Global events (ambient state)

| Event name | Payload | Emitted when | Replaces (egui) |
|------------|---------|--------------|-----------------|
| `language-tools-status` | `LanguageToolsStatus` | tools finish loading or fail | `TaskResult::LanguageToolsLoaded` |
| `anki-status` | `AnkiStatus` | background poll (~5s) detects a change | `update_anki_status` + `TaskResult::AnkiConnection` |
| `player-status` | `PlayerStatus` | MPV/WebSocket connectivity or mode changes | `PlayerManager::update` (per-frame) |
| `terms-refreshed` | `FileLoadResult` | live Anki refresh completes | `TaskResult::TermsRefreshed` |
| `dictionaries-changed` | `array<DictionaryState>` | dictionaries reloaded or states changed | `TaskResult::FrequencyDictionariesReloaded` |
| `knowledge-summary` | `KnowledgeSummary` | background recompute finishes | `TaskResult::KnowledgeSummary` |
| `analysis-complete` | `AnalysisPreview` | analyzer finishes (non-cancelled) | `FrequencyAnalysisUpdate::Complete` |
| `analysis-cancelled` | `()` | analyzer cancelled | `FrequencyAnalysisUpdate::Cancelled` |
| `export-complete` | `{ ok: bool, message: string }` | export finishes | `TaskResult::FrequencyExport` |
| `error` | `{ title: string, message: string, detail: string \| null }` | a backend op fails in a way the UI should modal | `ErrorModal` |

## Background task (single spawned loop, started at setup)

Mirrors the work egui did each frame in `YomineApp::update`:

1. **Anki status poll** — every ~5s, check AnkiConnect; emit `anki-status` on change. (Was
   `update_anki_status`, throttled 5s.)
2. **Player mode switch** — drive `PlayerManager::update`: prefer MPV when present, otherwise
   ensure the WebSocket server is running; emit `player-status` on change. (Was per-frame.)
3. **Knowledge summary** — when an Anki vocab cache exists and inputs changed, recompute and
   emit `knowledge-summary`. (Was `maybe_compute_knowledge_summary`, throttled.)

The loop holds `AppHandle` + `State` and respects the same throttles as egui so behavior matches.

## Frontend subscription model

- On startup: `listen` to all global events; invoke `load_language_tools` with a progress
  channel; then `get_settings`, `get_pos_catalog`, `get_anki_status`, `get_player_status`,
  `get_knowledge_summary`, `get_recent_files` to hydrate stores.
- Stores: `termsStore` (from `process_file`/`get_terms`/`terms-refreshed`), `settingsStore`,
  `statusStore` (anki/player/tools), `overlayStore` (loading message), `knowledgeStore`,
  `analyzerStore`. Components are pure functions of these stores.

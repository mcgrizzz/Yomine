# Research: Yomine Tauri Port

Decisions, rationale, and alternatives for the re-platform. Grounded in the current codebase.

## R1. Frontend framework — SvelteKit (static adapter)

**Decision**: SvelteKit + TypeScript, built with `@sveltejs/adapter-static` (full
prerender/SPA), served by Tauri as static assets. No Node runtime ships.

**Rationale**: Chosen by the maintainer. Smallest bundles and least boilerplate of the
candidates; reactivity model fits a single-window app with a few large reactive stores (term
list, settings, status). Good Tauri community support.

**Alternatives considered**: React+TS (largest ecosystem, heavier), SolidJS (great perf, smaller
community), Leptos/Dioxus (stay in Rust, but immature ecosystem and the entire UI would still
be rewritten). Rejected in favor of the maintainer's choice.

**Notes**: Use SvelteKit SPA mode (`export const prerender = true` / `ssr = false`) so there is
no server. `adapter-static` outputs to a dir Tauri serves via `frontendDist`.

## R2. Tauri version — v2

**Decision**: Tauri v2.

**Rationale**: Current stable; capability/permission model, `tauri::ipc::Channel` for streaming
progress, and the plugin split (dialog, fs, opener) are v2 features we rely on. `cargo-tauri` is
not yet installed in the dev environment — install `tauri-cli` (v2) during Phase B.

**Plugins**: `tauri-plugin-dialog` (open-file dialog, replaces `rfd`), `tauri-plugin-opener` or
`tauri-plugin-shell` (replaces the `open` crate for opening URLs in the setup checklist),
`tauri-plugin-fs` only if the frontend needs direct file reads (most file IO stays in Rust).

## R3. Decoupling the engine from egui

**Problem (verified)**: Non-UI code reaches back into `gui` in exactly three places:
- `src/core/pipeline.rs` → `gui::LanguageTools`
- `src/core/tasks/manager.rs` → `gui::LanguageTools` / `gui::app::LanguageTools`
- `src/player/mod.rs` → `gui::websocket_manager::WebSocketManager`

`LanguageTools` is defined in `src/gui/app/mod.rs`; `WebSocketManager` in
`src/gui/websocket_manager.rs`. All other `crate::gui::` references are gui→gui (fine).

**Decision**:
- Move `LanguageTools` to `src/core/language_tools.rs`, re-export from `core`. It holds
  `Arc<Tokenizer>`, `Arc<FrequencyManager>`, `Arc<Mutex<IgnoreList>>`, `known_interval` — no UI
  types — so the move is mechanical.
- Move `WebSocketManager` to `src/websocket/manager.rs` (next to the server it wraps). Update
  `player/mod.rs` and the gui websocket settings modal import.
- Feature-gate the `gui` module and egui-family deps (`eframe`, `egui_extras`, `egui_flex`,
  `egui_double_slider`, `egui_ltreeview`, `rfd`) behind a default-on `gui` feature. `TaskManager`
  is egui-plumbing (mpsc/poll) and stays gated under `gui`; the Tauri app gets its own command
  layer instead.

**Alternative considered**: Extract a separate `yomine-core` crate. Rejected for now — moving
~8k LOC across a crate boundary is churn and risk; feature-gating achieves UI-independence
(Principle IV) with far less movement. Can be done later as cleanup if desired.

## R4. Making types crossable (serde)

**Decision**: Derive `Serialize`/`Deserialize` on the IPC types. Current state:
- `Term`, `Sentence`, `SourceFile`, `SourceFileType`, `TimeStamp` (in `core/models.rs`) derive
  only `Debug, Clone` → add serde.
- `POS` (in `segmentation/word.rs`) derives `Deserialize` only → add `Serialize`. It is a
  fieldless enum; default serde gives string variant names, which is fine and stable.
- `FilterResult` (`core/pipeline.rs`) and the analyzer/knowledge result types (`tools/...`) →
  add serde to whatever is returned to the frontend.

**`TimeStamp` / `time::Time`**: `Sentence` timestamps wrap `time::Time`. Options: (a) enable the
`time` crate's `serde` feature and derive, or (b) custom (de)serialize, or (c) the simplest —
serialize timestamps to the frontend as the `(start_secs, end_secs)` floats and human-readable
strings the UI actually needs (the engine already has `TimeStamp::to_secs` and
`to_human_readable`). **Chosen**: (c) for the wire DTO — the frontend never needs `time::Time`,
only seconds for seeking and a label for display. Keep `time::Time` internal.

**Non-serializable by design**: `Tokenizer`, `FrequencyManager`, `IgnoreList`, `TaskManager`
never cross the boundary; they live in `tauri::State`.

**DTO vs. direct serialize**: Where a domain type carries fields the UI doesn't need (e.g.
`Term.full_segment*`, internal indices) we may expose a trimmed DTO instead of serializing the
domain type directly. `data-model.md` specifies the wire shape per type. Default: serialize
domain types directly when cheap; introduce a DTO only when the domain type is awkward on the
wire (e.g. `TimeStamp`).

## R5. Async model — commands + events replace frame-poll

**Problem**: egui drives everything from `update()` each frame: `TaskManager::poll_results()`
drains an mpsc channel; `PlayerManager::update()` switches asbplayer/MPV modes; Anki status is
polled every 5s; the knowledge summary is recomputed opportunistically.

**Decision**:
- Each `TaskManager` operation becomes an async `#[tauri::command]`. Long-running ones
  (`load_language_tools`, `process_file`, `refresh_terms`, `analyze_frequency`,
  `compute_knowledge_summary`) stream progress over a `tauri::ipc::Channel<Progress>` argument
  or via `app.emit("…", payload)`; the frontend subscribes with `listen`/channel `onmessage`.
- The frame-driven background loops move to a single spawned async task started at setup: a
  ~5s Anki status poll, the MPV/WebSocket mode switch (currently `PlayerManager::update`), and
  the knowledge-summary recompute trigger. Each emits an event when state changes.
- Cancellation (frequency analyzer) keeps the existing `Arc<AtomicBool>` token, owned by
  `AppState` and flipped by a `cancel_analysis` command.

**Rationale**: This is idiomatic Tauri and removes the busy per-frame poll. State changes become
push events the reactive frontend consumes.

## R6. Where sort/filter/search runs

**Decision**: The engine returns the fully enriched term list once (per load/refresh). The
frontend holds it in a store and does sort/filter/search/derive reactively in Svelte. Frequency
bands, comprehension, and ranking are computed in Rust and included in the payload.

**Rationale**: Term counts are hundreds to low thousands; client-side derivation is simpler and
snappier than round-tripping per interaction, and keeps Principle II intact because no analysis
is reimplemented — only presentation-level ordering/filtering of already-computed values. The
current egui `TableState` (sort.rs/filter.rs/search.rs) is the behavior spec to mirror.

**Virtualization**: Use a virtual list (e.g. `@tanstack/svelte-virtual` or
`svelte-virtual-list`) for the table body to stay responsive on large files (SC-006).

## R7. Furigana / sentence rendering

**Decision**: Render readings with HTML `<ruby>`/`<rt>` over Japanese text. The engine already
produces `Sentence.segments` as `(reading, POS, start, end)` spans over the sentence text — the
frontend maps each span to a ruby element. This is a concrete win of the port (egui hand-draws
this; the browser does it natively).

## R8. Fonts, icon, and bundled data

**Decision**:
- Noto Sans/Serif JP (currently `include_bytes!` in `gui/app/mod.rs`) move to the frontend as
  `@font-face` web fonts; the serif/sans toggle becomes a CSS class.
- The app icon (currently `include_bytes!` in `main.rs`) is configured in `tauri.conf.json`.
- `assets/jlpt_vocab.json` is **embedded at compile time** (`include_str!` in
  `src/jlpt/mod.rs`), consumed by the engine, not the UI. It travels inside the `yomine` crate
  that the Tauri backend depends on, so it needs **no** Tauri resource bundling and no path
  resolution — leave as-is. (O1 resolved.)
- Runtime downloads (unidic via `ensure_dictionary`, frequency dicts, Anki cache) are unchanged
  and resolve to `dirs::data_local_dir()/yomine`.

## R9. Settings & persistence

**Decision**: Reuse the `persistence` module (`save_json`/`load_json`, already serde + same
app-data dir) directly from Tauri commands. `SettingsData` already derives serde. Settings are
owned by `AppState`; `get_settings`/`save_settings` commands mirror to the frontend. No change
to on-disk format → existing users' `settings.json` keeps working in both apps.

## Open Items (carry into tasks / clarify during Phase A)

- ~~**O1**: Confirm `jlpt_vocab.json` load path.~~ **Resolved**: embedded via `include_str!`;
  no bundling needed. (Note: `JlptLevel`/`JlptEntry` derive only `Deserialize` today — add
  `Serialize` to `JlptLevel` for the knowledge-summary DTO, see T010.)
- **O2**: Drag-and-drop — use Tauri's `onDragDropEvent` (file paths) to mirror egui's dropped
  files; confirm it delivers absolute paths on all platforms.
- **O3**: Recent-files storage — confirm where egui persists recent files (`gui/recent_files.rs`)
  and reuse the same store/format.
- **O4**: Final call (Phase D): retire the egui crate/feature or keep it gated for a transition.

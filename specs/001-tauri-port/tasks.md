---
description: "Task list for the Yomine Tauri + SvelteKit port"
---

# Tasks: Yomine on a Web-Based UI Shell (Tauri Port)

**Input**: Design documents in `specs/001-tauri-port/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Reuse the existing `cargo test` suite (esp. `src/tests/segmentation_regression.rs`)
as the analysis-parity guardrail. No new unit-test suite is requested; parity is verified
against the egui app per quickstart.md. New tests are added only where noted.

**Organization**: Phases A→D from plan.md. Phase A (Setup + Foundational) blocks everything.
Phase B (IPC foundation) blocks the UI stories. Phase C is organized by user story (US1–US7)
so each is independently demoable against egui. `[P]` = parallelizable (different files).

## Progress (resume pointer)

- **Phase A (Decouple engine) — ✅ COMPLETE & VERIFIED** (T001–T012). Engine builds
  `cargo check --no-default-features` (no egui compiled); egui app builds (`cargo check`);
  `cargo test` green (6 pass). Toolchain: rustc **1.96** (egui 0.34.3 needs ≥1.92).
- **Phase B scaffold — DONE, COMPILE-BLOCKED on system libs:**
  - `tauri-cli` v2.11.2 installed. `src-tauri` crate scaffolded (T013): `Cargo.toml`
    (`yomine { default-features = false }`, tauri 2 + dialog/opener plugins), `build.rs`,
    `src/{main,lib}.rs`, `tauri.conf.json` (window 1400×805, frontendDist `ui/build`,
    devUrl :5173, beforeDev/Build → `pnpm` in `ui`), `capabilities/default.json`, icon set
    in `icons/` (generated from a squared `assets/icon.png`). Root `Cargo.toml` now lists
    `members = [".", "src-tauri"]`.
  - SvelteKit frontend scaffolded under `src-tauri/ui` (T014): adapter-static SPA
    (`+layout.ts` prerender/ssr=false), vite tuned for Tauri (:5173), `pnpm install` +
    `pnpm build` **green** → `ui/build/index.html` produced. Placeholder `+page.svelte`.
  - Plugins added in code (T015): `tauri-plugin-dialog`/`-opener` registered in `lib.rs`;
    capabilities grant `core:default`, `dialog:default`, `opener:default`.
- **✅ BLOCKER RESOLVED.** WebKitGTK dev libs installed by the user (Ubuntu 22.04). A second,
  upstream issue surfaced: cargo greedily resolved `tauri-runtime` 2.11.2 / `wry` 0.53.5, which
  are incompatible with `tauri-runtime-wry` 2.9.3 (newest compatible with `tauri` 2.9.5 on
  Linux) — E0046 (`eval_script_with_callback` not implemented) + E0277 (dropped `Sync` bound).
  **Fix:** pinned the whole stack to the matched 2.11.2 set —
  `tauri-runtime = "=2.11.2"` / `wry = "=0.55.1"` (with `tauri` resolving to 2.11.2,
  `tauri-runtime-wry` 2.11.2, `webkit2gtk` 2.0.2) in `src-tauri/Cargo.toml` (commented; the
  three runtime crates must bump together). Build matrix **all green**: `cargo build -p yomine-tauri` ✓;
  `cargo build -p yomine --no-default-features` ✓; `cargo build -p yomine` (egui) ✓.
- **Settings decoupling — DONE** (prerequisite for T016/T019/T040, mirrors T004/T006):
  `SettingsData`/`WebSocketSettings`/`FrequencyDictionarySetting`/`AnkiModelInfo` moved from
  `gui/settings/data.rs` to new neutral `src/core/settings.rs`, re-exported from `core` and
  from `gui::settings::data` (so all gui call sites still resolve). `AnkiModelInfo` gained
  serde. `ModelMappingEditor` (egui editor state) stayed in gui. Engine builds gui-off ✓;
  egui builds gui-on ✓.
- **IPC foundation — DONE (T016–T018), compiles green.** `src-tauri/src/{state,events,dto}.rs`
  written and wired into `lib.rs` (manages `Mutex<AppState>`). `cargo build -p yomine-tauri` ✓
  (26 transient "field never read" warnings — `AppState`/DTO fields are consumed by the command
  layer in T019–T021; they clear as commands land).
- **Player isolation — DONE** (`src-tauri/src/player_task.rs`, maintainer-approved; see T016
  note). Spawned in `lib.rs` setup; `PlayerHandle` managed in tauri state. This covers the
  **player half of T020** (mode switch + `player-status` emit) and the backend for T035 (seek)
  / T041 (`set_websocket_port` → `PlayerHandle::set_port`). Added `tokio`
  (`sync`/`time`/`macros`) to `src-tauri/Cargo.toml`. Compiles green (27 transient dead-code
  warnings — consumed by the command layer).
- **Command layer + frontend round-trip — DONE (T019–T024), all builds green.** New files:
  `src-tauri/src/commands/{mod,lifecycle,file}.rs`, `src-tauri/src/background.rs`,
  `src-tauri/ui/src/lib/ipc.ts`, `src-tauri/ui/src/lib/stores/index.ts`; `+page.svelte` is now a
  real (placeholder) round-trip surface. Core change: added canonical `POS::all()` in
  `src/segmentation/word.rs`. `AppState` gained `knowledge_dirty: Arc<AtomicBool>` and `FileData`
  gained `base_terms`. Build matrix: `cargo build -p yomine-tauri` ✓ (13 transient dead-code
  warnings — later-task commands consume them); `cargo build -p yomine --no-default-features` ✓;
  `cargo build -p yomine` (egui) ✓ (3 pre-existing warnings); `cargo test` ✓ (6 pass);
  `pnpm build` ✓ → `ui/build` produced.
- **NEXT: T025 verify + Phase C shell (T026).** T025 is code-complete but needs an **interactive
  `cargo tauri dev`** run (load an SRT → confirm the term count renders) to tick — I can't drive
  the GUI from here. Then start Phase C: T026 app shell/theme (consume `settings.dark_mode`),
  T028 top bar, T029 `TermTable` (replace the placeholder `<ul>` in `+page.svelte`).
- **Deferred (tracked, intentional):**
  - Auto-`refresh_terms`/`terms-refreshed` on a live Anki connection → **US2/T033** (backend keeps
    `base_terms` ready; `onTermsRefreshed` listener already wired in the store).
  - `analyzer` store → **US6/T047**. Remaining commands (anki/player/dict/analyzer/knowledge/misc
    wrappers) land with their UI tasks and append to the `invoke_handler` list in `lib.rs`.
  - **Pre-existing scaffold debt (T014, not mine):** `pnpm check` reports 4 errors in
    `vite.config.ts` (`process` global needs `@types/node`). Doesn't block `pnpm build`; fix before
    T053 (CI) by adding `@types/node` or `import process from 'node:process'`.
- Engine refs confirmed: `pipeline::process_source_file` → `(Vec<Term>, FilterResult,
  Vec<Sentence>, f32)`; `init_vibrato(&DictType::Unidic, cb)` / `process_frequency_dictionaries(cb)`
  / `IgnoreList::load()` (loader trio); `anki::api::get_version()` + `anki::has_cached_vocab()`;
  `knowledge_summary::compute_knowledge_summary(Arc<FrequencyManager>, u32)`;
  `FrequencyManager::{dictionary_states, set_dictionary_state}` (interior-mutable, `&self`);
  `persistence::{load_json_or_default, save_json}`; `POS::{all, as_key, display_name}`.
- Pre-existing (not part of this work): `src/tests/segmentation_regression.rs` is orphaned
  (nothing declares `mod tests;`, so it never runs); 3 egui `Panel::show` deprecation warnings.

## Format: `[ID] [P?] [Story] Description`

---

## Phase A: Decouple the engine (Setup + Foundational) ⚠️ BLOCKS ALL

**Goal**: Engine compiles with no UI framework; egui still default-builds and runs unchanged;
all IPC types serialize. Verifies the three-command build matrix in quickstart.md.

- [x] T001 Convert the repo to a Cargo workspace: add `[workspace] members = [".", "src-tauri"]`
      (src-tauri added in Phase B) to root `Cargo.toml`; keep the `yomine` crate as-is otherwise.
- [x] T002 Add a default-on `gui` feature in `Cargo.toml`; move egui-family deps (`eframe`,
      `egui_extras`, `egui_flex`, `egui_double_slider`, `egui_ltreeview`, `rfd`) to
      `optional = true` and into the `gui` feature's dependency set.
- [x] T003 Gate the egui surface: `#[cfg(feature = "gui")] pub mod gui;` in `src/lib.rs`; make
      the egui binary (`src/main.rs`) `required-features = ["gui"]` (or gate its body).
- [x] T004 Move `LanguageTools` from `src/gui/app/mod.rs` to new `src/core/language_tools.rs`;
      re-export from `core` (`pub use language_tools::LanguageTools;`). Pure data move (holds
      `Arc<Tokenizer>`, `Arc<FrequencyManager>`, `Arc<Mutex<IgnoreList>>`, `known_interval`).
- [x] T005 Update `LanguageTools` importers to `core::LanguageTools`: `src/core/pipeline.rs`,
      `src/core/tasks/manager.rs`, and the gui re-export sites.
- [x] T006 Move `WebSocketManager` from `src/gui/websocket_manager.rs` to
      `src/websocket/manager.rs`; update `src/player/mod.rs` and
      `src/gui/settings/websocket_settings_modal.rs` imports.
- [x] T007 Gate `src/core/tasks/` (the mpsc/poll `TaskManager`) behind the `gui` feature — it is
      egui plumbing; the Tauri app uses its own command layer. Adjust `core/mod.rs` exports.
- [x] T008 [P] Add `serde::{Serialize, Deserialize}` to `Term`, `Sentence`, `SourceFile`,
      `SourceFileType` in `src/core/models.rs`. For `Sentence.segments` ensure `POS` is
      serializable (T009). Do NOT serialize `time::Time` — handled by the DTO in T030.
- [x] T009 [P] Add `Serialize` to `POS` in `src/segmentation/word.rs` (already `Deserialize`).
      Confirm default variant-name encoding matches `as_key()` or add `#[serde(rename=...)]`.
- [x] T010 [P] Add serde to `FilterResult` (`src/core/pipeline.rs`), `AnkiModelInfo` &
      `FieldMapping` (`src/gui/settings/data.rs`, `src/anki/types.rs`), `BandStats` /
      `KnowledgeSummary` / `KnowledgeMode` (`src/tools/knowledge_summary.rs`), `JlptLevel`
      (`src/jlpt/mod.rs` — add `Serialize`, currently `Deserialize`-only), and the analyzer
      result/progress/export types (`src/tools/analysis/models.rs`) needed by the UI.
- [x] T011 ~~Resolve O1~~ DONE: `assets/jlpt_vocab.json` is embedded via `include_str!` in
      `src/jlpt/mod.rs` — no Tauri bundling/path work needed. (Recorded in research.md.)
- [x] T012 **Verify Phase A**: `cargo build` (egui) ✓; `cargo build -p yomine
      --no-default-features` ✓; `cargo test` ✓; `cargo run` egui app behaves unchanged.

**Checkpoint**: Engine is UI-independent and serializable; egui untouched behaviorally.

---

## Phase B: Tauri scaffold + IPC foundation ⚠️ BLOCKS UI STORIES

**Goal**: A Tauri window that completes one real round trip (open file → term count), with the
command/event layer and background loop in place. Implements contracts/.

- [x] T013 Install Tauri tooling: `cargo install tauri-cli --version "^2"`. Scaffold `src-tauri`
      (Tauri v2) as a workspace member depending on `yomine { default-features = false }`.
      (Done; build verification pending WebKitGTK system libs — see Progress blocker.)
- [x] T014 Scaffold SvelteKit under `src-tauri/ui` with `@sveltejs/adapter-static` in SPA mode
      (`prerender`/`ssr=false`); wire `tauri.conf.json` `build.frontendDist`/`devUrl` and
      `beforeDevCommand`/`beforeBuildCommand` to pnpm. (`pnpm build` green.)
- [x] T015 Add Tauri plugins: `tauri-plugin-dialog` (file open), `tauri-plugin-opener` (open URL),
      `tauri-plugin-fs` if needed; register capabilities/permissions. (dialog+opener registered;
      fs deferred until a command needs it.)
- [x] T016 Implement `AppState` in `src-tauri/src/state.rs`: `Mutex`-guarded struct holding
      `Option<LanguageTools>`, `SettingsData`, current `FileData`
      (terms/sentences/comprehension), analyzer cancel token + last `FrequencyAnalysisResult`.
      (Managed as `Mutex<AppState>` in `lib.rs`; settings loaded from `settings.json` at start.)
      **Design change (maintainer-approved):** `PlayerManager` is NOT in `AppState` — it is
      owned solely by a dedicated task (`src-tauri/src/player_task.rs`) and reached via a
      channel handle (`PlayerHandle`, managed separately). Rationale: `PlayerManager::update()`
      does blocking-ish I/O on a ~250ms timer (socket reconnect, WS server restart); isolating
      it means a player tick never contends with the state lock. `seek`/`status` carry a
      `oneshot` reply; `set_port` is fire-and-forget. The task emits `player-status` on change.
- [x] T017 Implement `src-tauri/src/events.rs`: event-name constants + payload structs mirroring
      contracts/events.md (`AnkiStatus`, `PlayerStatus`, `LoadingMessage`, `FileLoadResult`, …).
      (`names` const module; `LanguageToolsStatus` serializes to the TS union shape.)
- [x] T018 Define the `SentenceDto` + `FileLoadResult` mapping in the backend (data-model.md):
      convert `Sentence` → `SentenceDto` (timestamp → secs+labels via `TimeStamp::to_secs`/
      `to_human_readable`). (In `src-tauri/src/dto.rs`; also `SegmentDto`, `TimeStampDto`, `PosInfo`.)
- [x] T019 Implement lifecycle commands (`src-tauri/src/commands/lifecycle.rs`):
      `load_language_tools` (Channel progress, loads into AppState, emits
      `language-tools-status`), `get_pos_catalog`, `get_settings`, `save_settings`. (Added a
      canonical `POS::all()` in core for the catalog; weights propagate via shared
      `commands::apply_frequency_weights`, mirroring egui's `apply_frequency_settings`.)
- [x] T020 Implement the background task (`src-tauri/src/background.rs`): ~5s Anki poll →
      `anki-status`; knowledge summary recompute → `knowledge-summary`. (Player mode switch is
      handled by `player_task` already; not duplicated here.) Started in `lib.rs` setup with the
      same 5s throttle egui used; recompute is gated by `AppState.knowledge_dirty` (set on
      settings save, mirroring egui's `knowledge_summary_attempted` reset).
- [x] T021 Implement `process_file` + `get_terms` (`commands/file.rs`) calling
      `pipeline::process_source_file`; return `FileLoadResult`. `open_file_dialog` via dialog
      plugin. **Deferred to US2/T033:** the auto-`refresh_terms`/`terms-refreshed` on a live Anki
      connection — `FileData` now keeps `base_terms` so that lands cleanly later.
- [x] T022 Register commands in `lib.rs` invoke_handler (`main.rs` is the thin entry that calls
      `run()`); `AppState` managed, background + player tasks started. The handler currently lists
      the 7 implemented commands and grows as US2–US7 commands land.
- [x] T023 [P] Frontend IPC layer `src-tauri/ui/src/lib/ipc.ts`: typed wrappers over
      `invoke`/`listen`/`Channel`; TS types mirror data-model.md. (Only the 7 registered commands
      are wrapped, so startup hydration can't call a missing command.)
- [x] T024 [P] Frontend stores `src-tauri/ui/src/lib/stores/index.ts`: `languageToolsStatus`,
      `ankiStatus`, `playerStatus`, `overlay`, `fileResult`, `knowledge`, `settings`,
      `posCatalog`, `lastError` + a `hydrate()` that wires events and loads tools.
      (`analyzer` store deferred to US6/T047 — nothing hydrates it at startup.)
- [ ] T025 **Verify Phase B**: `cargo tauri dev` opens; startup loads tools (progress shows);
      `process_file` on a real SRT returns a term count rendered in a placeholder list.
      (Code complete + builds green; **needs an interactive `cargo tauri dev` run** to tick.)

**Checkpoint**: IPC foundation proven end-to-end; UI stories can proceed in parallel.

---

## Phase C: Port the UI to parity

Each story is verified against the egui app (quickstart.md). Shell first (T026–T028) since all
stories render inside it.

### Shell (prerequisite for all stories)

- [ ] T026 App shell + theme `src-tauri/ui/src/routes/+page.svelte` + `app.css`: dark/light
      theme (from `settings.dark_mode`), layout matching egui's information architecture.
- [ ] T027 [P] Fonts: bundle Noto Sans/Serif JP as `@font-face`; serif/sans toggle → CSS class
      driven by `settings.use_serif_font`.
- [ ] T028 Top bar `lib/components/TopBar.svelte`: menu entries opening the modals (file, anki,
      websocket, ignore list, freq weights, POS filters, analyzer, setup checklist) + status
      indicators (anki/player) from `statusStore`.

### US1 — Mine vocabulary from a file (P1) 🎯 MVP

- [ ] T029 [US1] `TermTable.svelte`: virtualized rows (e.g. `@tanstack/svelte-virtual`); columns
      term/reading/POS/frequency(`frequencies.HARMONIC`)/sentence-count/comprehension. Default
      sort = frequency, mirroring `gui/table` defaults.
- [ ] T030 [US1] `SentenceView.svelte`: render `SentenceDto.segments` as `<ruby><rt>` furigana
      over the Japanese text; expandable per term (multi-sentence browsing via
      `sentence_references`).
- [ ] T031 [US1] File open + drag-drop: `open_file_dialog`→`process_file`; Tauri
      `onDragDropEvent` for drops (O2); loading overlay from `overlayStore`; error modal on
      failure (don't clobber existing results).
- [ ] T032 [US1] **Verify** against egui: same term count/order/readings/POS/frequencies; furigana
      renders; drag-drop parity.

### US2 — Hide known words via Anki (P1)

- [ ] T033 [US2] Wire cached-load + background live refresh: consume `terms-refreshed`; show
      comprehension column values; `anki-status` indicator in top bar.
- [ ] T034 [US2] **Verify**: same hidden terms + comprehension % as egui; offline load works;
      live refresh updates in place.

### US3 — Seek the video player (P2)

- [ ] T035 [US3] Timestamp UI in `SentenceView`: clickable timestamp → `seek_timestamp(secs,
      label)`; reflect `player-status` (mode/no-player) and surface the no-player error.
- [ ] T036 [US3] **Verify**: asbplayer + MPV seek; MPV preferred when both; no-player handled.

### US4 — Refine & search (P2)

- [ ] T037 [US4] Table controls `lib/components/TableControls.svelte`: sort selector (frequency,
      chronological, sentence count, comprehension), search box, POS multiselect filter, and a
      frequency-range double-slider. All operate client-side on `termsStore` (research R6),
      mirroring `gui/table/{sort,filter,search}.rs`.
- [ ] T038 [US4] Ignore list: right-click term → `add_to_ignore_list`; ignore-list modal
      (`get/remove_from_ignore_list`); table updates from returned `FileLoadResult`.
- [ ] T039 [US4] **Verify**: each sort/filter/search/ignore action yields the same visible set as
      egui.

### US5 — Configure & personalize (P2)

- [ ] T040 [P] [US5] Anki settings modal: `list_anki_models`→ note-type/field mapping UI with
      field guessing + live `anki-status`; save via `save_settings`.
- [ ] T041 [P] [US5] WebSocket settings modal: edit port → `set_websocket_port`.
- [ ] T042 [P] [US5] Frequency weights modal: `list_dictionaries` + `set_dictionary_state`;
      consume `dictionaries-changed` and re-fetch terms.
- [ ] T043 [P] [US5] POS filters modal: default POS visibility from `get_pos_catalog` +
      `settings.pos_filters`.
- [ ] T044 [P] [US5] Theme + font toggles wired to `save_settings` (uses T026/T027).
- [ ] T045 [US5] Setup checklist + banner: `get_setup_status`; actions (`open_url`, open Anki
      settings, load dicts, open websocket settings).
- [ ] T046 [US5] **Verify**: every setting persists across restart and takes effect; checklist
      reflects true state.

### US6 — Frequency analyzer (P3)

- [ ] T047 [US6] Analyzer modal: file selection; `start_analysis` (Channel progress with ETA);
      `cancel_analysis`; results-preview table; export form → `export_analysis` (Yomitan/CSV +
      options); consume `analysis-complete`/`analysis-cancelled`/`export-complete`.
- [ ] T048 [US6] **Verify**: same ranking + equivalent exported artifacts as egui; cancel works.

### US7 — Knowledge summary (P3)

- [ ] T049 [US7] Knowledge summary widget: `get_knowledge_summary` + `knowledge-summary` event;
      JLPT + frequency bands; coverage/estimate toggle (`KnowledgeMode`).
- [ ] T050 [US7] **Verify**: JLPT + band values match egui for the same Anki snapshot.

**Checkpoint**: All user stories functional and verified against egui.

---

## Phase D: Package, sign off, CI

- [ ] T051 Declare bundled resources in `tauri.conf.json` (fonts handled in-frontend; icon;
      `assets/jlpt_vocab.json` per O1 outcome); confirm runtime downloads (unidic, freq dicts,
      Anki cache) still resolve to `dirs::data_local_dir()/yomine`.
- [ ] T052 `cargo tauri build` produces installers on Win/macOS/Linux; smoke-test each artifact.
- [ ] T053 CI: replace/augment `.github/workflows/release*.yml` + `manual-release.yml` with the
      Tauri bundler; update `test.yml` to build the workspace (egui on/off matrix) + run
      `svelte-check`.
- [ ] T054 Final parity sign-off: walk the full quickstart.md checklist; tick spec.md
      Success Criteria SC-001..SC-009.
- [ ] T055 Resolve Open Item O4: decide whether to retire the egui crate/feature or keep it
      gated for a transition; document in README/RELEASES.

---

## Dependencies & Execution Order

- **Phase A (T001–T012)** blocks everything. Within A: T001→T002→T003 (workspace/gating) before
  the moves; T004/T005 and T006 are independent moves; T008/T009/T010 [P] are independent serde
  edits; T012 is the gate.
- **Phase B (T013–T025)** depends on A. T016/T017/T018 before the commands that use them
  (T019/T021); T020 background after AppState (T016); T023/T024 [P] frontend can start once
  contracts are fixed.
- **Phase C** depends on B. Shell (T026–T028) before stories. US1 (T029–T032) is the MVP and is
  a soft prerequisite for US2/US3/US4 (they extend the table). US5/US6/US7 are largely
  independent and parallelizable across developers once the shell + IPC exist.
- **Phase D** depends on all targeted stories.

## Parallel Opportunities

- T008/T009/T010 (serde) in Phase A.
- T023/T024 (IPC layer + stores) in Phase B.
- T027 (fonts) alongside shell.
- US5 modal tasks T040–T044 are all `[P]` (separate components/commands).
- US5/US6/US7 can proceed in parallel after US1.

## Notes

- Verify analysis parity continuously: `cargo test` must stay green after every Phase A change.
- Commit only when the maintainer asks; spec-kit auto-commit hooks are intentionally skipped.
- Keep the egui app runnable for side-by-side verification through Phase C (Constitution I).

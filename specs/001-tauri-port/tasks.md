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
- **NEXT: Phase B / T013 — Tauri scaffold + IPC.** Begins with `cargo install tauri-cli`.
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

- [ ] T013 Install Tauri tooling: `cargo install tauri-cli --version "^2"`. Scaffold `src-tauri`
      (Tauri v2) as a workspace member depending on `yomine { default-features = false }`.
- [ ] T014 Scaffold SvelteKit under `src-tauri/ui` with `@sveltejs/adapter-static` in SPA mode
      (`prerender`/`ssr=false`); wire `tauri.conf.json` `build.frontendDist`/`devUrl` and
      `beforeDevCommand`/`beforeBuildCommand` to pnpm.
- [ ] T015 Add Tauri plugins: `tauri-plugin-dialog` (file open), `tauri-plugin-opener` (open URL),
      `tauri-plugin-fs` if needed; register capabilities/permissions.
- [ ] T016 Implement `AppState` in `src-tauri/src/state.rs`: `Mutex`-guarded struct holding
      `Option<LanguageTools>`, `SettingsData`, `PlayerManager`, current `FileData`
      (terms/sentences/comprehension), analyzer cancel token + last `FrequencyAnalysisResult`.
- [ ] T017 Implement `src-tauri/src/events.rs`: event-name constants + payload structs mirroring
      contracts/events.md (`AnkiStatus`, `PlayerStatus`, `LoadingMessage`, `FileLoadResult`, …).
- [ ] T018 Define the `SentenceDto` + `FileLoadResult` mapping in the backend (data-model.md):
      convert `Sentence` → `SentenceDto` (timestamp → secs+labels via `TimeStamp::to_secs`/
      `to_human_readable`).
- [ ] T019 Implement lifecycle commands (`src-tauri/src/commands/lifecycle.rs`):
      `load_language_tools` (Channel progress, loads into AppState, emits
      `language-tools-status`), `get_pos_catalog`, `get_settings`, `save_settings`.
- [ ] T020 Implement the background task (`src-tauri/src/background.rs`): ~5s Anki poll →
      `anki-status`; player mode switch (`PlayerManager::update`) → `player-status`; knowledge
      summary recompute → `knowledge-summary`. Start it in `main.rs` setup with the same
      throttles egui used.
- [ ] T021 Implement `process_file` + `get_terms` (`commands/file.rs`) calling
      `pipeline::process_source_file`; return `FileLoadResult`; if Anki reachable, spawn
      `refresh_terms` and emit `terms-refreshed`. Implement `open_file_dialog` via dialog plugin.
- [ ] T022 Implement `src-tauri/src/main.rs`: Tauri builder, manage `AppState`, register all
      commands, start background task. Window config (size/title/icon) from tauri.conf.json.
- [ ] T023 [P] Frontend IPC layer `src-tauri/ui/src/lib/ipc.ts`: typed wrappers over
      `invoke`/`listen`/`Channel`; mirror TS types from data-model.md.
- [ ] T024 [P] Frontend stores `src-tauri/ui/src/lib/stores/`: `terms`, `settings`, `status`,
      `overlay`, `knowledge`, `analyzer`; hydrate on startup per contracts/events.md.
- [ ] T025 **Verify Phase B**: `cargo tauri dev` opens; startup loads tools (progress shows);
      `process_file` on a real SRT returns a term count rendered in a placeholder list.

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

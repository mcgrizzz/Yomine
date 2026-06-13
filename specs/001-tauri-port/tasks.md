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
- **Phase C in progress: T026 (shell/theme) + T029 (term table) DONE — uncommitted.** Both build
  green (`pnpm build` ✓, `pnpm check` clean except the pre-existing vite.config.ts debt). The
  working tree holds these as two units on top of the pushed `fb939f3` (T019–T024): T026 =
  `app.css` + `+layout.svelte` + `+page.svelte` shell; T029 = `lib/components/TermTable.svelte` +
  `+page.svelte` wiring. `+page.svelte` and `tasks.md` carry deltas from both.
- **T030 DONE — verified working on Windows (`cargo tauri dev`), uncommitted.** Opened a real SRT
  → 208 terms / 335 sentences / 51% comprehension render; row expansion shows the furigana
  sentence view. Two bugs fixed during bring-up: (1) `openAndProcessFile` had the dialog call
  outside its try/catch → failures were silent; now wrapped, and `+page` surfaces `lastError` in a
  banner. (2) `Term.id` is **not unique** (engine doesn't assign distinct ids), so the keyed
  `{#each}` crashed (`each_key_duplicate`); rows now key on `lemma_form + reading` (unique
  post-dedup, stable across the re-sort T037 adds) and expansion tracks that key.
  Frontend is **not built in WSL** (must not touch the Windows `node_modules`); verified by running.
  Two uncommitted units: **toolchain** (`package.json`, `pnpm-workspace.yaml`, `pnpm-lock.yaml`)
  and **T030** (`Cargo.toml`, `dto.rs`, `ipc.ts`, `pos.ts`, `SentenceView.svelte`,
  `TermTable.svelte`, `+page.svelte`, `stores/index.ts`).
- **SPEC CORRECTION (2026-06-07).** Maintainer review found the table drifted from egui: sentence
  must be **inline** in each row (not an expander) and the term reading must be **furigana above**
  the word. Also `spec.md` lacked the **no-file landing state** (FR-001 now covers it). Edited
  `spec.md` (US1 desc/AS1/AS2, FR-001, FR-003), `quickstart.md` (#1), and re-scoped **T029/T030 →
  `[~]` (rework to inline "lean first pass")**, **T031** (landing state + recent files). Recent
  files were already specced (FR-001) — they need a `get_recent_files` command + the file UI.
- **T029/T030 REWORK DONE (2026-06-07) — code-complete in WSL, NOT frontend-verified, uncommitted.**
  Restructured to the egui inline layout: `TermTable.svelte` now renders four columns
  **Term │ Sentence │ Frequency │ POS** with the lemma's reading as `<ruby><rt>` **furigana above**
  the word; the example sentence renders **inline** in each row (no expander, no `<button>`/caret/
  `expandedKey`). `SentenceView.svelte` reduced to a single-occurrence inline renderer
  (furigana over kanji, term highlighted red); the nav/timestamp/comprehension meta row was
  **removed** and the prop changed `occurrences: Occurrence[]` → `occurrence: Occurrence` (TermTable
  resolves the term's first resolvable `sentence_reference`). Reused as-is: `harmonic` sort, `？`
  sentinel, `posColor`, `posCatalog` labels, byte-offset highlight, the `SegmentDto` enrichment.
  `+page.svelte` wiring unchanged. The deferred nav + per-sentence comprehension indicator are now
  filed as **T030b** (US1 sentence polish). **Furigana fix:** ruby must sit over **kanji only**, not
  okurigana (心ない → こころ over 心, ない bare). Added `ui/src/lib/furigana.ts` (`furiganaParts` —
  splits surface+hiragana reading into kanji/kana runs, kana runs are verbatim anchors into the
  reading; whole-over-whole fallback for unalignable irregular readings) + a shared
  `ui/src/lib/components/Furigana.svelte` (renders the parts inline, color inherits). Both the term
  column and `SentenceView` now render via `Furigana` (SentenceView dropped its own `hasKanji`
  whole-segment ruby). **Overhang fix (2nd pass):** readings wider than their kanji were overhanging
  into the next word and merging (警戒+心 → "けいかいこころ"). `Furigana` now emits ONE `<ruby>` per
  word with alternating base/`<rt>` pairs (empty `<rt>` over okurigana) and wraps it in an
  `inline-block` box so each word's reading centers over itself and can't overhang the neighbor;
  lines still break between words. Also `ruby-align: space-around` so a reading narrower than its
  base (jukugo like 警戒→けいかい, or a whole-word fallback) spreads across the region it covers
  instead of bunching in the centre (WebKit's old default). **Frontend not built in WSL** (must not touch Windows `node_modules`) — verify
  via `cargo tauri dev` on Windows (folds into T032).
- **T031 DONE (2026-06-07) — backend builds green, frontend NOT built in WSL, uncommitted.**
  Landing state + recent files + drag-drop. **Backend:** moved `gui/recent_files.rs` → neutral
  `core/recent_files.rs` (re-exported from `core` and from `gui::recent_files` so egui call sites
  still resolve — mirrors the settings/`LanguageTools` decoupling; needed because the Tauri crate
  builds `--no-default-features` and never compiles `gui`). Added `get_recent_files` command
  (loads the shared `recent_files.json`, `get_valid_files`, most-recent-first) + `record_recent_file`
  helper now called inside `process_file` (egui parity: `add_recent_file` with `terms.len()`), and
  registered `get_recent_files` in `lib.rs`. **Frontend:** `ipc.ts` gained `RecentFileEntry`,
  `getRecentFiles()`, `onFileDrop()` (Tauri `getCurrentWebview().onDragDropEvent`); `stores`
  refactored `openAndProcessFile` → shared `loadAndStore(path)` + `openRecentFile(path)`, added a
  `recentFiles` store (hydrated + refreshed after each load) and drag-drop wiring in `hydrate`;
  `+page.svelte` replaced the bare placeholder with the egui-parity landing block ("No File Loaded"
  cyan / `ファイルがまだ読み込まれていません` orange / drag-drop hint / "Open New File" / recent-files
  list with title·filename·term-count·creator·date·size). **Drag-over overlay + scroll (refinement):**
  egui shows a "📥 Drop to open" modal while a *supported* file hovers (`draw_file_drop_overlay`);
  mirrored it — `ipc.onDragDrop` now exposes enter/drop/leave, the store tracks a `dragHovering`
  flag (true when a dragged path matches `srt/ass/ssa/txt`, drop loads the first supported path),
  and `+page.svelte` renders a `pointer-events:none` `.drop-overlay`. The welcome screen no longer
  scrolls the whole main: `.landing` is `height:100%` and only the recents `<ul>` scrolls
  (`flex:1;min-height:0;overflow-y:auto`), matching egui's `ScrollArea::max_height(200)`. Recent
  rows also carry a `title={file_path}` tooltip. Drag-drop is **gated on tools-ready**
  (`get(languageToolsStatus) === 'ready'`) so an early drop during the "Loading language tools…"
  splash shows no overlay and doesn't load (backend `process_file` also guards as a backstop).
  Build matrix: `cargo build -p yomine-tauri`
  ✓ · `-p yomine --no-default-features` ✓ · `-p yomine` (egui) ✓. **Verify the frontend via
  `cargo tauri dev` on Windows** (folds into T032): dialog open, recent-file click, and native drop
  all route through `loadAndStore`; the landing list populates after the first load.
- **T032 DONE — VERIFIED on Windows (maintainer, 2026-06-07).** US1 passes against egui (term
  count/order/readings/POS/frequencies; furigana; landing state + recent files + drag-drop). T031
  committed as `dc0ba36`; working tree clean at the start of the T037 session.
- **T037 DONE (2026-06-07) — code-complete in WSL, frontend NOT built (Windows `node_modules`),
  uncommitted.** Client-side term-table controls (US4), mirroring `gui/table/{sort,filter,search}.rs`.
  **New `lib/table.ts`** (pure): `harmonic` (moved out of `TermTable`'s module script — the `"HARMONIC"`
  key is `get_weighted_harmonic`, so it equals egui's `weighted_frequency`), `SortField`
  (frequency/chronological/sentenceCount/comprehension) + `defaultDir` (freq/chrono asc, count/comp
  desc — egui parity), `freqBounds` (= `configure_bounds`, lower bound floored at 1), `matchesSearch`
  (term forms/readings/POS + sentence text; case-insensitive + katakana→hiragana fold), and
  `applyControls` (filter→sort, = `recompute_indices`). **`stores/index.ts`:** control writables
  `tableSearch`/`tableSort`/`posEnabled`/`freqFilter` + a `visibleTerms` derived; `posEnabled` seeded
  from `settings.pos_filters` (missing key = enabled, = `is_enabled` default); `freqFilter` re-derived
  on every `fileResult` change (selection resets to full range). **New `TableControls.svelte`:** search
  box, sort `<select>` + ▲/▼ direction toggle, POS multiselect `<details>` popover (All/None +
  per-POS checkboxes from `posCatalog`), dual frequency `<input type=range>` (min/max, mutually
  clamped) + "?" include-unknown toggle, and an `N / total shown` counter. **`TermTable.svelte`** now
  renders the pre-filtered/sorted `terms` (`{#each terms}`, no internal sort) and imports `harmonic`
  from `lib/table`. **`+page.svelte`** renders `<TableControls/>` above `<TermTable terms={$visibleTerms}/>`.
  **Behavior delta to verify (T039):** egui's freq filter defaults `include_unknown=false`, so
  unknown-frequency ('？') terms are now **hidden by default** (revealed via "?"); the meta line still
  shows the full minable count, the controls counter shows the visible subset. **Search parity (full):**
  `matchesSearch` is a faithful port of `core::utils::text_matches_search` — it normalizes via the
  `wanakana` JS package (`toHiragana`: romaji→kana + katakana→kana) plus a hand-port of
  `normalize_long_vowel` (o-row+お→う / e-row+え→い on all-hiragana strings), then an ASCII fallback.
  `wanakana` is the library `wana_kana` (Rust, the engine's) was ported from, so romaji/kana/kanji/
  English all match egui (residual risk only on exotic romaji edge cases between JS 5.3.1 / Rust 4.0.0).
  **New runtime dep `wanakana@^5.3.1`** added to `src-tauri/ui/package.json` → **run `pnpm install` on
  Windows before `cargo tauri dev`** (updates `pnpm-lock.yaml`; commit as a toolchain unit). Also
  added `optimizeDeps.include: ['wanakana']` to `vite.config.ts` so a fresh install is pre-bundled
  without a manual dev restart.
  **VERIFIED on Windows (maintainer, 2026-06-07):** search works incl. romaji (`jinsei`→人生's reading),
  kana, kanji; freq/POS filters apply alongside search (a known term like 人生 is correctly absent
  because it's filtered out of the minable set, not a search bug). Remaining T039 parity sweep
  (sort/filter/ignore vs egui) still open.
- **T038 DONE (2026-06-07) — backend builds green in WSL; frontend NOT built (Windows
  `node_modules`), uncommitted.** Ignore list (US4). **Backend (3 new commands in
  `src-tauri/src/commands/ignore.rs`, registered in `lib.rs`):** `get_ignore_list` → `Vec<String>`
  (locks `LanguageTools.ignore_list`, `get_all_terms`); `add_to_ignore_list(lemma)` /
  `remove_from_ignore_list(lemma)` → `Option<FileLoadResult>` (null when no file loaded). Both
  mutate+persist the shared `IgnoreList` (`add_term`/`remove_term` call `save()` internally → same
  `ignore_list.json` egui uses), then re-run `pipeline::apply_filters(base_terms, &tools,
  AnkiFilter::KnownLemmas(anki_known))` — a faithful port of egui's `partial_refresh` (re-applies
  ignore + cached-Anki filter, no Anki connection), store the new minable set, and return the
  refreshed `FileLoadResult`. **For parity this required storing the Anki-known lemma set:**
  `state.rs::FileData` gained `anki_known_lemmas: HashSet<String>` (mirrors egui's
  `FileData::anki_filtered_terms`), populated in `process_file` from `filter_result.anki_filtered`,
  so an ignore change doesn't resurrect Anki-known terms. `commands/file.rs::load_result` made
  `pub(crate)` and reused by the ignore commands. **Locking discipline kept** (clone handles under a
  brief lock → mutate/await unlocked → re-lock to store; `Mutex<AppState>` never held across the
  `.await`). **Frontend:** `ipc.ts` gained `getIgnoreList`/`addToIgnoreList`/`removeFromIgnoreList`;
  `stores/index.ts` gained `ignoreList`/`ignoreModalOpen` writables + `openIgnoreModal`/`addToIgnore`/
  `removeFromIgnore` actions (add/remove set `fileResult` from the returned result → table re-filters
  in place). **`TermTable.svelte`** got a right-click context menu on the term cell ("Add to ignore
  list" → the visible rows are never already-ignored, so removal lives only in the modal; closes on
  any click/scroll/contextmenu via `<svelte:window>`). **New `IgnoreListModal.svelte`** lists the
  ignored lemmas with per-row remove ✕ + backdrop/Esc close. **`+page.svelte`** renders the modal and
  an **interim** "Ignore list…" topbar button (gated on tools-ready; T028 folds it into the proper
  menu — TODO updated). Backend matrix: `cargo build -p yomine-tauri` ✓ (only pre-existing transient
  dead-code warnings). `-p yomine`/`--no-default-features` unaffected (zero engine changes).
  **Verify the frontend via `cargo tauri dev` on Windows** (folds into T039): right-click a row →
  term disappears + persists; open the modal → remove → term reappears in the table; ignored set
  matches egui's filtering. **NOTE:** T038 is the *minimal* ignore list (FR-007's add-via-context +
  remove-via-view). Maintainer reviewed the modal vs. egui (2026-06-07) and chose **full parity** —
  the gap (manual add, in-modal search, file-import subsystem, staged Save/Cancel, Restore Default,
  Export) is now specced as **T038b** with an expanded **FR-007**, a rewritten contract "Ignore list"
  section (`get_ignore_list_full`/`import_ignore_file`/`refresh_ignore_file`/`save_ignore_list`/
  `get_default_ignored_terms`/`export_ignore_list` + `IgnoreFile`/`IgnoreFileView`/`IgnoreListView`),
  and an updated T039 verify. T038b is not yet implemented.
- **T038b DONE (2026-06-07) — backend builds green in WSL (`cargo build -p yomine-tauri` ✓, exit 0,
  only the pre-existing transient dead-code warnings); frontend code-complete but NOT built (Windows
  `node_modules`), uncommitted.** Ignore-list modal → full egui parity
  (`src/gui/settings/ignore_list_modal.rs`). **Backend (6 new commands in `commands/ignore.rs`,
  registered in `lib.rs`; DTOs in `dto.rs`):** `get_ignore_list_full` → `IgnoreListView` (manual
  terms + file pills, each with `exists` + `term_count` via a `file_view` helper using
  `IgnoreList::{file_exists, load_terms_from_file}`); `import_ignore_file` → `IgnoreFileView | null`
  (`.txt` open dialog, modelled on `open_file_dialog`'s `app.dialog().file()` + oneshot); 
  `refresh_ignore_file(path)` → `IgnoreFileView` (re-reads exists/count; frontend keeps the staged
  `enabled`); `save_ignore_list(terms, files: IgnoreFile[])` → `Option<FileLoadResult>` — the single
  commit point: `set_terms` + `set_files` (which persists + `reload_file_cache` internally) then the
  same `apply_filters(base_terms, &tools, AnkiFilter::KnownLemmas(anki_known))` re-filter path as
  `add_to_ignore_list`; `get_default_ignored_terms` → `Vec<String>` (`DEFAULT_IGNORED_TERMS`);
  `export_ignore_list(terms)` → `String | null` (`.txt` save dialog + newline-join write, dated
  default filename via `chrono` — added to `src-tauri/Cargo.toml`, unifies to the engine's 0.4.41).
  New DTOs `IgnoreFileView`/`IgnoreListView` in `dto.rs` (+ data-model.md "Ignore list" section).
  Same locking discipline (no `Mutex<AppState>` across `.await`). **Frontend:** `ipc.ts` gained the 6
  wrappers + `IgnoreFile`/`IgnoreFileView`/`IgnoreListView` types; `table.ts` now **exports**
  `textMatches` (the string-level port of egui's `text_matches_search`) for the modal's term search;
  `stores/index.ts` replaced the minimal pieces — **removed** the now-unused `ignoreList` writable +
  `removeFromIgnore` action (the modal stages + persists via Save, so per-term immediate remove is
  gone), simplified `openIgnoreModal` to just open (the modal self-hydrates), kept `addToIgnore`
  (row right-click stays **immediate**), and added a `saveIgnore(terms, files)` action.
  **`IgnoreListModal.svelte` fully rewritten** to egui parity: Add-New-Term input (Enter/Add stages),
  Search-Terms filter, file pills (📄 name · enable checkbox · ↻ refresh · ✕ remove · `(missing)`
  state · count) + "+ Import File" pill (hidden while searching), term pills (✕), "Manual: N | From
  Files: M" counts, staged Save (enabled when dirty) / Cancel (reverts staged, egui keeps modal open)
  / Export… / Restore Default, and a ⚠ "Settings have been modified" dirty indicator (temp vs.
  original snapshot, files compared on path+enabled). **Decisions (per handoff):** row right-click =
  immediate `add_to_ignore_list`; modal = staged `save_ignore_list`. `remove_from_ignore_list`
  backend command + `removeFromIgnoreList`/`getIgnoreList` ipc wrappers **kept for API completeness**
  (contract still documents them) though the modal no longer calls them. **File-content search
  deviation:** egui filters file pills by loading each file's terms and matching; the client only
  ships `term_count`, not the terms, so file pills stay visible regardless of search (only manual
  term pills + the import pill react to search). Noted intentional. `-p yomine` /
  `--no-default-features` / `cargo test` unaffected (zero engine changes). **Verify on Windows
  (`cargo tauri dev`, folds into T039):** `pnpm install` first (T037's `wanakana` dep still pending);
  open modal → add/search/import/refresh/remove/toggle stage; dirty ⚠ shows; Save persists + re-filters
  the table; Cancel reverts; Restore Default + Export work; compare against egui.
- **T039 DONE — VERIFIED on Windows (maintainer, 2026-06-09).** All of US4 (sort / filter / search /
  POS / frequency from T037 + the T038/T038b ignore modal) yields the same visible set/behaviour as
  egui. Closes the US4 verify gate; T038b's frontend verify folded in.
- **T028 BACKEND COMMANDS DONE (2026-06-09) — backend builds green in WSL (`cargo build -p
  yomine-tauri` ✓, exit 0; warnings dropped to 8, all pre-existing transient — the new commands now
  consume `DICTIONARIES_CHANGED`/`PLAYER_STATUS`/`AnkiStatus`/`PlayerHandle`).** This is the
  **backend half** of T028 (the command batch the handoff flagged as the blocker); the TopBar.svelte
  UI + the per-modal `ipc.ts` wrappers are the **frontend half**, deferred to their own
  tasks/Windows builds (T028 UI, T035, T041) so untestable TS isn't shipped from WSL. **8 new
  commands** (registered in `lib.rs`): `commands/anki.rs` — `get_anki_status` (point-in-time
  `get_version` probe, `fetching:false`), `list_anki_models` (mirrors egui `fetch_models`: version
  gate → `anki::get_models` → map `Model`→`AnkiModelInfo`; `Err("Anki Offline")` when down).
  `commands/player.rs` — `seek_timestamp(seconds,label)` / `get_player_status` (thin `PlayerHandle`
  channel wrappers; covers T035/T041 backend), `set_websocket_port(port)` (persist
  `settings.websocket_settings.port` + `PlayerHandle::set_port` restart). `commands/dictionary.rs` —
  `list_dictionaries` (live `dictionary_states` → `DictionaryStateDto{name,weight,enabled}`, sorted
  by name) / `set_dictionary_state(name,weight,enabled)` (manager gets `weight.max(0.1)`, settings
  keep raw value + persist + `knowledge_dirty`, emit `dictionaries-changed`; faithful to egui's
  `apply_frequency_settings`). `commands/setup.rs` — `get_setup_status` → `SetupStatus`
  (tools/mapping/dict read under a brief lock; anki + player probed unlocked; mirrors the
  `check_*` in `setup_checklist_modal.rs`). **New DTOs** `DictionaryStateDto`/`SetupStatus` in
  `dto.rs` (per data-model.md). Locking discipline kept (no `Mutex<AppState>` across `.await`). Zero
  engine changes → `-p yomine` / `--no-default-features` / `cargo test` provably unaffected.
- **T028 FRONTEND DONE (2026-06-09) — code-complete in WSL, frontend NOT built (Windows
  `node_modules`), uncommitted.** `TopBar.svelte` — the full egui `top_bar.rs` IA: ☀/🌙 theme toggle
  + 字 font toggle (new `toggleDarkMode`/`toggleSerifFont` store actions: flip the bit, mirror to the
  `settings` store so the root `+layout` re-applies theme/serif, then `saveSettings` — egui's
  `request_save_settings`), the **File / Settings / Tools** dropdown menus (one-open-at-a-time via a
  local `openMenu` rune; `<svelte:window onclick>` closes; trigger `stopPropagation` so its own click
  doesn't re-close), and the right-aligned **asbplayer / mpv / Anki** status indicators (egui
  `show_status_indicators` colors: green `#00c800` / yellow `#c8c800` / grey `#646464` / anki-red
  `#c85050`; a CSS spinner while `anki.fetching`). **Wired now (actions exist):** File→Open New File
  (`openAndProcessFile`, gated on tools-ready), File→Quit (`getCurrentWindow().close()` — added
  `core:window:allow-close` to `capabilities/default.json`), Settings→Ignore List (`openIgnoreModal`,
  gated). **Disabled-pending entries** (full menu shell, each a `<button disabled title="Coming
  soon">` with a `TODO` naming its task — flip to enabled + add `onclick` when the modal lands):
  File→Load New Frequency Dictionaries, File→Open Data Folder; Settings→Anki, WebSocket Server (T041),
  Frequency Weighting, Part of Speech Filters, Setup Checklist; Tools→Frequency Analyzer (US6).
  **`+page.svelte`** dropped its interim header (the old "Open file…"/"Ignore list…" buttons + Anki/
  Player chips + the `.topbar`/`.brand`/`.spacer`/`.status`/`.chip` styles + the now-unused
  `openIgnoreModal`/`ankiStatus`/`playerStatus` imports + `playerLabel`) → now just `<TopBar />`.
  **Status-indicator deviation (noted):** `PlayerStatus` collapses the WebSocket server state to
  `mode` + `ws_clients`, so the asbplayer dot can't show egui's Error/Starting sub-states — it maps
  connected→green, mode==='asbplayer' (running, waiting)→yellow, else→grey. **No new `ipc.ts`
  wrappers** (TopBar reads existing stores/events + `saveSettings`; the pending modals add their own
  wrappers). **Verify on Windows (`cargo tauri dev`):** `pnpm install` first (T037's `wanakana` dep
  still pending); compare the menu structure / theme+font toggle (persists across restart) / status
  dots against egui; confirm Quit closes the window, Open New File + Ignore List work, disabled
  entries are inert.
- **T027 DONE (2026-06-11) — frontend NOT built in WSL (Windows `node_modules`), uncommitted.**
  Bundled the two Japanese faces. Reused the **exact TTFs the egui app embeds**
  (`assets/fonts/Noto{Sans,Serif}JP-Regular.ttf`, per `gui/app/mod.rs::setup_fonts`) → copied to
  `src-tauri/ui/static/fonts/` (adapter-static serves `static/` at root, so `/fonts/*` resolves in
  the Tauri webview). Added two `@font-face` blocks at the top of `src-tauri/ui/src/app.css`
  (`format('truetype')`, weight 400, `font-display: swap`) whose family names match the **already
  present** `body { font-family: 'Noto Sans JP' … }` / `body.font-serif { font-family: 'Noto Serif
  JP' … }` rules + the `.font-serif` toggle (driven by `+layout.svelte` from `settings.use_serif_font`,
  flipped by T028's 字 button). Pure font-face wiring; no markup/store changes. Fonts are not
  gitignored (`check-ignore` → tracked-ok). **Verify on Windows (`cargo tauri dev`):** base text
  renders in Noto Sans JP; the 字 toggle switches to Noto Serif JP and persists across restart.
  (Closes the [P] font-faces piece; T044 "theme+font toggles wired to save_settings" was already
  satisfied by T028.)
- **T035 DONE (2026-06-11) [US3] — frontend NOT built in WSL (Windows `node_modules`), uncommitted.**
  Clickable sentence timestamp → seek. Pure frontend wiring (backend `seek_timestamp` already landed
  + registered in `lib.rs` with the T028 batch; `SentenceDto.timestamp` already carries
  `{start_secs,end_secs,start_label,end_label}` via `TimeStampDto`). **`ipc.ts`:** added
  `seekTimestamp(seconds,label)` → `invoke('seek_timestamp',{seconds,label})`. **`stores/index.ts`:**
  added a `playerConnected` derived (`mpv_connected || ws_clients>0`, = egui `Player::is_connected`) +
  a `seekTimestamp` action (calls ipc, surfaces failures via `lastError` instead of egui's eprintln).
  **`SentenceView.svelte`:** renders the sentence's `timestamp` below the text — a clickable `▶
  {start_label}` button when `$playerConnected`, else a weak-text label (egui `ui_timestamp`:
  button when connected, weak label otherwise). **Deviation (noted, same as T028's player dot):** no
  "confirmed" state on the wire (`PlayerStatus` omits it), so the button is always `▶` — egui's 👁
  confirmed-seek variant is not mirrored. Zero engine/Rust changes → `-p yomine` /
  `--no-default-features` / `cargo test` unaffected. **Verify on Windows (`cargo tauri dev`, folds
  into T036):** with mpv/asbplayer connected, a sentence's `▶ mm:ss` seeks the player; with none
  connected the timestamp shows as a plain weak label (no button); TXT sentences show no timestamp.
- **T030c DONE (2026-06-11) [US1] — sentence wrapping, uncommitted with this batch.**
  `SentenceView.svelte`'s `{#each}` over segments now emits `{#if i > 0}<wbr />{/if}` between the
  inline-block word boxes, restoring a soft-wrap opportunity between words (root cause per the task
  note: Svelte strips the whitespace between adjacent atomic inline-blocks). **Verify on Windows
  (folds into T032):** long sentence wraps to 2+ lines (row grows), furigana still centres per word,
  no horizontal scrollbar.
- **T030b DONE (2026-06-11) [US1] — sentence polish (◀ n/m ▶ nav + 5-bar comprehension), egui
  `sentence_column.rs` parity.** Backend: `FileLoadResult` gains `anki_filter_active: bool` (set in
  `commands/file.rs::load_result` from `!anki_known_lemmas.is_empty()`, mirroring egui's
  `anki_filtered_terms.is_empty()` gate) — indicator gating is backend-derived, single source of
  truth per Constitution. `ipc.ts` field + new `ankiFilterActive` derived in `stores/index.ts`.
  `TermTable.svelte`: `firstOccurrence()` → `occurrencesOf()` (all resolvable `sentence_references`)
  passed to `SentenceView`, which was rewritten — props `{ occurrences, term }`, local clamped `idx`,
  wrap-around prev/next (egui `TableState::next/prev_sentence` are modulo); meta row under the
  sentence = nav (⏮ n/m ⏭, disabled when count ≤ 1, counter cyan 0.7rem) + T035 timestamp
  (unchanged) + comprehension bars (heights [2.5, 4, 6.5, 10.5, 14.5]px, width 3px, gap 1px,
  filled = `min(ceil(pct/20), 5)`, gated on `$ankiFilterActive`, tooltip "N% comprehensibility",
  empty bars `color-mix(in srgb, var(--comment) 30%, transparent)`). Color helper extracted to
  `$lib/comprehension.ts` (shared with T033). **Verify on Windows (folds into T032/T034):** nav
  cycles a term's occurrences in place; bars appear only with Anki data.
- **T033 DONE (2026-06-11) [US2] — live refresh + full-parity file summary
  (`summary.rs::ui_current_file_summary`; scope confirmed by maintainer).** Backend:
  `FileData.ignored_count` (display-only; kept fresh in both `commands/ignore.rs` re-filter sites);
  `FileLoadResult.{total_terms, ignored_terms}` set in `load_result`; `process_file` now takes
  `AppHandle` and, after storing `FileData`, spawns a `anki::api::get_version()` probe → on success
  `live_refresh(&app)`, on error emit `ERROR` "Refresh Error" (egui `handle_processing_result`
  tail). New `live_refresh(&AppHandle)` = port of `TaskManager::refresh_terms` + `TermsRefreshed`
  handler: no-op Ok when no file; `ANKI_STATUS {connected:true, fetching:true}` →
  `apply_filters(base_terms, &tools, AnkiFilter::Live(mappings))` → reconstruct `all_terms`,
  recompute per-sentence (`calculate_sentence_comprehension`) + avg file comprehension, update
  `FileData`, set `knowledge_dirty`, `ANKI_STATUS {fetching:false}` then `TERMS_REFRESHED` with the
  `load_result` payload. New `#[tauri::command] refresh_terms` (manual path) registered in `lib.rs`.
  Frontend: new `$lib/comprehension.ts` `comprehensionColor(pct)` (egui gradient red→yellow→green
  then desaturate `ch * 0.4 + 84`); `refreshTerms()` ipc wrapper + store action (guards tools ready
  + file loaded, overlay "Refreshing terms…", errors → `lastError` "Refresh Error"; result lands via
  the already-wired `terms-refreshed` listener); TopBar 🔄 after the Tools menu (shown only with
  `$fileResult`, disabled till tools ready); `+page.svelte` summary parity — gated
  `Comprehension estimate: N%` (colored via `comprehensionColor`, only when `anki_filter_active` and
  sentences exist, ~13px strong) + counts line (~12px weak) `{shown} shown / {known} known /
  {total} total` with the "known" segment only when > 0 and a hover breakdown
  (`Ignore list: n` / `Anki filtered: n`) — plus F5 / Ctrl(Cmd)+R → `preventDefault()` (also stops
  webview reload) + `refreshTerms()`. Manual-refresh errors return `Err` → banner; the auto path
  emits the `error` event (egui shows the same modal for both). **Builds (this session):**
  `cargo check -p yomine-tauri` ✓ 0 errors, `cargo check -p yomine` ✓ 0 errors (engine/egui
  untouched); `pnpm run check` = only the 4 known `vite.config.ts` scaffold errors (to run it from
  WSL, the linux rollup/esbuild native binaries were dropped into the Windows-installed
  `node_modules` — no package.json/lockfile change). **Verify on Windows (`cargo tauri dev`, folds
  into T034):** auto-refresh on load with Anki open (terms update in place, spinner during fetch),
  manual 🔄 / F5 / Ctrl+R, summary line parity against egui.
- **T032/T034/T036 DONE — VERIFIED on Windows (maintainer, 2026-06-11), walked check-by-check.**
  **T032 (US1):** term count/order/readings/POS/frequencies match egui; furigana renders; dialog +
  recent-file click + native drag-drop all pass; T030b nav cycles a term's occurrences in place and
  the comprehension bars gate correctly on Anki data. Two notes: (1) **furigana bug on numeric
  compounds** — `8月22日` shows がつにち (digit readings dropped, kana misplaced); niche, deferred
  to US6 tokenization tooling (see Deferred). (2) T030c wrap could not be exercised (no long-enough
  sentence in the test file); low risk now that wrapping is plain CSS — re-check opportunistically.
  **T034 (US2):** identical hidden terms + comprehension % vs egui; offline load (Anki closed) fine
  with disconnected indicator; auto-refresh on load updates terms in place with fetching spinner;
  manual 🔄 / F5 / Ctrl+R all refresh without reloading the webview; summary line parity incl.
  hover breakdown. **T036 (US3):** asbplayer seek ✓; MPV preferred when both connected ✓;
  no-player → weak label, TXT → no timestamp, no spurious errors ✓. Maintainer noticed the seek
  button doesn't mark itself "visited" — that is the recorded intentional T035 deviation
  (`PlayerStatus` omits egui's 👁 confirmed state); promote to a task only if requested.
  T031's folded frontend checks passed with T032, so T031 is ticked too.
- **T041 DONE (2026-06-11) [US5] — WebSocket settings modal; checks green in WSL, frontend NOT
  built (Windows `node_modules`), uncommitted.** Backend untouched (`set_websocket_port` landed +
  registered with the T028 batch — persists `settings.websocket_settings.port` to `settings.json`
  then `PlayerHandle::set_port` restarts a live server). **`ipc.ts`:** `setWebsocketPort(port)` →
  `invoke('set_websocket_port', { port })`. **`stores/index.ts`:** `websocketModalOpen` writable +
  `openWebsocketModal()` + `saveWebsocketPort(port)` action (mirrors the new port into the
  `settings` store on success; failure → `lastError` banner "WebSocket Server / Failed to apply the
  new port" — egui shows these inline as `restart_status`, the banner is our manual-action error
  convention; returns success so the modal knows whether to close). **New
  `WebsocketSettingsModal.svelte`** (IgnoreListModal pattern: backdrop/Esc/✕ close, staged temp vs
  original snapshot), parity with `gui/settings/websocket_settings_modal.rs`: "Server Port:" number
  input + "(Valid range: 1024-65535)" hint + red "⚠ Port must be between 1024 and 65535" when out
  of range (egui's DragValue clamps to the range; the number input validates instead), ℹ status
  line ("Invalid port range. Please use ports 1024-65535." on save-with-invalid), ⚠
  "Settings have been modified" dirty indicator, footer Save Settings / Cancel (both gated on
  dirty; Cancel reverts the staged port but keeps the modal open, egui parity) / right-aligned
  Restore Default (→ 8766 = `WebSocketSettings::default()`). Save closes on success (egui's
  `ui.close()`), stays open on failure for a retry. **`TopBar.svelte`** flipped the disabled
  "WebSocket Server" entry to `openWebsocketModal` (ungated, like egui); **`+page.svelte`** renders
  the modal next to `IgnoreListModal`. Checks: `cargo check -p yomine-tauri` ✓ 0 errors,
  `-p yomine` ✓ 0 errors, `pnpm run check` = only the 4 known `vite.config.ts` errors (the modal
  adds the same backdrop a11y *warning* IgnoreListModal already carries). **Verify on Windows
  (`cargo tauri dev`, folds into T046):** Settings→WebSocket Server opens; edit port → dirty ⚠;
  Save persists (`settings.json`) and a running server moves — asbplayer reconnects on the new
  port; Cancel reverts; Restore Default → 8766; out-of-range port shows the red warning and Save
  reports "Invalid port range"; restart starts the server on the saved port.
- **T044 DONE (2026-06-11) [US5] — audited as already complete; zero code changes this session.**
  Confirmed the T027 note's claim: egui's counterpart (`top_bar.rs` ~69–114) has exactly two
  controls — the ☀/🌙 theme-preference toggle and the 字 serif toggle, both flip the bit +
  `request_save_settings`; there is no separate theme/font picker menu to port. The Tauri side
  already mirrors it end-to-end: `stores/index.ts::toggleDarkMode`/`toggleSerifFont` (T028) flip
  the bit, mirror into the `settings` store, and call `save_settings` (persists `settings.json`);
  `+layout.svelte` (T026) reactively applies `data-theme` dark/light + `body.font-serif`; on
  restart `hydrate()`'s `get_settings` restores both — the full round trip. T027 supplies the Noto
  Sans/Serif JP `@font-face`s the 字 toggle switches between. Ticked with no new code. **Verify on
  Windows (`cargo tauri dev`, folds into T046):** toggle ☀/🌙 and 字 → instant restyle; restart →
  both persist (same items already on T028's verify list).
- **T040 DONE (2026-06-11) [US5] — Anki settings modal (note-type/field mappings + known
  interval); cargo checks green in WSL; `pnpm run check` was sandbox-denied for the subagent but
  the orchestrator ran it after: only the 4 known vite.config.ts errors + the modal's backdrop
  a11y warning (same pattern as T038b/T041). Uncommitted.** **Engine:** egui's term/reading guessing heuristic
  (`guess_field_mappings` + `is_likely_term`/`is_likely_reading`) moved verbatim out of
  `gui/settings/components.rs` into the new UI-neutral `src/anki/field_guessing.rs` (re-exported
  as `anki::guess_field_mappings`; components.rs `pub use`s it so old paths resolve) — the gui
  module is feature-gated off in the Tauri build, so this is the single-source-of-truth move the
  Constitution wants. **Backend:** one new command, `get_anki_sample_note(model_name, fields)` →
  `SampleNote { sample_note, guessed_term, guessed_reading }` (commands/anki.rs, registered in
  `lib.rs`): fetches the model's sample note (errors swallowed to `None`, mirroring egui's
  `fetch_sample_note` `.unwrap_or(None)`) and runs the shared guesser over it, so the heuristic
  never exists in TS. `list_anki_models` (T028) returns `sample_note: None` — samples are lazy
  per model, same as egui. **`ipc.ts`:** `AnkiModelInfo`/`SampleNote` types + `listAnkiModels()`
  + `getAnkiSampleNote(modelName, fields)`. **`stores/index.ts`:** `ankiModalOpen` writable +
  `openAnkiModal()` + `saveAnkiSettings(mappings, interval)` → merges the two staged fields into
  `settings`, `save_settings` (backend already propagates `known_interval` into live tools +
  marks knowledge dirty, the exact egui post-save behavior), mirrors into the `settings` store on
  success, `lastError` banner "Anki Settings / Failed to save settings" on failure, returns
  success so the modal can close. **New `AnkiSettingsModal.svelte`** (WebsocketSettingsModal
  pattern: backdrop/Esc/✕, staged temp vs original snapshot), full parity with
  `gui/settings/anki_settings_modal.rs`: ① Known Interval Threshold — ℹ hover, number input
  clamped 1–365 on change (DragValue parity), "(Default: 30 days)"; ② Current Notetypes — one row
  per staged mapping (Notetype / Term Field / Reading Field in egui's blue, Edit + Delete);
  ③ Add/Edit Notetype — heading flips with editor mode, "Anki Connection Status:" line (spinner +
  "Fetching models..." while loading, red "Error: …" on fetch failure shown *inline* like egui —
  not a banner — green "Connected" / yellow "Ready" otherwise, with the resting state live from
  the ambient `anki-status` store) + "Refresh Notetypes"/"Refreshing..." button, notetype select
  (selection clears both fields then guesses from the cached/fetched sample — egui
  `ui_model_selection`), Term/Reading field selects with the ＊ "guessed" marker + truncated
  (30-char) green/blue "Example:" values from the sample note (`ui_field_selection`), Add/Update
  gated on all three non-empty, rename-while-editing removes the original key; ④ footer — ⚠
  dirty indicator, Save Settings / Cancel gated on dirty (Cancel reverts staged edits, stays
  open; Save closes on success, stays open on failure), right-aligned Restore Default. Models are
  cached across opens (egui keeps `available_models` for the app lifetime); on open, sample notes
  are fetched for already-mapped models lacking one (egui `open_settings`). **Deviations
  (flagged):** Restore Default resets only the two fields this modal owns (mappings → empty,
  interval → 30) — egui resets the *whole* staged `SettingsData` to default, which on Save would
  clobber unrelated settings (websocket port, POS filters, theme…); scoped reset is the
  egui-faithful-minus-the-bug option. And the resting connection text is live from `anki-status`
  (per the task), so it can flip Connected→Ready while open, which egui's fetch-frozen string
  doesn't. **`TopBar.svelte`** flipped the disabled "Anki" entry to `openAnkiModal` (ungated,
  like egui); **`+page.svelte`** renders the modal next to the other two. Checks:
  `cargo check -p yomine-tauri` ✓ 0 errors, `-p yomine` ✓ 0 errors (egui still compiles against
  the moved guesser), `pnpm run check` ✓ (orchestrator; only the 4 known vite.config.ts errors).
  **Post-verify fix (same day):** first Windows verify hit `effect_update_depth_exceeded` on open —
  the hydrate `$effect` tracked state `hydrate()` both writes and reads (`tempInterval` w@45/r@46,
  `tempMappings` w@43/r via `fetchMappedSamples`), so it re-triggered itself. Fixed with
  `untrack(hydrate)` so the effect depends on the open flag only; same guard applied to
  `WebsocketSettingsModal.svelte` (no loop there, but its hydrate reads `$settings`, which would
  re-hydrate and clobber the staged port on any settings change while open). `IgnoreListModal`
  audited: safe (sync section only writes; post-await reads are untracked).
  **Second post-verify fix (same day) — stale grey "WebSocket server stopped" indicator.**
  Maintainer saw the asbplayer dot grey while the server was demonstrably up (turned green once a
  client connected). Root cause: the backend `player-status`/`anki-status` events emit **only on
  change** (player_task `last_status` dedupe), but `hydrate()` never pulled the resting snapshot —
  so any freshly-(re)loaded webview sat on the store's initial placeholder (`mode:'none'` → grey)
  until the next change. Fix: `ipc.ts` gains `getPlayerStatus()`/`getAnkiStatus()` wrappers (both
  backend commands already existed) and `hydrate()` pulls both alongside settings and seeds the
  stores. Note for future verifies: grey while **MPV is connected** is *correct* parity — the
  engine deliberately shuts the WS server down when MPV connects (`PlayerManager::update` prefers
  MPV) and restarts it when MPV drops. Known residual gap (pre-existing T028 deviation, still
  open): `PlayerStatus` carries no `ServerState`, so egui's Starting (blue) / Error (red) dot
  sub-states still can't render; promoted to **T056** at the maintainer's request.
  **Verify on Windows (`cargo tauri dev`, folds into T046):** Settings→Anki opens;
  with Anki running the status shows Connected and notetypes populate; selecting a notetype
  auto-guesses term/reading (＊ marker + Example values) matching egui's guesses for the same
  deck; Add → row appears, Edit/Delete/rename work; interval edit → dirty ⚠; Save persists
  (`settings.json` `anki_model_mappings` + `anki_interval`) and the knowledge summary recomputes;
  Cancel reverts staged edits but keeps the modal open; Restore Default empties mappings +
  interval 30; with Anki closed → "Error: Anki Offline" inline (no banner) and Refresh retries.
- **T042 DONE (2026-06-11) [US5] — Frequency weights modal; cargo checks green in WSL,
  `pnpm run check` green (only the 4 known vite.config.ts errors + the modal's backdrop a11y
  warning, same pattern as T038b/T040/T041). Uncommitted.** **Backend (one surgical fix, no new
  commands):** `commands/dictionary.rs::set_dictionary_state` now rebakes the stored
  `file.terms`/`file.base_terms` `"HARMONIC"` entries via `manager.get_weighted_harmonic` after
  applying the new state — the Tauri table reads the *baked* `frequencies.HARMONIC` (table.ts)
  whereas egui recomputes it from the manager every frame, so without this the
  `dictionaries-changed` re-fetch returned stale weighted frequencies. (`get_weighted_harmonic`
  skips the `"HARMONIC"` key itself — it only counts names present in the states map — so the
  rebake is idempotent.) **`ipc.ts`:** `DictionaryState` type + `listDictionaries()` +
  `setDictionaryState(name, weight, enabled)` + `onDictionariesChanged`. **`stores/index.ts`:**
  `frequencyModalOpen` + `openFrequencyModal()` + `saveDictionaryStates(entries)` (commits each
  *changed* entry via `set_dictionary_state` — egui saves the whole map in one shot, the
  per-dictionary command makes the changed subset the minimal equivalent; mirrors into the
  `settings` store's `frequency_weights` on success; failure → `lastError` banner "Frequency
  Weights / Failed to save dictionary settings"; returns success so the modal can close);
  `hydrate()` wires `onDictionariesChanged` → `getTerms()` → `fileResult.set` (freq bounds rederive
  via the existing `fileResult` subscription; knowledge summary recomputes via `knowledge_dirty` +
  the background task — both already in place). **New `FrequencyWeightsModal.svelte`**
  (WebsocketSettingsModal pattern: backdrop/Esc/✕, staged vs original, `untrack(hydrate)` on the
  open flag): parity with `gui/settings/frequency_weights_modal.rs` — header row
  (blank/Dictionary/Weight/Value), one row per dict: enabled checkbox (unchecking floors weight at
  0.1, egui guard), name greyed when disabled, *logarithmic* 0.1–5.0 slider (egui
  `Slider::logarithmic`, mapped onto a linear 0..1000 range input) + number input (step 0.05,
  clamped 0.1–5, 2 decimals, "x" suffix), both disabled when the dict is off; "No frequency
  dictionaries loaded." when empty; hydrate = `list_dictionaries` overlaid with
  `settings.frequency_weights` (egui `build_entries`, incl. the settings-less fallback when tools
  aren't loaded); footer ⚠ dirty + Save Settings/Cancel gated on dirty (Cancel reverts staged,
  stays open; Save closes on success) + right-aligned Restore Default (all enabled, weight 1.0,
  staged only). **`TopBar.svelte`** flipped the disabled "Frequency Weighting" entry (ungated,
  like egui); **`+page.svelte`** renders the modal. **Verify on Windows (`cargo tauri dev`, folds
  into T046):** Settings→Frequency Weighting lists the loaded dicts; drag a weight / toggle a dict
  → dirty ⚠; Save persists (`settings.json` `frequency_weights`), the table's Frequency column +
  bounds shift immediately (no reload), and the knowledge-summary bands recompute; Cancel reverts;
  Restore Default → all on at 1.00x; with a dict disabled its slider/value grey out; restart →
  weights persist and apply at load (`apply_frequency_weights`).
- **T043 DONE (2026-06-11) [US5] — POS filters modal (persisted defaults); checks green (same
  results as T042's run). Uncommitted.** No backend changes (`get_pos_catalog` + `save_settings`
  suffice). **Parity answer to the task's question:** egui's save updates *both* — it assigns
  `settings_data.pos_filters` **and** calls `table_state.apply_pos_settings(...)` (app/mod.rs
  ~360), and the modal *opens* seeded from the **live** table snapshot
  (`table_state.pos_snapshot()`, top_bar.rs ~157), not from the persisted defaults. Mirrored
  exactly: open seeds staged from the live `posEnabled` session store (T037 semantics: missing key
  = enabled), Save → `savePosFilters(map)` = `save_settings` with the full map as
  `settings.pos_filters` + `posEnabled.set(map)` so the table refilters immediately
  (`visibleTerms` is derived; egui's extra `ensure_indices` has no Tauri counterpart to call).
  **`stores/index.ts`:** `posModalOpen` + `openPosModal()` + `savePosFilters(filters)` (mirrors
  into `settings` on success; failure → `lastError` banner "POS Filters / Failed to save
  settings"; returns success so the modal can close). **New `PosFiltersModal.svelte`**
  (same modal pattern, `untrack(hydrate)`): parity with `gui/settings/pos_filters_modal.rs` —
  toggle chips in vertical-fill columns (CSS multi-column ≈ egui's width-based column flow), egui's
  exact chip list/order: strong parent "Noun" chip gating greyed-uninteractable
  ProperNoun/CompoundNoun/AdjectivalNoun/SuruVerb sub-chips (their on-state preserved while
  gated), then the 20 remaining chips; **NounExpression is hidden but still saved** (egui's `raw`
  map covers all 26 variants; staged map spans the full `posCatalog`); footer ⚠ dirty + Save/
  Cancel gated on dirty (Cancel reverts, stays open; Save closes on success) + Restore Default →
  egui `default_pos_map` (all on except Unknown/Other/Symbol/KanaExpression, staged only).
  **Flagged (pre-existing T037 fork, not changed here):** egui's *session* table defaults turn
  KanaExpression off (`PosToggle::off`) even with empty settings, while T037's `posEnabled` treats
  missing keys as enabled — the modal seeds from the live Tauri state, so on a fresh profile it
  shows KanaExpression on where egui would show it off; defaults converge after the first Save.
  **`TopBar.svelte`** flipped the disabled "Part of Speech Filters" entry (ungated, like egui);
  **`+page.svelte`** renders the modal. **Verify on Windows (`cargo tauri dev`, folds into
  T046):** Settings→Part of Speech Filters opens seeded with the table's current POS state (toggle
  something in TableControls first to confirm); chip toggles → dirty ⚠; Noun off greys its four
  sub-chips and the table hides noun terms only after Save; Save persists (`settings.json`
  `pos_filters`, incl. NounExpression) *and* refilters the table + updates the TableControls POS
  count immediately; Cancel reverts; Restore Default turns everything on except
  Unknown/Other/Symbol/KanaExpression; restart → saved defaults seed the table (T037 hydrate).
  **Post-review UX deviation (maintainer decision, 2026-06-11):** having two POS surfaces (T037's
  header popover + this modal, both inherited from egui) was judged redundant — the TableControls
  POS popover was **removed**; the header "POS (n/m)" control is now a button that opens this
  modal, making it the single POS surface. Consequence: POS changes always go through
  Save (persisted) — there is no session-only POS tweaking anymore, unlike egui. `setPos`/
  `setAllPos` + popover markup/styles deleted from `TableControls.svelte`; the All/None quick
  actions only exist as the modal's Restore Default now. Re-verify: header POS button opens the
  modal; count still tracks `posEnabled`.
- **NEXT options:**
  - **Other Settings modals** (Anki settings, Frequency Weighting, POS Filters, Setup Checklist) —
    backend commands exist (`list_anki_models`/`list_dictionaries`/`set_dictionary_state`/
    `get_setup_status`); each builds its modal + `ipc.ts` wrapper and flips its TopBar entry.
  - **T025 verify** still needs an interactive `cargo tauri dev` (maintainer).
- **Deferred (tracked, intentional):**
  - **Furigana on numeric compounds** (found in the T032 verify): `8月22日` → がつにち — digits get
    no reading and the kana isn't aligned to the right characters. Engine-side
    (tokenization/reading alignment), affects egui too; fold into **US6/T047** fixtures.
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
- [x] T025 **Verify Phase B**: `cargo tauri dev` opens; startup loads tools (progress shows);
      `process_file` on a real SRT renders the term list. ✓ Verified on Windows — opened an SRT,
      208 terms / 335 sentences / 51% comprehension rendered. (Two bring-up bugs fixed: dialog
      call moved inside try/catch + error banner; rows re-keyed off `lemma_form+reading` since
      `Term.id` isn't unique.)

**Checkpoint**: IPC foundation proven end-to-end; UI stories can proceed in parallel.

---

## Phase C: Port the UI to parity

Each story is verified against the egui app (quickstart.md). Shell first (T026–T028) since all
stories render inside it.

### Shell (prerequisite for all stories)

- [x] T026 App shell + theme: `src/app.css` (theme tokens mirroring the egui **Dracula** palette —
      `dracula`/`dracula_light` from `src/gui/theme.rs` — dark default, `:root[data-theme='light']`
      override) + `+layout.svelte` (applies dark/light + serif class from `settings`) +
      `+page.svelte` restructured into the `.app-shell` top-bar/main IA. Round trip still works.
      `pnpm build` ✓. (Full menu = T028; virtualized table = T029; font faces = T027.)
- [x] T027 [P] Fonts: bundle Noto Sans/Serif JP as `@font-face`; serif/sans toggle → CSS class
      driven by `settings.use_serif_font`.
- [x] T028 Top bar `lib/components/TopBar.svelte`: menu entries opening the modals (file, anki,
      websocket, ignore list, freq weights, POS filters, analyzer, setup checklist) + status
      indicators (anki/player) from `statusStore`. **Backend command batch DONE (2026-06-09,
      cargo-green WSL); frontend `TopBar.svelte` code-complete (NOT built — Windows), uncommitted.**
      Full menu shell built; entries whose modal isn't built yet are disabled-pending (each later
      modal task flips its entry on). See the Progress block (T028 backend + T028 frontend entries)
      for the as-built record. Verify on Windows folds into the next interactive `cargo tauri dev`.

### US1 — Mine vocabulary from a file (P1) 🎯 MVP

> **SPEC CORRECTION (2026-06-07, maintainer review):** egui shows the example sentence **inline**
> in each row (Term │ Sentence │ Frequency │ POS), not behind an expander; the term's reading is
> furigana **above** the word. The original tasks said "expand a term," which drove an expandable
> build (committed in `a92800b`/T030). T029/T030 below are re-scoped to the inline "lean first
> pass" the maintainer chose; the existing components are largely reused, restructured.
- [x] T029 [US1] (rework) `lib/components/TermTable.svelte` — **rework to the egui inline row shape**:
      four columns **Term │ Sentence │ Frequency │ POS** (drop the separate sentence-count /
      comprehension columns — those move into the Sentence cell / nav, deferred). Term cell shows
      the lemma with its reading as **furigana above** it (not a side column). Default sort =
      frequency ascending. **Reuse (already done & correct):** `harmonic` sort, `？` at u32::MAX,
      `posColor` groups, `posCatalog` labels, frequency formatting. **Remove:** row-expansion
      (`<button>` toggle, `expandedKey`). **Deferred:** virtualization (`@tanstack/svelte-virtual`);
      interactive sort/filter/search = T037.
- [x] T030 [US1] (rework) `lib/components/SentenceView.svelte` — **render inline in each table row** (not an
      expander): `SentenceDto.segments` as `<ruby><rt>` furigana (kanji spans only), POS-colored,
      term's own segments highlighted. **Reuse (done & correct):** the furigana/highlight rendering,
      the `SegmentDto.surface` + hiragana `reading` backend enrichment (`wana_kana` in the tauri
      crate; UI never byte-slices UTF-8), index-based occurrence resolution + byte-offset highlight
      (mirrors `sentence_column.rs`: `sentences[ref.0]`, `ref.1`=offset), shared `lib/pos.ts`.
      **Deferred per "lean first pass":** clickable timestamp→seek = **US3/T035**; per-sentence
      comprehension indicator + ◀ n/m ▶ multi-sentence nav = a **US1 sentence-polish follow-up**
      (now **T030b**). **Note:** expression highlighting is approximate (egui
      has special `find_expression_segments`).
- [x] T030b [US1] (sentence polish) Re-add the deferred per-sentence affordances under the inline
      sentence cell (egui `sentence_column.rs`): the **◀ n/m ▶ multi-sentence nav** (browse a term's
      multiple `sentence_references` in place — the old `SentenceView` already resolved them
      index-based) and the **per-sentence comprehension indicator** (egui's 5-bar gradient, shown
      only once Anki filtering is active). Restores AS2's "browsed in place" + comprehension
      conveyance. Clickable timestamp→seek stays in **US3/T035**. Split out of T029/T030 by the
      2026-06-07 maintainer "lean first pass" call.
- [x] T030c [US1] (sentence wrapping) Long example sentences must **wrap to multiple lines** in the
      Sentence cell (egui parity: `sentence_widget.rs` renders via `ui.horizontal_wrapped`), not
      truncate/overflow as one line. **Root cause:** `Furigana.svelte` wraps each word in an
      `display:inline-block` box (so a reading can't overhang its neighbour), and Svelte
      (`preserveWhitespace:false`) strips the whitespace between the per-word `<span>`s in
      `SentenceView`, so adjacent atomic inline-blocks give WebKit **no soft-wrap opportunity between
      words** → the sentence renders as one unbreakable line that overflows the `1fr` grid column.
      **Fix:** restore an inter-word break opportunity without reintroducing overhang — emit a `<wbr>`
      (or `​`) between the word boxes in `SentenceView`'s `{#each}` (keeps each word atomic;
      lines break only between words). **Verify on Windows:** a long sentence wraps to 2+ lines inside
      its row (row grows in height); furigana still centres per word with no overhang/merge; no
      horizontal scrollbar on the table.
- [x] T031 [US1] File open + drag-drop + **no-file landing state (FR-001)** — code-complete,
      frontend verified on Windows 2026-06-11 (folded into T032):
      `open_file_dialog`→`process_file`; Tauri `onDragDropEvent` for drops (O2); loading overlay
      from `overlayStore`; error banner on failure (don't clobber existing results — done).
      Landing state (egui parity): "no file loaded" message + "drop a file anytime" hint + an
      "Open file" action surfacing **recent files** (needs a `get_recent_files` backend command
      reusing `gui/recent_files.rs`, O3). Replaces the current bare "Open a subtitle…" placeholder.
- [x] T032 [US1] **Verify** against egui: same term count/order/readings/POS/frequencies; furigana
      renders; drag-drop parity.

### US2 — Hide known words via Anki (P1)

- [x] T033 [US2] Wire cached-load + background live refresh: consume `terms-refreshed`; show
      comprehension column values; `anki-status` indicator in top bar.
- [x] T034 [US2] **Verify**: same hidden terms + comprehension % as egui; offline load works;
      live refresh updates in place.

### US3 — Seek the video player (P2)

- [x] T035 [US3] Timestamp UI in `SentenceView`: clickable timestamp → `seek_timestamp(secs,
      label)`; reflect `player-status` (mode/no-player) and surface the no-player error.
- [x] T036 [US3] **Verify**: asbplayer + MPV seek; MPV preferred when both; no-player handled.
- [ ] T056 [P] [US3] Surface `ServerState` in `PlayerStatus` (closes the T028 indicator
      deviation): add the websocket server state (running/starting/error+message/stopped, from
      `ws.get_server_state()`) to the DTO + `ipc.ts`, and drive the TopBar asbplayer dot from it
      like egui `show_status_indicators` — green has-clients, yellow Running, blue Starting,
      red Error (tooltip carries the message), grey Stopped. Found during the 2026-06-11
      T040 verify session: a bind failure is currently indistinguishable from "waiting".

### US4 — Refine & search (P2)

- [x] T037 [US4] Table controls `lib/components/TableControls.svelte`: sort selector (frequency,
      chronological, sentence count, comprehension), search box, POS multiselect filter, and a
      frequency-range double-slider. All operate client-side on `termsStore` (research R6),
      mirroring `gui/table/{sort,filter,search}.rs`.
- [x] T038 [US4] Ignore list (minimal): right-click term → `add_to_ignore_list`; ignore-list modal
      (`get/remove_from_ignore_list`); table updates from returned `FileLoadResult`.
      (Backend green in WSL; frontend not built — verify on Windows, folds into T039.)
- [x] T038b [US4] Ignore-list modal → **full egui parity** (`gui/settings/ignore_list_modal.rs`).
      DONE 2026-06-07 (backend green in WSL; frontend code-complete, Windows-build pending). See the
      T038b note in Progress for the as-built summary. `remove_from_ignore_list` + its ipc wrapper
      kept for API completeness; the modal stages + persists via `save_ignore_list`.
      Adds the management features T038 left out, per FR-007. **Backend** (new commands in
      `src-tauri/src/commands/ignore.rs`, registered in `lib.rs`; DTOs in `dto.rs` + `data-model.md`):
      `get_ignore_list_full` → `IgnoreListView { terms, files: IgnoreFileView[] }` (per-file `exists`
      + `term_count` via `IgnoreList::{file_exists, load_terms_from_file}`); `import_ignore_file` →
      `IgnoreFileView|null` (`.txt` open dialog + load); `refresh_ignore_file(path)` →
      `IgnoreFileView`; `save_ignore_list(terms, files)` → `FileLoadResult|null` (`set_terms` +
      `set_files` + `reload_file_cache` + reapply filters — the single staged-commit point);
      `get_default_ignored_terms` → `string[]` (`DEFAULT_IGNORED_TERMS`); `export_ignore_list(terms)`
      → `string|null` (`.txt` save dialog + newline-join write). **Frontend** (`IgnoreListModal.svelte`):
      "Add New Term" input (reuses staged temp state); "Search Terms" filter (reuse
      `lib/table::matchesSearch`); file pills (📄 name · checkbox enable · ↻ refresh · × remove ·
      missing state) + "+ Import File"; pill layout for terms (× remove); "Manual: N | From Files: M"
      counts; staged Save/Cancel with a ⚠ modified indicator (dirty = temp ≠ original); "Restore
      Default"; "Export…". The row right-click add stays immediate (`add_to_ignore_list`); the modal
      stages all edits and persists on Save. **Note:** with staged save, `remove_from_ignore_list`
      goes unused by the modal — keep for API completeness or drop (decide at impl). Backend is
      cargo-checkable in WSL; frontend builds/verifies on Windows. Folds its verify into T039.
- [ ] T039 [US4] **Verify**: each sort/filter/search/ignore action yields the same visible set as
      egui; the ignore-list modal (T038b) matches egui — manual add, search, file import/toggle/
      refresh/remove, Save/Cancel, Restore Default, Export.

### US5 — Configure & personalize (P2)

- [x] T040 [P] [US5] Anki settings modal: `list_anki_models`→ note-type/field mapping UI with
      field guessing + live `anki-status`; save via `save_settings`.
- [x] T041 [P] [US5] WebSocket settings modal: edit port → `set_websocket_port`.
- [x] T042 [P] [US5] Frequency weights modal: `list_dictionaries` + `set_dictionary_state`;
      consume `dictionaries-changed` and re-fetch terms.
- [x] T043 [P] [US5] POS filters modal: default POS visibility from `get_pos_catalog` +
      `settings.pos_filters`.
- [x] T044 [P] [US5] Theme + font toggles wired to `save_settings` (uses T026/T027).
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

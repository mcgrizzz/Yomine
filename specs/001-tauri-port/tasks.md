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
- **T045 DONE (2026-06-13) [US5] — setup checklist modal + incomplete banner, the last US5 modal.**
  Parity with egui's `setup_checklist_modal.rs` + `setup_banner.rs`. New files:
  **`SetupChecklistModal.svelte`** (6 check items in egui order, ✓ green Complete / ✕ red
  Incomplete-required / ◯ grey Incomplete-optional, per-item help-docs + action buttons, Close
  footer) and **`SetupBanner.svelte`** (amber rgb(180,140,0) clickable strip → opens the modal).
  **Wiring:** `ipc.ts` gained the `SetupStatus` type + `getSetupStatus()` wrapper (the command was
  registered since T028 but unexposed in JS). `stores/index.ts` gained `setupStatus` writable +
  `refreshSetupStatus()` (best-effort re-pull), `setupModalOpen` + `openSetupModal()`, and the
  `showSetupBanner` derived store (= freq dict missing OR Anki model mappings empty, mirroring
  egui's `should_show_banner`; the Anki bit reads the live `settings` mirror so a Save reflects
  immediately). `get_setup_status` probes Anki/player live, so it's a command not an event: pulled
  once at the end of `hydrate()` (after tools load, when the freq-dict bit resolves) and re-pulled
  on `dictionaries-changed`. `TopBar.svelte` flipped the disabled "Setup Checklist" entry to
  `openSetupModal`. `+page.svelte` renders `<SetupBanner/>` between TopBar and main and mounts
  `<SetupChecklistModal/>`. The modal self-hydrates on open via the `untrack` pattern
  (`if ($setupModalOpen) untrack(() => refreshSetupStatus())`) — read-only, no staged edits.
  **Layout change:** `.app-shell` switched from `grid-template-rows: auto 1fr` to a flex column
  (TopBar + optional banner are auto, main flex-grows) so the conditional banner inserts a third
  row cleanly without breaking main's height.
  **DTO addition (item-6 decision):** egui's item 2 (default dict) vs item 6 (additional dicts)
  differ only by COUNT (item 6 wants >1 dict), but the DTO only had `has_frequency_dict: bool`.
  Added `frequency_dict_count: usize` to `SetupStatus` (dto.rs) + setup.rs (engine untouched —
  reads `frequency_manager.get_dictionary_names().len()`; the bool is now `count > 0`). Item 6's
  status is `count > 1`.
  **LoadFrequencyDictionary scope-gap handling:** the two "Install Dictionary" buttons (items 2 &
  6) have **no Tauri command** — the freq-dict import is a separate File-menu task (still disabled
  "Coming soon" in TopBar). Rendered them **disabled with a "Coming soon" title** (mirrors the
  File-menu precedent; clearer than wiring a button labelled "Install Dictionary" to open docs).
  Item 6 still gets its working "📖 View Docs" button (opener plugin); item 2 has no help_url in
  egui so it has neither working action (default dict auto-downloads → normally Complete anyway).
  **No JS dep added** — `@tauri-apps/plugin-opener` was already in package.json (and capabilities
  grant `opener:default`); `openUrl` imported from `@tauri-apps/plugin-opener` resolved with no
  missing-module error in `pnpm check`. **Build matrix:** `cargo check -p yomine-tauri` ✓ (5
  pre-existing dead-code warnings); `cargo check -p yomine` (egui) ✓ (3 pre-existing deprecations);
  `pnpm run check` → only the 4 known vite.config.ts errors + backdrop a11y warnings (mine matches
  the other modals' pattern). **Verify on Windows (`cargo tauri dev`, folds into T046):**
  Settings→Setup Checklist opens the modal; with all green, banner is hidden; force an incomplete
  state (no file / Anki offline / clear mappings) → amber banner appears and click opens the modal;
  item icons/colors match (Tokenizer ✓ once loaded, default dict ✓ after auto-download, AnkiConnect
  ✓/✕ on live probe, Anki Notetypes ✕→✓ after a mapping save, player ✕→✓ when asbplayer/mpv
  connects, additional dicts ◯ optional until >1 dict); "Setup Anki" / "Configure WebSocket" open
  their modals and close the checklist; "📖 View Docs" opens the URL in the browser; the two
  "+ Install Dictionary" buttons are disabled ("Coming soon").
- **T047 BACKEND DONE (2026-06-13) [US6] — analysis commands; frontend modal pending.**
  Ports egui's `gui::frequency_analyzer::modal` + `TaskManager::{analyze_frequency,
  export_frequency}` to the IPC layer. **New file `src-tauri/src/commands/analysis.rs`** (module
  pattern like `commands/{file,dictionary}.rs`; `pub mod analysis;` added to `commands/mod.rs`).
  **Four commands, all registered in `lib.rs` invoke_handler:**
  - `start_analysis(paths: Vec<String>, progress: Channel<AnalysisProgressDto>) -> Result<AnalysisPreview, String>`
    — async. Brief lock to `clone()` the Arc-backed `LanguageTools` (Err "Language tools are still
    loading" if None) + grab the `analysis_cancel` Arc (reset to false first). Pre-sums file sizes
    for `total_bytes` (ETA), then runs the **synchronous CPU-heavy** `analyzer::analyze_files` on
    `tauri::async_runtime::spawn_blocking` (no lock held across it). The engine progress callback
    `(file_idx_1based, message, file_size)` accumulates `bytes_processed` (AtomicU64) and computes
    the smoothed `eta_secs` via `tools::analysis::calculate_smoothed_time_estimate` (alpha=0.3,
    prev-estimate in a Mutex), pushing an `AnalysisProgressDto` per file over the cloned `Channel`.
    On Ok: stores the full `FrequencyAnalysisResult` in `AppState.last_analysis` under a brief lock,
    builds the preview, **emits `analysis-complete`** with it (contract lists both return + event —
    emitted for store consistency), returns the preview. On the cancel Err (engine returns
    "Analysis cancelled by user"): **emits `analysis-cancelled`** + returns Err. Other Err → Err
    (frontend banner).
  - `cancel_analysis()` — sync; flips `analysis_cancel` to true (engine checks per-file).
  - `export_analysis(output_dir: String, options: ExportOptions) -> Result<String, String>` —
    async. Brief lock clones `last_analysis` out (Err "No analysis to export" if None), runs
    `export_yomitan_zip` and/or `export_csv` per the `options` flags on `spawn_blocking`, maps empty
    option strings → `None` (egui parity), returns "✓ Export successful to: {dir}" (or joined
    errors), and **emits `export-complete { ok, message }`**.
  - `find_analysis_files(dir: String) -> Vec<String>` — sync; **addition beyond the original
    contract** (noted as allowed). Wraps `find_supported_files_recursive` so the frontend can turn a
    picked folder into the supported-file list to build its selection tree. The tree-building +
    selection UI is the frontend batch's job.
  **DTOs (dto.rs):** `AnalysisProgressDto { total_files, current_file (1-based), message,
  total_bytes, bytes_processed, eta_secs: Option<f32> }`; `AnalysisPreview { entries:
  Vec<AnalysisPreviewEntry>, total }` with `AnalysisPreviewEntry { term, reading: Option<String>,
  frequency, count }`. Preview is sorted by frequency desc and **capped at 250** (mirrors egui's
  `RESULTS_DISPLAY_LIMIT`); `total` is the full unique-lemma count before the cap. The engine
  `ExportOptions` (already serde) is **reused directly** in the `export_analysis` signature — no
  re-declared DTO. Event-name constants already existed in `events.rs`; `ExportComplete` payload
  reused. **ZERO engine changes.** The `analysis_cancel`/`last_analysis` dead-code warnings are now
  cleared (commands consume them). **Frontend batch still needs:** `ipc.ts` wrappers + types for all
  four commands, an `analyzer` store, the `FrequencyAnalyzerModal.svelte` (file/folder pick → tree
  via `find_analysis_files` → `start_analysis` with a `Channel` for progress/ETA + cancel button →
  results table from `AnalysisPreview` → export form driving `export_analysis`), `listen`ers for
  `analysis-complete`/`analysis-cancelled`/`export-complete`, and flipping the TopBar
  "Frequency Analyzer" entry. Folder picking reuses the dialog plugin (`@tauri-apps/plugin-dialog`).
  **Checks:** `cargo check -p yomine-tauri` ✓ (0 errors, 0 warnings); `cargo check -p yomine` (engine
  untouched) ✓ (3 pre-existing egui deprecations). Frontend not built this batch — **the frontend
  modal is the next batch.**
- **T047 FRONTEND DONE (2026-06-13) [US6] — analyzer modal.** Ports
  `gui::frequency_analyzer::{modal,export_form,results_table,progress_widget}.rs` to a Svelte state
  machine. **New files:** `src-tauri/ui/src/lib/components/FrequencyAnalyzerModal.svelte` (the driver)
  + `src-tauri/ui/src/lib/fileTree.ts` (selection-tree builder). **Edited:** `ipc.ts` (types +
  wrappers + event helpers), `stores/index.ts` (`analyzerModalOpen` + `openAnalyzerModal`),
  `TopBar.svelte` (flipped the disabled Tools→"Frequency Analyzer" entry to `openAnalyzerModal`,
  gated on `toolsReady`), `routes/+page.svelte` (mount).
  - **ipc.ts:** added `AnalysisProgressDto`, `AnalysisPreview`/`AnalysisPreviewEntry`,
    `ExportOptions` + `defaultExportOptions()` (Yomitan on, rest off/empty — mirrors the engine
    default), `ExportCompletePayload`. Wrappers: `findAnalysisFiles(dir)`, `startAnalysis(paths,
    onProgress)` (`new Channel<AnalysisProgressDto>()` + `channel.onmessage`, mirroring `processFile`),
    `cancelAnalysis()`, `exportAnalysis(output_dir, options)` (passed as `{ outputDir, options }` —
    Tauri camelCases the `output_dir` arg). Event helpers `onAnalysisComplete`/`onAnalysisCancelled`/
    `onExportComplete` exported **for parity but intentionally NOT wired** (see below).
  - **Promise vs. events:** the modal drives its whole flow off the `startAnalysis`/`exportAnalysis`
    **promises** (the contract's stated source of truth, mirroring `processFile`). The three events
    are left as belt-and-suspenders / for any future global observer; subscribing them in the modal
    would double-fire the same transition, so they're deliberately unused there. Flagged in both
    `ipc.ts` and the component header.
  - **Selection tree:** `fileTree.ts` mirrors the spirit of `tools::analysis::file_tree.rs` —
    `buildFileTree(paths)` trims the common-ancestor dirs (never the filename) and builds a nested
    dir/file tree (dirs-first, alpha sort; splits on both `/` and `\\` for Windows paths);
    `collectChecked(node, pred)` gathers a subtree's files. The modal accumulates picked paths into a
    deduped set across multiple Add-Files/Add-Folder actions, renders the tree with a recursive
    `{#snippet}`, per-file + per-dir cascade checkboxes (dir shows indeterminate when partial),
    everything checked by default, collapsible dirs, and an "N of M files selected" count. The checked
    file paths feed `startAnalysis`.
  - **State machine** (`phase`: selecting → analyzing → results → exporting → complete | error, all
    component-local `$state`, fresh each open): **analyzing** shows a progress bar (current/total
    files), the message line, and ETA from `eta_secs`, with a Cancel button (`cancelAnalysis()` → the
    rejected promise routes back to `selecting`; **cancel is not an error**). **results** shows the
    preview table (rank │ term │ reading │ freq, "N unique terms / showing top K") beside the export
    form (Title/Author/URL/Revision prefix/Description + the four format checkboxes; Export… disabled
    unless Yomitan or CSV is checked → folder pick → `exportAnalysis`). **complete** shows the success
    message with "← Back to Results" / "New Analysis"; **error** shows the message with "Start New
    Analysis". Modal owns its error display inline (egui parity) — does not touch the global banner.
  - **`balance_corpus`/`show_top` gap → promoted to T057 (maintainer: parity before changes).** The
    first pass omitted both egui toggles (backend `start_analysis` takes no analysis options). This
    is NOT acceptable as a permanent deviation — `balance_corpus` (CorpusBalancer down-sampling) and
    `show_top` (Top 250 / Bottom 250 radio) must be restored for US6 parity. **T057** tracks the
    backend param + bottom-slice preview + the two UI controls; it blocks T048 full sign-off.
  - **Checks:** `pnpm run check` → only the **4 known vite.config.ts `process` errors** (+ the paired
    `tsconfig.json` node-types warning) and the **backdrop a11y warning** on the new modal (identical
    to every other modal). **Zero new errors.** No new npm deps (dialog plugin already present). No
    Rust touched. Frontend not built in WSL (Windows `node_modules`).
  - **Verify on Windows (`cargo tauri dev`, folds into T048):** open Tools→Frequency Analyzer; (1)
    Add Files (multi-select srt/ass/ssa/txt) → tree renders, count updates; (2) Add Folder → recurses
    via `find_analysis_files`, dedupes against already-added files; (3) toggle a dir checkbox →
    cascades to its files (indeterminate when partial); uncheck some → "N of M selected" tracks;
    (4) Analyze → live progress bar + message + ETA countdown; (5) Cancel mid-run → returns to
    selection, no error; (6) re-run to completion → results table (ranks, readings, freq) + export
    form; (7) Export as Yomitan ZIP to a folder → ✓ message; (8) Export as CSV (and both) → files land;
    (9) error states: zero files selected (button disabled), and force an export failure to see the
    inline error + "Start New Analysis".
- **T057 DONE (2026-06-14) [US6] — analyzer parity: balance_corpus + Top/Bottom 250.** Restored the
  two egui analysis options the T047 first pass omitted; spans a shared DTO (backend + frontend).
  - **(a) Balance corpus by source.** `start_analysis` gained a `balance_corpus: bool` param (after
    `paths`, before `progress`; Tauri maps it to JS `balanceCorpus`). When true the command runs
    `CorpusBalancer::new(file_paths).balance()` (imported from `yomine::tools::analysis` alongside
    `analyze_files`) on the path list **before** pre-summing `total_bytes` and before `analyze_files`,
    so progress/ETA reflect the balanced (down-sampled) set — matches egui (`modal.rs` balances first,
    then pre-sums). `file_paths` is now `mut`; runs in the command body (cheap path math, no AppState
    lock held), not in `spawn_blocking`. Frontend: `balanceCorpus` `$state(false)` (egui
    `AnalysisOptions::default`), a checkbox in the SelectingFiles footer next to "Analyze files" with
    label "Balance corpus by source" + `title` hover "Uses trimmed mean (10% trimming) to calculate
    balanced sample sizes."; threaded through the `startAnalysis(paths, balanceCorpus, onProgress)`
    ipc wrapper (`invoke('start_analysis', { paths, balanceCorpus, progress })`) and the modal's
    `analyze()` call site.
  - **(b) Top 250 / Bottom 250 radio.** `AnalysisPreview` (dto.rs) gained a `bottom:
    Vec<AnalysisPreviewEntry>` field alongside `entries` (top, unchanged name — frontend already reads
    it). In `build_preview`: after the freq-desc sort and recording `total`, `bottom =
    entries[len.saturating_sub(PREVIEW_LIMIT)..].to_vec()` (the last ≤250, still desc order — egui's
    `entries[len-250..]`), then `entries.truncate(PREVIEW_LIMIT)` for the top. Small corpora (total ≤
    250) → top and bottom are the same list, matching egui (slices the same full list). Frontend:
    `bottom` added to the `AnalysisPreview` type in ipc.ts; modal gained `showTop` `$state(true)`
    (egui default) + a `displayedEntries` `$derived` (`showTop ? preview.entries : preview.bottom`);
    the results table renders a "Show: ● Top 250 ○ Bottom 250" radio pair above it and iterates
    `displayedEntries` (rank by position in the rendered slice, `i + 1`, as egui does).
  - **Checks:** `cargo check -p yomine-tauri` ✓ 0 errors; `cargo check -p yomine` ✓ (engine untouched,
    3 pre-existing egui deprecations only); `pnpm run check` → only the 4 known vite.config.ts
    `process` errors + the existing backdrop a11y warnings (the modal's 192:3 warning is the
    pre-existing backdrop, not new). Zero new errors. No engine changes (CorpusBalancer already
    existed). No commit.
  - **Verify on Windows (`cargo tauri dev`, folds into T048):** (1) check "Balance corpus by source"
    before Analyze → the analyzed set is down-sampled (fewer total files/bytes in progress vs.
    unchecked on a multi-source corpus); leave unchecked → full set. (2) In results, the Top 250 /
    Bottom 250 radio switches the table: Top shows the highest-frequency terms, Bottom switches to the
    lowest-frequency terms (last ≤250 of the desc list); on a small corpus (≤250 unique) both show the
    same list.
- **T048 DONE (2026-06-14) [US6] — analyzer verified on Windows (`cargo tauri dev`).** US6 is now
  fully signed off. Walked the maintainer through the folded T047+T057 checklist; all four groups
  pass: **(A) Selection** — Add Files multi-select renders the tree + count; Add Folder recurses
  (`find_analysis_files`) and dedupes against already-added; dir checkbox cascades to its files with
  indeterminate on partial; "N of M selected" tracks. **(B) Analysis** — progress bar shows
  `current/total` + message + "Estimated Xs remaining" ETA; **Cancel mid-run returns to selection
  with no error** (the `cancelled` reject drives the `selecting` transition, not an error banner);
  re-run completes to the results table (#/Term/Reading/Freq) + export form. **(C) Parity controls
  (T057)** — Top 250 / Bottom 250 radio switches the table (Bottom = lowest-freq terms); Balance
  corpus by source down-samples the analyzed set on a multi-source corpus. **(D) Export & errors** —
  Yomitan ZIP + CSV (and both) export to a chosen folder and files land; zero files selected disables
  Analyze. **Skipped:** the forced export-failure inline-error path (not practical to trigger a
  write failure interactively; the code path is `doExport` catch → `phase = 'error'` → red message +
  "Start New Analysis", unchanged from the verified error-render pattern). No code changes — verify
  only.
- **T049 DONE (2026-06-14) [US7] — knowledge summary widget; T050 verify PENDING.** Ports egui's
  `gui/table/summary.rs::ui_knowledge_profile`/`ui_knowledge_summary` (the global JLPT + frequency
  band card). The handoff billed this as "mostly frontend, backend already exists" — **two backend
  gaps were found and closed:**
  - **No `get_knowledge_summary` command existed** (only the `knowledge-summary` event, which the
    background task emits *only* on `knowledge_dirty`). Per the established convention (events that
    fire on change need a one-shot pull — the anki/player fix this session), added
    `commands/knowledge.rs::get_knowledge_summary -> Option<KnowledgeSummaryDto>` returning a cached
    snapshot; registered in `commands/mod.rs` + `lib.rs`. `AppState` gained
    `knowledge_summary: Option<KnowledgeSummaryDto>` (state.rs); `background.rs` now caches the DTO
    into state **and** emits it. ipc `getKnowledgeSummary()` + a hydrate pull in
    `stores/index.ts` (added to the `Promise.all`, `if (summary) knowledge.set(summary)`).
  - **Wire-format mismatch:** the engine `KnowledgeSummary` holds `jlpt: Vec<(JlptLevel, BandStats)>`
    / `frequency: Vec<(String, BandStats)>` — tuples serialize as positional JS arrays, but the TS
    `KnowledgeSummary` interface (written in T016, never exercised) expects `{ level, stats }` /
    `{ label, stats }` objects. Added `KnowledgeSummaryDto` + `JlptBand`/`FrequencyBand` to `dto.rs`
    (`from_summary` maps `level.label()` → `level` string; `BandStats` already serializes cleanly as
    `{ coverage, comprehension, total }`). Both the event emit and the new pull go through the DTO so
    the wire shape matches the TS interface. **No engine changes.**
  - **Frontend:** `components/KnowledgeSummary.svelte` — mode `$state` ('coverage'|'estimate', egui
    `KnowledgeMode` default Coverage); clickable title + ⇄ button toggle (hover "Switch to {other}");
    JLPT row (N5→N1, 40px bars) + frequency row (<1.5k→<20k, 28px bars), each a track+fill mini-bar
    with the band label beside it and a `{got}/{total} {pct}%` tooltip (freq repeats the label, egui
    `label_in_hover`). `frac` picks coverage vs comprehension; **vivid** red→yellow→green bar color
    (egui `coverage_color`, NOT the gray-blended `comprehensionColor` the text uses). Card hides when
    the summary is null/empty (egui parity). Mounted in `+page.svelte` top-right of a new
    `.header-row` (title/comprehension/counts on the left), mirroring egui's `horizontal_top`.
  - **Checks (orchestrator-run):** `cargo check -p yomine-tauri` ✓ clean; `cargo check -p yomine` ✓
    (3 pre-existing egui `Panel::show` deprecations only); `pnpm run check` → only the 4 known
    vite.config.ts `process` errors + tsconfig node-types warning + the pre-existing backdrop a11y
    warnings; **zero new errors/warnings** (the card uses real `<button>`s). No commit.
  - **Verify on Windows (`cargo tauri dev`, T050):** with a loaded file + Anki vocab cache + a
    frequency dict, the card shows top-right; JLPT (N5..N1) + frequency (<1.5k..<20k) bars render;
    clicking the title or ⇄ toggles "Anki Coverage" ↔ "Estimated Knowledge" and the bar fills change;
    hover shows `n/total pct%`; **band values match egui** for the same Anki snapshot (T050's gate).
    Card is absent when no Anki cache exists. Known cosmetic nit: the wrapped header block in
    `+page.svelte` is one indent level shallow (functional, svelte-check clean).
- **T051 DONE (2026-06-14) — bundle-resources audit; no code change needed.** Walked the resource
  surface against the current `tauri.conf.json`: (1) **jlpt_vocab.json** is embedded at compile time
  (`include_str!("../../assets/jlpt_vocab.json")`, `src/jlpt/mod.rs:8`) — O1 resolved (research.md
  §O1), so **no `resources` entry is needed** (it's in the binary, not a sidecar file). (2) **Fonts**
  are bundled frontend-side (T027, the egui TTFs as web `@font-face`) — not a Tauri resource. (3)
  **Icon** set is already declared in `bundle.icon` (32/128/128@2x/icns/ico). (4) **Runtime
  downloads** (unidic, frequency dicts, Anki vocab cache) resolve via the engine's unchanged
  `dirs::data_local_dir()/yomine` — identical code path to egui, no port-side change. Net: nothing to
  add to `tauri.conf.json`; the bundle's actual contents are validated by **T052**'s `cargo tauri
  build` (maintainer/per-OS). Ticked as a config audit.
- **Final parity inspection (2026-06-14) → T058/T059/T060 added as a PRIORITY GATE.** After US7
  the maintainer asked for a full egui-vs-Tauri sweep before any new feature/UX work. Compared the
  egui `UiAction` catalog + all three menus (`top_bar.rs`) + table/term/sentence surfaces against the
  Tauri port. **At parity:** all Settings modals, Tools→Analyzer, theme/font toggles, asbplayer/mpv/
  Anki indicators, all 4 sort fields (dropdown — T037 deviation), POS single-surface (intentional),
  freq-range + search, sentence widget (Tauri upgrades hover-reading → furigana), right-click ignore,
  recents (on the landing vs egui's file-open modal — reorganized, not missing), drag-drop, knowledge
  summary, comprehension bars. **Gaps → new tasks:** T058 Open Data Folder (disabled stub vs egui's
  working `open_folder`), T059 term ignore UX (egui Ctrl+Click toggle + grey-in-place + inline undo
  vs Tauri's eager right-click removal), T060 `load_frequency_dictionaries` import (two stubbed
  surfaces). **Already-tracked, same gate:** T056 (asbplayer Error/Starting sub-states). Intentionally
  unmirrored (not gaps): egui's 👁 confirmed-timestamp seek variant (T035). Maintainer directive:
  **T056/T058/T059/T060 jump ahead of any additive feature/UX/UI task** (need not all land at once).
  No code written this turn — tasks added only.
- **T058 DONE (2026-06-15) [parity] — File → Open Data Folder; first of the PRIORITY GATE.** Ports
  egui's `top_bar.rs::open_folder` (which spawns `explorer`/`open`/`xdg-open` on `get_app_data_dir()`)
  to the Tauri opener plugin. **Backend (one new command):** `commands/lifecycle::open_data_folder()`
  → `Result<(), String>` (registered in `lib.rs` after `save_settings`): reads the engine's
  `persistence::get_app_data_dir()` (same `dirs::data_local_dir()/yomine` path egui uses; it
  `create_dir_all`s, so it always exists — no existence guard needed) and opens it with
  `app.opener().open_path(dir, None::<&str>)` (`tauri_plugin_opener::OpenerExt`). `open_path` on a
  directory opens the OS file manager **at** that dir (showing its contents) — exact behaviour parity
  with egui, unlike `reveal_item_in_dir` which would select the dir in its parent. **No capability
  change:** Rust-side `OpenerExt` calls bypass the JS opener scope, so `opener:default` already
  suffices. **Frontend:** `ipc.ts` `openDataFolder()` wrapper; `stores/index.ts` `openDataFolder()`
  action (try/catch → `lastError` "Failed to open data folder", the manual-action error convention,
  mirroring `seekTimestamp`); `TopBar.svelte` imports it and flips the disabled "Open Data Folder"
  File-menu stub to `onclick={() => run(openDataFolder)}` (ungated, like egui). **Checks
  (orchestrator-run):** `cargo check -p yomine-tauri` ✓ clean (28.98s); `pnpm run check` → only the 4
  known `vite.config.ts` `process` errors + the tsconfig node-types warning + the 8 pre-existing
  backdrop a11y warnings — **zero new** (the menu item is a real `<button>`). Engine untouched
  (`get_app_data_dir` already existed) → `-p yomine` / `--no-default-features` / `cargo test`
  unaffected. **Verify on Windows (`cargo tauri dev`):** File → Open Data Folder opens
  `%LOCALAPPDATA%\yomine` in Explorer; matches egui's same menu action.
- **T056 DONE (2026-06-15) [US3] — asbplayer dot sub-states; second PRIORITY GATE task.** Closes the
  long-standing T028/T040 indicator deviation (a WS bind failure looked identical to "waiting").
  Mirrors egui's `top_bar.rs::show_status_indicators` reading `WebSocketManager::get_server_state()`.
  **No engine change** — the `ServerState` enum (Running/Starting/Error(msg)/Stopped) already exists
  in `src/websocket/types.rs` and is re-exported from `yomine::websocket`. **Backend:**
  `events::PlayerStatus` (the `player-status` event + `get_player_status` command DTO) gained two
  fields — `server_state: String` ("running"|"starting"|"error"|"stopped") + `server_error:
  Option<String>` — mapped wire-friendly so the TS side gets clean values and the struct keeps its
  `PartialEq` (the player loop's `last_status` change-dedupe relies on it; an externally-tagged enum
  would have worked too but the string pair is tidier on the wire). `player_task::current_status`
  now `match`es `player.ws.get_server_state()` into the pair (imported `ServerState` alongside
  `WebSocketManager`). Note: when MPV connects the engine stops the WS server → `server_state ==
  "stopped"` → grey dot, which is correct parity (the documented MPV-preferred behaviour). **Frontend:**
  `ipc.ts` `PlayerStatus` interface gained both fields; `stores/index.ts` seeded the
  `playerStatus` default with `server_state: 'stopped', server_error: null`; `TopBar.svelte` rewrote
  the `asbplayer` `$derived` to egui's full mapping — green `#00c800` (running + has-clients), yellow
  `#c8c800` (running, waiting), **blue `#6464c8` (starting)**, **red `#c80000` (error, tooltip =
  `WebSocket server error: {msg}`)**, grey `#646464` (stopped) — adding `BLUE`/`RED` constants and
  updating the indicator comment (the old "DTO can't show sub-states" caveat is now resolved). `mode`
  stays on the DTO (untouched) though the dot no longer reads it. **data-model.md** PlayerStatus line
  updated. **Checks (orchestrator-run):** `cargo check -p yomine-tauri` ✓ clean (25.25s); `pnpm run
  check` → only the 4 known `vite.config.ts` errors + tsconfig node warning + 8 pre-existing backdrop
  a11y warnings — **zero new**. Engine untouched → `-p yomine` / `cargo test` unaffected. **Verify on
  Windows (`cargo tauri dev`):** with a free port, the asbplayer dot is yellow (waiting) then green
  when asbplayer connects; set the WS port to one already in use (Settings→WebSocket Server) → the
  dot turns **red** with the bind-error message on hover; while MPV is connected the dot is grey
  (server stopped, correct); briefly blue on startup. Matches egui's same indicator.
  **Bug found + fixed during T056 verify (2026-06-15) — dot stuck yellow after asbplayer connects on
  startup.** Pre-existing hydrate race (not caused by T056; the old green gate was also `ws_clients >
  0`). `hydrate()` subscribes `onPlayerStatus` first (good) but then applied the one-shot
  `getPlayerStatus()` pull (captured pre-`await` in the `Promise.all`) unconditionally — if asbplayer
  connected during that await window, the 0→1 `player-status` event set the store green, then the
  stale pulled `ws_clients:0` snapshot **clobbered it back to yellow**; since the backend emits only
  on *change* it never re-emitted, so it stuck until a manual disconnect/reconnect forced a fresh
  transition (exactly the reported repro). Fix (`stores/index.ts`): the three change-only pulls now
  guard against clobber — `playerEventSeen`/`ankiEventSeen`/`knowledgeEventSeen` flags set in their
  listeners, and the post-`await` seeds run only `if (!…EventSeen)` so an event that arrived during
  hydrate wins over the stale snapshot (anki + knowledge had the same latent clobber). `pnpm run
  check` still only the 4 known vite.config.ts errors. Frontend-only.
- **T059 DONE (2026-06-15) [parity/US4] — term ignore UX → egui parity; third PRIORITY GATE task.**
  Mirrors egui's `table/mod.rs::ui_col_term`: ignoring a term **mutates the persisted ignore list but
  does NOT re-filter** — the term stays visible, *greyed in place*, and only leaves the table on the
  next `refresh_terms` (the old Tauri behaviour re-filtered eagerly so the row vanished with no undo).
  **Backend (`commands/ignore.rs`):** `add_to_ignore_list`/`remove_from_ignore_list` no longer
  re-filter — `mutate_ignore_list` now just clones the tools handle, mutates+persists the
  `IgnoreList` (`add_term`/`remove_term`, which save to `ignore_list.json`), and returns `()` (was
  `Option<FileLoadResult>`). The `apply_filters`/`load_result`/`base_terms`/`anki_known` re-filter
  path is gone from this fn; those imports stay (still used by the modal's staged `save_ignore_list`,
  which *does* re-filter — unchanged). Module doc + contracts/commands.md rows updated. **Frontend:**
  new `ignoredLemmas` store (`Set<string>` = the live ignore set, egui's `ignore_list.contains()`),
  `refreshIgnoredLemmas()` (pulls `get_ignore_list`, tools-ready-guarded) seeded at the end of
  `hydrate()` and re-pulled after the modal's `saveIgnore`; `addToIgnore` replaced by `toggleIgnore`
  (adds or removes by current membership, surfaces failures via `lastError`, updates `ignoredLemmas`
  optimistically so the cell flips instantly). `ipc.ts` add/remove wrappers now `Promise<void>`.
  `TermTable.svelte`: the term cell gains `class:ignored` (greyed via new `.term.ignored { color:
  var(--comment) }`, egui's weak_text_color), a `title` hover hint ("Ctrl+Click to ignore" /
  "…to UNDO ignore"), an `onclick` that toggles only when **Ctrl (Win/Linux) or Cmd (macOS)** is held
  (plain click left alone for text selection; on macOS Ctrl+Click opens the menu), and the right-click
  context menu now labels **Remove/Add** by `ignoredLemmas` membership and calls `toggleIgnore`. One
  `svelte-ignore a11y_click_events_have_key_events` on the term span (intentional mouse-modifier
  action, no keyboard equivalent in egui either) keeps the check clean. **Re-filter timing:** removal
  from the minable set is deferred to the next `refresh_terms` (manual 🔄 / F5 / auto-on-load) exactly
  as egui does; `ignoredLemmas` = the full ignore list, so after a refresh drops the rows the set is
  still accurate (no clear needed). **Checks (orchestrator-run):** `cargo check -p yomine-tauri` ✓
  (17.9s; one fixed E0308 — `add_term`/`remove_term` return `Result<bool>`, mapped to `()`);
  `cargo check -p yomine` untouched; `pnpm run check` → only the 4 known vite.config.ts errors + 8
  pre-existing backdrop a11y warnings — **zero new**. **Verify on Windows (`cargo tauri dev`):**
  Ctrl/Cmd+Click a term → it greys in place (stays visible), persists to `ignore_list.json`; Ctrl+Click
  again or right-click → "Remove from ignore list" → un-greys; hover shows the hint; hit 🔄 / F5 →
  the greyed terms drop out of the table (deferred re-filter); compare against egui's same cell.
  **Cursor refinement (2026-06-15, maintainer request):** added a pointing-hand cursor on the term
  while Ctrl/Cmd is held (egui `set_cursor_icon(PointingHand)` on ctrl+hover). `TermTable.svelte`
  tracks `ctrlHeld` via window `keydown`/`keyup` (`e.ctrlKey || e.metaKey`, reset on `blur`),
  `class:ignorable={ctrlHeld}` → new `.term.ignorable { cursor: pointer }`. Frontend-only; `pnpm run
  check` unchanged (zero new). Verify: hold Ctrl → term cursor becomes a hand; release → reverts.
- **T060 DONE (2026-07-06) [parity] — `load_frequency_dictionaries` import command; last PRIORITY
  GATE task.** Mirrors egui's File-menu `frequency_utils::load_frequency_dictionaries` and enables
  all three stubbed surfaces. **Backend (`commands/dictionary.rs`):** ONE command instead of the
  contract's drafted import+reload pair (picker lives backend-side like `import_ignore_file`;
  contracts/commands.md updated): `load_frequency_dictionaries(progress: Channel<LoadingMessage>)
  → usize`. Flow: require tools loaded (Err otherwise) → native multi-`.zip` picker
  (`tauri-plugin-dialog` `pick_files`, "Yomitan Frequency Dictionaries" + "All Files" filters =
  egui's `select_frequency_dictionary_zips`) → engine `copy_frequency_dictionaries` (skips
  already-present filenames) → `0` copied = return `Ok(0)`, NO reload (egui parity for cancel /
  all-duplicates) → else `process_frequency_dictionaries` on `spawn_blocking` streaming its
  progress messages over the Channel (mirrors `load_language_tools`) → on success: lock,
  `apply_frequency_weights` (persisted weights onto the new manager), swap
  `tools.frequency_manager`, rebake stored terms' HARMONIC (same step as `set_dictionary_state`;
  new per-term dict entries appear on the next refresh_terms, exactly as egui), `knowledge_dirty =
  true`, emit `dictionaries-changed`. Registered in `lib.rs`. **Frontend:** `ipc.ts`
  `loadFrequencyDictionaries(onProgress) → Promise<number>` (Channel wrapper); stores action
  `loadFrequencyDictionaries()` — tools-ready-guarded, streams progress into the `overlay`
  (overlay only appears once the reload starts, so no flash behind the picker; cancelled dialog =
  silent no-op), failures → `lastError` "Reload Error" banner (egui's Reload Error modal). The
  existing `onDictionariesChanged` hydrate wiring already re-fetches terms + setup status, so the
  table/checklist/banner update without new plumbing. Surfaces enabled: `TopBar.svelte` File →
  "Load New Frequency Dictionaries" (gated on toolsReady, stub comment removed);
  `SetupChecklistModal.svelte` items 2 & 6 "+ Install Dictionary" now call the action (the
  now-vestigial `actionDisabled` field + its disabled/title template attrs removed — every action
  is live). **Checks (orchestrator-run):** `cargo check -p yomine-tauri` ✓ (24.3s clean); engine
  crate untouched (was ✓ this session); `pnpm run check` → exactly the 4 known vite.config.ts
  errors + 8 pre-existing backdrop a11y warnings, **zero new**. **Verify on Windows (`cargo tauri
  dev`):** File menu entry enabled once tools load; click → multi-select `.zip` picker; cancel →
  nothing happens (no overlay/error); pick a new Yomitan freq zip → overlay shows
  "Reloading/Loading dictionary …" then clears; Frequency Weighting modal lists the new dict; with
  a file loaded, table frequencies/bounds update; checklist item 6 flips ✓ (>1 dicts) and both
  "+ Install Dictionary" buttons open the same picker (modal closes after, egui parity); re-pick
  the SAME zip → silent no-op (skip + no reload); knowledge widget recomputes (bands include the
  new dict). Compare against egui's File → Load New Frequency Dictionaries.
  **Re-bake refinement (2026-07-06, maintainer decision) — deliberate egui deviation.** Maintainer
  verify found the imported dict showing "No frequency data": per-term frequencies are baked only
  at file-process time (`extract_words` → `build_freq_map`), and `refresh_terms` re-*filters*
  without re-running lookups — so in egui a new dict never reaches a loaded file's terms until the
  file is reopened (with only the new dict enabled, every HARMONIC = u32::MAX → bounds collapse →
  TableControls "No frequency data"). Diagnosed against the real dict (`[Freq] Narou Freq`, probe
  vs the extracted term bank: 49269/49269 entries parse, all common-lemma lookups hit — import +
  engine were fine; the data was just stale). Fix: after the manager swap, the command now re-bakes
  `terms` + `base_terms` with `term.frequencies = manager.build_freq_map(lemma, reading, is_kana)`
  (the exact call `extract_words` uses; bakes ALL dicts + HARMONIC, drops removed dicts) instead of
  the HARMONIC-only rebake — imports now take effect immediately, no reopen needed.
  `set_dictionary_state` intentionally keeps its HARMONIC-only rebake: per-dict entries are always
  all baked regardless of enabled state, so enable/disable/weight changes only move HARMONIC. Note
  the re-bake keeps the originally-chosen lemma (deinflection candidate ranking consults the
  manager at process time); full re-ranking still needs a reopen — acceptable, edge-case only.
  Verify addition: with a file loaded, import a new dict → per-term frequency + breakdown show the
  new dict immediately; disable all other dicts → table still shows frequencies (was "No frequency
  data").
- **T061+T062+T063 DONE (2026-07-06) [parity] — T039 regression fixes + confirmed seeks.** Batch
  from the T039/T054 maintainer sign-off (T039 passed *functionally*; two UI regressions spun into
  T061/T062, and the T035 "👁 stays unmirrored" decision was reversed as T063).
  **T061 sort → column headers (`TermTable.svelte`, egui `table/header.rs`):** the SORT dropdown +
  ▲/▼ button are gone from `TableControls`; the Sentence and Frequency **headers** are the sort
  surface. Click inactive → activate in `defaultDir` (Sentence starts Chronological asc); click
  active → reverse. Active header: cyan-10% background (egui `draw_column_highlight`) + cyan ⬆/⬇;
  inactive hover reveals a weak default-direction preview arrow (egui `sort_arrow_text`); `title`
  tooltips mirror egui's hover text ("Sorted by X in ascending order"). When a Sentence mode is
  active a small weak-text chip (🕒 Chronological / # Sentence Count / 📊 Estimated Comprehension)
  cycles the three modes keeping direction (egui's cycle icon). POS button stays in TableControls
  (T046 single-surface deviation); `SORT_FIELDS` removed from `table.ts` (orphaned).
  **T062 dual-thumb log slider (`DualSlider.svelte` new):** replaces the two split linear range
  inputs. One track, two knobs, **logarithmic** value mapping (egui `DoubleSlider::logarithmic`;
  safe: `freqBounds` floors lo at 1); pointer-driven with `setPointerCapture` (press anywhere on
  the track grabs the nearest thumb), thumbs are `role="slider"` + arrow-key nudge (1%/press);
  cyan fill between knobs. `TableControls`: slider + `type=number` Min/Max fields (egui's
  DragValues; commit-on-change, clamped to bounds and each other, live during slider drags) + the
  "?" unknown toggle. Layout: `FREQ [slider] [min] – [max] [?]`.
  **T063 confirmed seeks (backend+frontend):** `PlayerStatus` gains `confirmed_timestamps:
  Vec<f32>` (engine `PlayerManager::get_confirmed_timestamps`, ws+mpv merged; Vec order is stable
  insertion so the player task's `PartialEq` change-detection emits exactly on new confirmations,
  within one 250ms tick). `ipc.ts`/store placeholder updated; `SentenceView` timestamp button:
  `confirmed = confirmed_timestamps.includes(start_secs)` → `👁 label` with egui's green `#559449`
  fill (else ▶ as before); contracts/events.md row updated. **Checks (orchestrator-run):**
  `cargo check -p yomine-tauri` ✓ (23.4s); `pnpm run check` → baseline only (the new DualSlider
  track needed `role="group"` to stay warning-clean). **Verify on Windows (`cargo tauri dev`):**
  headers sort/reverse/highlight/cycle like egui (compare side-by-side); dropdown gone; freq
  slider = one track two knobs, log feel (low end roomy), min/max fields commit + clamp, "?"
  works; with asbplayer/mpv connected, click a timestamp → after the player acknowledges, the
  button turns green 👁 (egui parity), other rows unaffected.
  **Polish (2026-07-06, maintainer verify feedback):** (1) `DualSlider` geometry — thumbs inset by
  half their width (`PAD = 7px`; rail/fill/thumb positions + pointer-fraction math all use the
  inset) so the knobs no longer overhang the track ends, plus `margin: 0 0.35rem` breathing room;
  (2) sortable-header affordance — inactive Sentence/Frequency headers now show a persistent dim
  **⇅** (swaps to the default-direction preview arrow on hover) and `.head-btn:hover` gets a
  background highlight, so click-to-sort is discoverable without hovering. Verify: ⇅ visible on
  both headers at rest; knobs stay inside the track at 0%/100%.
- **T064 DONE (2026-07-06) [feature] — In-App Dictionary Manager (issue #100); first post-parity
  feature.** Maintainer design calls (question tool): update-check via **remote manifest** in the
  repo, **deletion with confirm** included, seed = JPDB + Jiten. Key discovery: Jiten's index.json
  carries Yomitan's update protocol (`isUpdatable`/`indexUrl`/`downloadUrl` → api.jiten.moe), so
  its latest revision is checked **live** — no maintainer URL needed (his installed `Jiten
  25-11-28` vs remote `26-07-02` = a real update-available test case). **Manifest:**
  `assets/recommended_dictionaries.json` (JPDB v2.2 Kana → Kuuuube zip + static
  `latest_revision`; Jiten → jiten.moe download + `index_url` live check), fetched from
  `raw.githubusercontent.com/mcgrizzz/Yomine/main/...` with the same file `include_str!`-baked as
  offline/pre-publish fallback (**publish note: badges use the baked copy until this lands on
  main**). **Engine (additive):** `http::fetch_text(url)`;
  `FrequencyManager::dictionary_revisions()` (title→revision; `dictionaries` is private).
  **Backend:** `src-tauri/src/recommended.rs` (manifest types/parse/consts); AppState gains
  `recommended_catalog` (install resolves URLs from the cached catalog — frontend only ever sends
  a title); T060's reload tail factored into `dictionary::reload_and_swap` (spawn_blocking
  process → weights → swap → per-term re-bake → knowledge_dirty → `dictionaries-changed`), reused
  by all paths; new `commands/recommended.rs`: `get_recommended_dictionaries` (manifest + live
  index fetches off-runtime; status = not-installed/installed/up-to-date/update-available),
  `install_recommended_dictionary(title, progress)` (download to `.zip.part` FIRST so a failed
  update never kills the working copy → `remove_dictionary_files(title)` (deletes every folder
  whose index.json title matches + its same-stem zip — an update's new zip name would otherwise
  extract beside the old folder and double-load the title) → rename into place → reload_and_swap),
  `remove_dictionary(title, progress)` (files + `settings.frequency_weights` entry + persist +
  reload; works for ANY installed dict; removing the last one re-triggers the engine's default
  auto-download on reload — engine behavior, noted). All three registered in lib.rs; contracts
  updated. **Frontend:** `FrequencyWeightsModal.svelte` → title/menu renamed **"Frequency
  Dictionaries"** (TopBar Settings entry too, per issue); new top "Recommended" section (name,
  revision — `old → new` when update-available — description, status badge, Download/Update
  button; "Checking for updates…" while loading; inline op-error line; busy state serializes all
  mutating ops); weights rows gain a 🗑 with **two-step confirm** (🗑 → Confirm/✕). Install/remove
  are immediate (not staged), stream progress into an inline hint line, then re-hydrate the
  weights list + catalog (staged edits reset — the dict set changed) and mirror the dropped weight
  out of the local `settings` store. `ipc.ts`: `RecommendedDictionary` type + 3 wrappers.
  **Checks (orchestrator-run):** `cargo check -p yomine-tauri` ✓ (6.6s), `-p yomine` ✓ (21.8s, 3
  known warnings); `pnpm run check` → baseline only. **Verify on Windows (`cargo tauri dev`):**
  Settings → Frequency Dictionaries: Recommended shows JPDB "(…2024-10-13)" **Up to date** (no
  button) and Jiten "25-11-28 → 26-07-02" **Update available**; Update Jiten → progress hint →
  list re-hydrates, badge flips Up to date, table frequencies still live (re-bake); 🗑 a spare
  dict → Confirm → it leaves the weights list + `dictionaries-changed` refreshes terms/checklist;
  🗑 then ✕ cancels; kill network → reopen modal: badges degrade gracefully (baked manifest, no
  live Jiten check), Download/Update surface an inline error.
  **Refinements (2026-07-06, maintainer verify feedback):** (1) **File → Load New Frequency
  Dictionaries removed** — the zip import lives in the Frequency Dictionaries modal footer
  ("Import from file…", streams progress into the modal's hint line, re-hydrates + re-checks on
  success); the setup-checklist buttons still use the store action (which now also refreshes the
  catalog). Deliberate egui divergence, noted in TopBar. (2) **Update checks off the modal-open
  path**: new `recommendedDicts` store (null until first successful check), checked ONCE at launch
  (end of `hydrate()`, tools-ready-guarded, failure = console only) + a manual "⟳ Check for
  updates" button in the Recommended header (inline error); install/import/remove re-check
  afterwards (statuses changed anyway). Modal open is now network-free. (3) **Knowledge widget
  latency fix**: `background.rs` split into two independent loops — the Anki probe keeps its 5s
  cadence, the knowledge recompute gets its own 1s loop (idle check = lock + atomic read), so the
  first summary lands ~1s after tools load instead of queueing behind the AnkiConnect
  attempt/timeout; refresh-marked dirty flags also land within ~1s. Checks: cargo + svelte both
  baseline. Verify: launch with Anki closed → coverage box appears right after tools load; open
  Frequency Dictionaries → no "checking" flash, badges from the launch check, ⟳ re-checks;
  File menu has no import entry; footer Import from file… works end-to-end.
- **T065 DONE (2026-07-06) [objective #3] — segmentation regression suite + the numeric-furigana
  fix (and two more engine bugs it surfaced).** Designed first-principles per maintainer directive
  (NOT the orphaned `src/tests/segmentation_regression.rs`); design calls via question tool: live
  UniDic tokenizer (truth over speed; auto-download / UNIDIC_PATH; loud skip when absent,
  `YOMINE_REQUIRE_UNIDIC=1` hard-fails for CI), TOML fixtures, scope = segmentation+readings+POS +
  deinflection + phrase promotion. **Suite:** `tests/segmentation.rs` — fixture files
  `tests/fixtures/segmentation/{core,numbers,deinflection,phrases}.toml` (10 cases), schema =
  `[[case]]` with `text` + optional ordered `segments` (surface/reading/pos, partial asserts) +
  `terms` (matched by surface; lemma/reading/surface_reading/pos asserted only when present) +
  `absent`; per-file `frequencies` table builds a **synthetic in-memory dictionary**
  (`FrequencyManager::from_dictionaries`, new pub engine ctor) so deinflection ranking + phrase
  promotion are deterministic. One shared tokenizer (OnceLock); ALL failing cases reported in one
  run with expected-vs-actual lines + an actual-segments/terms dump. Companion tool:
  `examples/segment.rs` (`cargo run --example segment -- "<sentence>"`) dumps raw features →
  parsed tokens → words → terms → display segments. New dev-dep `toml`; `pub use vibrato`
  re-export in lib.rs. **Bug 1 — the T032 numeric furigana bug (8月22日 → がつにち), two real
  causes found via the dumper:** (a) UniDic emits multi-digit numbers per-digit (22 = 2ニ+2ニ →
  にに) and single multi-digit OOV tokens with surface-fallback "readings" (3000 → "3000"); new
  `segmentation/numbers.rs` (`merge_digit_runs` at the head of `process_tokens`) folds digit runs
  into one token with a **place-value synthesized reading** (`number_to_katakana`: 22 ニジュウニ,
  300 サンビャク, 8000 ハッセン, 12500 イチマンニセンゴヒャク; 兆-range cap; leading-zero strings
  read per-digit; unit-tested in-module). (b) display segments carried the **main word's** reading
  over the **full segment's** span (8月 span with がつ; also 勉強します-type words showed べんきょう
  over everything) → `extract_words` now emits `full_segment_reading` (one line + comment). Result:
  8月22日 renders はちがつ/にじゅうににち over the right spans; the frontend needed NO changes (its
  whole-ruby fallback is correct once the reading is complete). **Bug 2 — rendaku blocked ALL
  compound promotion:** the phrase loop's pair lookup deliberately rejects a form whose entries
  carry a different reading, and dictionary compounds carry rendaku (かいしゃ→がいしゃ) that the
  naive component concat lacks → no rendaku compound could ever be promoted. Fix:
  `phrase_reading_candidates` — plain concat first, then one-boundary-at-a-time dakuten/handakuten
  variants of each interior component's first kana; a variant hit also **adopts the corrected
  reading** on the phrase term (fixes its furigana too). **Documented finding (not changed):**
  `min_len = 4` in the phrase loop means ≤3-char compounds (土曜日, 誕生日) are NEVER promoted even
  with a dict entry — pinned by an `absent` fixture; loosening it is a maintainer tuning call.
  **Engine-shared:** all fixes benefit egui too. **Checks:** `cargo test -p yomine` all green
  (7 lib + suite); suite runtime ~7s warm; `cargo check -p yomine-tauri` ✓; frontend untouched.
  **Verify on Windows (optional — engine-level covered by tests):** open a file containing
  8月22日-style dates → furigana shows はちがつ/にじゅうににち aligned; 勉強します-style rows show
  the full reading; a known 4+ char compound (株式会社) still promotes.
- **T053 code-complete (2026-07-07) — CI workflows; NOT ticked until the first green run.**
  **test.yml:** paths filter now covers the workspace (`src-tauri/src`, tauri config/capabilities,
  root `tests/`/`examples/`/`assets/`) + a new `frontend` filter (`src-tauri/ui/**`); `rust-tests`
  gains the Tauri linux deps (webkit2gtk-4.1, ayatana-appindicator, rsvg, xdo), a **UniDic model
  cache** (`~/.local/share/yomine/dictionaries/tokenizer`, key `unidic-tokenizer-v1`) with
  `YOMINE_REQUIRE_UNIDIC=1` so the segmentation suite (T065) runs for real and a silent skip fails
  CI, and now runs the matrix: `cargo test -p yomine` (gui on) → `cargo check -p yomine
  --no-default-features` (the Tauri configuration of the engine) → `cargo check -p yomine-tauri`
  (with `mkdir -p src-tauri/ui/build` — `generate_context!` needs frontendDist to exist). New
  **svelte-check job** (pnpm 10 / node 22, lockfile-cached, `pnpm install --frozen-lockfile` +
  `pnpm run check`), gated on the frontend filter. **release.yml:** new `build-tauri` job
  (windows-latest, ubuntu-22.04, macos universal via `--target universal-apple-darwin`) using
  `tauri-apps/tauri-action@v0` — uploads installers to the tag's release, or builds + attaches
  artifacts in `build_only` mode; the egui binary jobs stay untouched until T055/O4.
  `manual-release.yml` needs no change (it `workflow_call`s release.yml). **@types/node fix:**
  `^22.10.0` added to `src-tauri/ui/package.json` devDependencies — clears the 4 baseline
  vite.config.ts errors + the tsconfig warning (tsconfig already declares `types: ["node"]`).
  **MAINTAINER STEPS (Windows):** (1) `pnpm install` in `src-tauri/ui` to regenerate
  `pnpm-lock.yaml` (CI uses --frozen-lockfile and WILL fail without it; note this wipes the
  hand-patched WSL linux binaries in node_modules — re-drop them or run checks from Windows);
  confirm `pnpm run check` is now **0 errors**; (2) push / open the PR and watch the Tests
  workflow (first run downloads UniDic once, then cached); (3) when releasing, `manual-release`
  or a release tag now also produces Tauri installers — smoke-test them (that's T052) — then tick
  T053.
- **T066 DONE (2026-07-07) [feature] — load subtitles from asbplayer (issue #105 phase 1;
  maintainer-directed).** Uses the `get-bound-media` (asbplayer PR #1033) + `get-subtitles`
  commands the maintainer landed upstream (extension v1.20+; shapes from asbplayer's
  docs/reference/external-api.md). **Engine (`src/websocket/`):** the protocol gains real
  request/response — `CommandResponse` now carries an optional `body`; `ServerCommand::
  ProcessConfirmation` generalized to `ProcessResponse { message_id, body }` (a pending request
  wins the messageId, else it's a seek confirmation exactly as before); `WebSocketServer` gains a
  `pending_requests` map (messageId → `SyncSender`), `request_blocking(command, body, timeout)`
  (no clients → "asbplayer is not connected"; timeout error hints "needs the asbplayer extension
  v1.20+"), and typed `get_bound_media()` (5s) / `get_subtitles(media_id, track_numbers)` (15s).
  New types `BoundMedia`/`SubtitleTrack`/`RemoteSubtitle` (camelCase serde matching asbplayer);
  `RemoteSubtitle::to_sentence` converts a cue (ms timings) into a pipeline `Sentence` — same
  cleanup as the SRT parser via the newly-shared `parser::clean_subtitle_text` (parse_srt now uses
  it too; adds a trailing trim, behavior otherwise identical) and a `TimeStamp` from ms so
  **seek + 👁 confirmations work unchanged**. `pipeline::process_sentences` factored out of
  `process_source_file` (the tokenize→dedupe→filter→comprehension tail) so non-file sources run
  the identical pipeline. **Tauri:** `PlayerCommand::{GetBoundMedia, GetSubtitles}` — handled in
  the player task by cloning the ws server Arc into `spawn_blocking` (the 250ms poll loop never
  stalls on the request timeout); `PlayerHandle` async wrappers; snake_case `BoundMediaDto`;
  commands `get_asbplayer_media` (player.rs) and `load_asbplayer_media(media_id, track_numbers?,
  title, progress)` (file.rs — mirrors `process_file`'s tail: FileData store, background
  live-Anki refresh, but **skips recent-files recording**; `SourceFile { source: "asbplayer",
  file_type: Other("asbplayer"), original_file: "asbplayer://<id>" }`; empty subtitle list →
  friendly Err). Registered in lib.rs; contracts updated. **Frontend:** `AsbplayerModal.svelte`
  media picker — fetches on open + ⟳ Refresh; rows show favicon (🎬 fallback), title
  ("Untitled video" fallback), streaming/local badge, green "active tab" badge; media without
  loaded subtitles are visible-but-disabled with a "load a subtitle file in asbplayer first"
  hint; ONE track = one-click Load, multiple tracks = radio choice (+ "All tracks"); success
  closes the modal into the loaded table. Entry points: landing screen gains "▶ Load from
  asbplayer" beside Open New File **only while asbplayer is connected** (`ws_clients > 0`, live
  via player-status), and File → "Load from asbplayer…" (disabled + tooltip when not connected).
  `stores`: `asbplayerModalOpen`/`openAsbplayerModal`/`loadFromAsbplayer` (overlay progress,
  `lastError` on failure). **Checks:** `cargo check -p yomine` + `-p yomine-tauri` ✓, engine
  tests all green, `pnpm run check` → **0 errors** (first run with the T053 @types/node fix +
  maintainer's lockfile regen) + 8 known backdrop warnings. **Verify on Windows (`cargo tauri
  dev`):** with asbplayer connected + a video bound: landing shows the asbplayer button → picker
  lists the video (title/favicon/badges) → Load → table appears with timestamps; click a
  timestamp → seeks + goes green 👁; with multiple subtitle tracks → radio choice; video with no
  subtitles → disabled row + hint; disconnect asbplayer → landing button disappears, File-menu
  entry disables; old extension → timeout error mentions v1.20+. Phase 2 of #105 (Yomitan
  `get-rendered-fields` → AnkiConnect `create note` → `mine-subtitle` one-click flow) is a
  separate task.
  **Follow mode + picker polish (2026-07-07, maintainer verify feedback + design Q&A).** Picker:
  dialog widened to 720px; titles wrap up to 3 lines (`line-clamp`, badges/Load pinned right) —
  no more mid-title ellipsis. **Follow mode (maintainer choices: "follow after first load",
  persisted):** new `SettingsData.asbplayer_follow` (serde-default false; shared settings.json,
  egui-compatible); the picker gains a persisted "Keep following asbplayer — load new videos
  automatically" checkbox. Backend: third `background.rs` loop (3s cadence) that is **armed** only
  while the setting is on AND the current file came from asbplayer (`source == "asbplayer"`) —
  arming seeds a seen-set from the current `get-bound-media` list so ONLY genuinely-new media ids
  trigger (tab-switching among open videos never does; a new id fires once its subtitles are
  loaded, which can land a couple polls after the video appears; active tab preferred when
  several are new). Auto-load reuses the factored `load_asbplayer_into_state` (the command's body,
  progress optional) and pushes the new `FileLoadResult` via the new **`asbplayer-media-loaded`**
  event; failures → `error` event. Opening a regular file (or unchecking) disarms; the next
  asbplayer load re-arms with a fresh seed. Frontend: event listener swaps the table in place +
  a new non-blocking `notice` toast ("Loaded from asbplayer: <title>", auto-dismiss 4s, green
  accent, top-center) — first use of the toast primitive. Checks: both crates ✓, svelte-check
  0 errors / 8 known warnings. Verify: enable the checkbox, load a video, play the next episode
  (or open a new subtitled video) → within ~3-6s the table swaps + toast; switching among
  already-open tabs does nothing; open a local file → following stops until the next asbplayer
  load.
  **Round 2 (2026-07-07, maintainer verify feedback) — exposure, active-tab follow, poll rate,
  recents integration.** (1) **Settings split + exposure**: `asbplayer_follow` →
  `asbplayer_follow_new_media` + new `asbplayer_follow_active_tab` + `asbplayer_poll_secs`
  (default 3, clamp ≥1; all serde-default). The **TopBar asbplayer status indicator is now a
  click-menu** ("Load from asbplayer…", ✓ Follow new videos, ✓ Follow active tab — right-anchored
  panel; stopPropagation so toggling keeps it open, Esc closes) — the follow toggles are one click
  away while connected; the picker keeps both checkboxes too (shared persisted state via a new
  `patchSettings` store helper). Poll rate lives in the **WebSocket settings modal** (staged
  field, 1-60s validation, Restore Default resets both). (2) **Active-tab follow**: the background
  loop (now `sleep(asbplayer_poll_secs)` re-read each iteration) checks new-media first, then — if
  `follow_active_tab` — whether the loaded video is still among the active-with-subtitles tabs;
  if not, switches to the first one. Needs to know what's loaded: `FileData.asbplayer_media_id:
  Option<String>` (set by asbplayer loads, `None` for files) — also replaces the source-string
  check for arming. Stable by construction: no flapping when two windows both have active videos
  (loaded ∈ actives → no-op). (3) **Recents integration**: asbplayer loads now write the raw cues
  as a real `.srt` (engine `websocket::subtitles_to_srt`) to `<app data>/asbplayer_subtitles/
  <sanitized title>.srt` (overwrite = reloading the same video updates its entry) and
  `record_recent_file` it — sessions reopen later WITHOUT asbplayer via the normal parser (raw
  text written; both paths clean identically). `SourceFile.file_type` = SRT + real path when
  saved (fallback `asbplayer://<id>` + no recents on write failure, best-effort). Checks: both
  crates ✓, svelte-check 0 errors / 8 known warnings. Verify additions: asbplayer dot → menu opens
  with working toggles (state matches the picker's); tab-switch with "Follow active tab" on →
  table swaps to that tab's video (~poll interval); poll rate change in WebSocket settings takes
  effect next tick; after an asbplayer load, the landing recents list shows the video title and
  reopening it works with asbplayer closed.
- **T067 DONE (2026-07-07) [bugfix] — DualSlider fast-drag 🚫 + startup loading surface
  (maintainer verify feedback).** (1) **DualSlider (T062)**: pulling a knob fast showed a red
  not-allowed cursor and the knob stopped — the `pointerdown` never `preventDefault()`ed, so the
  WebView started a native text-selection drag, which fires `pointercancel` and kills our drag.
  Fix: `e.preventDefault()` in `down()`, `user-select: none` on the track, and
  `onlostpointercapture={up}` so any way capture ends clears the drag state. (2) **Startup
  loading**: the main region rendered an inline "Loading language tools…" `<p>` instead of the
  overlay popup used everywhere else (the overlay WAS already streaming tools-load progress; the
  inline branch just sat behind/instead of it). Fix: dropped the `!toolsReady` branch in
  `+page.svelte` — the landing screen now renders behind the `$overlay` popup (which blocks
  pointer events while up); landing "Open New File" / "Load from asbplayer" get
  `disabled={!toolsReady}` for the brief gap between overlay clear and the `ready` event; orphaned
  `.muted` style removed. Checks: both crates ✓, svelte-check 0 errors. **Verify on Windows:**
  yank a frequency knob violently (no 🚫, knob tracks, drag survives leaving the track); fresh
  launch shows the landing screen under the dimmed progress popup, no inline loading text.
- **T068 DONE (2026-07-07) [feature] — Appearance modal: whole-UI font scale (maintainer request,
  placement Q&A → Settings → Appearance).** New `SettingsData.font_scale: f32` (serde-default 1.0;
  shared settings.json, egui ignores it) applied by the root layout as **CSS `zoom` on
  `documentElement`** (scales px sizes too, unlike a rem-only root-font-size change; no Tauri
  capability needed). `AppearanceModal.svelte` (WebSocket-modal pattern: staged edit,
  hydrate-on-open `$effect`+`untrack`): range slider 75–150% step 5 with −/+ steppers and %
  readout, **live preview while adjusting** (modal sets the same zoom property directly), Save
  persists via new `setFontScale` (clamped 0.75–1.5, `patchSettings`), Cancel reverts the staged
  value, ✕/backdrop revert the preview to the saved value, Restore Default → 100%. Settings menu
  gains "Appearance" (above Setup Checklist); modal mounted in `+page.svelte`. `ipc.SettingsData`
  + stores (`appearanceModalOpen`/`openAppearanceModal`/`setFontScale`). Checks: both crates ✓,
  svelte-check 0 errors / 9 warnings (8 known + the new modal's identical backdrop pattern).
  **Verify on Windows:** Settings → Appearance — slider live-previews the whole UI (menus, table,
  modals), Cancel/close reverts, Save persists across restart, default 100%.
- **T069 round 1 (2026-07-07) [UI/UX pass] — full component audit + six fixes.** Audited every
  component + app.css (modal chrome/staging patterns were already consistent). Fixed: (1) **Esc
  now closes every modal from anywhere** — the backdrop `onkeydown` only fired once focus was
  inside the modal (right after opening, focus is on `body`, so Esc did nothing); each of the 9
  modals gains a top-level `<svelte:window>` Esc listener gated on its open store (Appearance/
  asbplayer/Setup/Analyzer route through their `close()` so preview-revert etc. still runs), and
  the TopBar menus close on Esc too. (2) **Global themed inputs** (app.css): text-like
  `input`/`select`/`textarea` get the fg/bg-light/border/3px treatment — the table-controls
  search + Min/Max fields were rendering native WebView chrome; checkbox/radio/range excluded,
  component styles still override. (3) **Term-table empty state**: filters excluding every row
  left a bare header — now "No terms match the current filters." (4) **Toast + error banner
  above modals**: both were under the z-50 backdrops (a WebSocket save failure showed its error
  banner BEHIND the modal); now z-60. (5) **asbplayer picker per-row busy**: `busy` was global so
  every row's button read "Loading…"; now only the clicked row does (all stay disabled). (6)
  **Global `:focus-visible` outline** (cyan, matches the DualSlider thumbs) for keyboard nav.
  Flagged, not changed (maintainer's call): the counts line under the title ("N shown / known /
  total") and the controls-row "N / M shown" say nearly the same thing twice. Checks:
  svelte-check 0 errors / 9 known warnings. **Verify on Windows:** Esc closes each modal
  immediately after opening it from a menu; search + Min/Max inputs look themed (dark, bordered)
  in both themes; filter everything out → empty-state message; trigger a WS-port save failure
  with the modal open → banner visible on top; picker with 2+ videos → only the clicked row says
  Loading…; Tab around → cyan focus outlines.
  **Round 2 (2026-07-07, maintainer verify feedback) — scroll scoping + menu reorganization.**
  (1) **Outer window scrollbar** (whole app scrolled, menu bar included): T068 regression —
  `.app-shell` was `height: 100vh`, and vh ignores the root CSS `zoom` the Appearance scale
  applies, so any scale >100% overflowed the window. Now `height: 100%` (chains through
  html/body) + `overflow: hidden` on html/body so the window can never scroll. (2) **Inner
  scroll scoped to the term rows only**: `.app-main` no longer scrolls (flex column,
  overflow hidden); the file view wraps `TermTable` in a `.table-scroll` region — title, Anki
  coverage card, and search/filter controls stay put; the sticky column header sticks to the
  rows region's top (also kills the old rows-visible-above-the-sticky-header padding gap). The
  landing screen already scoped its own recents scroll. TermTable's context menu now closes on
  `onscrollcapture` (plain `onscroll` on window never fires for inner-container scrolls). (3)
  **Menu reorganization** (maintainer choice via Q&A: "Mining menu"): the one-item Tools menu is
  gone; new **Mining** menu = Ignore List, Part of Speech Filters, Frequency Dictionaries, ─,
  Frequency Analyzer (the data you tweak while working); **Settings** = Anki, WebSocket Server,
  Appearance, ─, Setup Checklist (true config + onboarding); **File** gains a separator before
  Open Data Folder/Quit. `.menu-sep` divider style; setup-checklist description string updated
  to "Mining → Frequency Dictionaries". Checks: svelte-check 0 errors / 9 known warnings.
  **Verify on Windows:** at 125% Appearance scale there is NO outer scrollbar; with a long file
  loaded, scrolling keeps title/coverage/search fixed and only the rows move (column header
  pinned); menus read File / Mining / Settings with separators; all six relocated entries open
  their modals.
- **T070 DONE (2026-07-07) [cleanup] — code structure + comments pass on the tauri code
  (maintainer Q&A: strip task IDs; keep only behavioral egui refs; split stores only).**
  (1) **stores/index.ts (704 lines) split by concern** into ui / status / modals (all 9
  open-flags + open fns in one place) / file / controls (table search/sort/POS/freq +
  visibleTerms) / settings (mirror + every settings-persisting action via a shared
  patchSettings that now reports unsaved-because-not-hydrated) / ignore / dictionaries /
  setup / player (status + seek + loadFromAsbplayer) / hydrate — index.ts is pure re-exports,
  so every component import (`$lib/stores`) is unchanged. toggleDarkMode/toggleSerifFont/
  saveAnkiSettings now route through patchSettings (same behavior, less duplication).
  (2) **Comments**: all TNNN/USn task references, dates, and "maintainer decision" attributions
  stripped across src-tauri/src + ui (regex pass + hand-fixes); provenance-only egui refs
  ("parity with src/gui/…", "Ports src/gui/…") dropped from every component/module header —
  behavioral egui refs kept ("Cancel reverts but keeps the modal open (egui behavior)").
  Stale references fixed along the way (+page header still said "scrolling main region";
  IgnoreListModal pointed at a nonexistent stores.addToIgnore). Contracts/spec files keep
  their task IDs (they ARE the process log). Checks: cargo check (tauri) clean, svelte-check
  0 errors / 9 known warnings. **Verify on Windows:** smoke-test the app (menus, modals,
  file load, table controls, asbplayer picker) — this round is behavior-neutral, so anything
  broken is a refactor slip.
  **Round 2 (2026-07-07, maintainer feedback: "comments should only be for non-obvious intent
  or tricky edge cases").** Full prune of paragraph-style explanatory comments across
  src-tauri (Rust + UI): component/module header paragraphs deleted or cut to the one
  constraint they carried; command doc paragraphs reduced to contract semantics (rejects,
  nulls, ordering, idempotency); DTO docs reduced to wire quirks; all remaining provenance
  egui refs ("Mirrors egui's X", "(egui `fn_name`)") stripped, keeping only
  behavior-explaining ones ("Cancel reverts but keeps the modal open (egui behavior)"). Kept
  intact: lock-discipline notes, untrack/effect-loop rationale, the hydrate event-race note,
  the `<wbr>`/ruby layout constraints, the DualSlider preventDefault fix note, follow-mode
  seeding semantics. Net ≈ −1000 comment lines. Checks: cargo check clean, svelte-check
  0 errors / 9 known warnings; regex-artifact scan clean. Same verify: behavior-neutral
  smoke test.
- **T071 pre-flight (2026-07-07) — every CI gate run locally, two fixes.** Ran the full
  test.yml gauntlet locally before the first real run: `cargo +nightly fmt -- --check` was
  FAILING (formatting drift since T065 — examples/segment.rs, numbers.rs, websocket, tests,
  gui files — plus reflow from the T070 comment edits); applied `cargo +nightly fmt`,
  format-only diff across 13 rs files, now clean. Fixed a test.yml typo: `ARGO_PROFILE_DEV_DEBUG`
  → `CARGO_PROFILE_DEV_DEBUG`. Then with CI's `RUSTFLAGS="-D warnings -A deprecated"`:
  `cargo test -p yomine` (all suites green incl. segmentation fixtures), `cargo check -p yomine
  --no-default-features`, `cargo check -p yomine-tauri` — all clean; `pnpm run build`
  (production adapter-static build) passes; svelte-check 0 errors. Versions aligned at 0.5.5
  across tauri.conf.json / ui package.json / both Cargo.tomls; identifier + frontendDist OK.
  Noted, not blocking: release.yml's `create-checksums` only covers the egui `yomine-*` binaries,
  not the tauri installers (different naming + job not in its `needs`) — fine while both ship,
  revisit at T072.
  **First run failed + fixed (2026-07-07):** "Check Tauri app" died on glib-sys — the
  `cache-apt-pkgs-action` restores listed packages but NOT their transitive dev deps
  (libglib2.0-dev, owner of glib-2.0.pc, never landed; the engine steps passed because they
  don't link GTK). Replaced it with plain `apt-get install` (what release.yml already uses),
  and split the Tauri check into its own `tauri-check` job (release.yml's dep set + the
  frontendDist placeholder) so the heavy GTK/WebKit compile runs IN PARALLEL with the engine
  tests — addresses the "slow" complaint too; later runs also get warm rust-cache + the UniDic
  cache. Dropped `--verbose` from cargo test (log noise). YAML validated locally.
  **Green (2026-07-07):** PR #107's run passed fully — fmt gate, engine tests (with live UniDic
  fixtures), no-default-features check, parallel tauri-check, svelte-check. A CodeQL scan flagged
  the missing `permissions` block on test.yml; added workflow-level least-privilege
  `{contents: read, pull-requests: read}` (the PR read is required by dorny/paths-filter on PR
  events; release.yml already had its explicit block). **T053 ticked.**
  **Remaining T071 (maintainer):** merge the PR, then a `workflow_dispatch` release build with
  `build_only: true` for installer smoke tests (T052) — Windows .msi/.exe is the one that matters
  most; Linux .deb/.AppImage and macOS .dmg come out of the same run.
  **SHIPPED (2026-07-07): v0.6.0 tagged and released** — the Tauri port is the shipping app.
  T052 smoke tests all passed (data continuity via the shared %LOCALAPPDATA%\yomine dir needed
  no migration by design); release notes drafted from the tasks log. **T071 DONE.**
- **T072 DONE (2026-07-07) — egui retired (T055 decision executed).** The egui code is preserved
  on the **`egui` branch** (cut from main at the v0.6.0 bump commit — push it BEFORE merging this
  change). On main: deleted `src/gui/`, `src/main.rs`, and `src/core/tasks/` (the egui task
  system, only ever consumed by gui code); removed the `gui` feature, the `[[bin]]` target, and
  the six egui-only deps (eframe, egui_extras, egui_flex, egui_double_slider, egui_ltreeview,
  rfd) plus `open` from Cargo.toml — the engine is now a plain lib crate with no features;
  frequency_utils lost its three rfd/TaskManager-gated fns; src-tauri drops the now-meaningless
  `default-features = false`. Workflows: test.yml's engine job needs only libssl (gtk/xcb were
  egui's), the "check without egui" step is gone (that IS the build now); release.yml loses the
  `build-and-upload` + `build-macos-universal` egui-binary jobs, and `create-checksums` now
  `needs: [build-tauri]` and checksums the actual installers (`sha256sum *` with updated
  descriptions — closes the gap flagged at T071 pre-flight). README: Tauri build instructions
  (pnpm + cargo tauri dev), egui → Tauri/Svelte in credits. Checks: fmt clean, all engine tests
  green, tauri check clean under CI RUSTFLAGS, release.yml YAML validated, zero
  `feature = "gui"` references left. **Verify:** push `egui` branch first; PR CI green; next
  tagged release produces installers + correct SHA256SUMS only.
- **NEXT options:**
  - **`load_frequency_dictionaries` import command (freq-dict import)** — the File-menu "Load New
    Frequency Dictionaries" entry and the checklist's two "+ Install Dictionary" actions (T045)
    are both disabled pending this backend command. Building it (native picker → parse Yomitan
    dict → register in `frequency_manager` → persist + `dictionaries-changed`) would enable all
    three surfaces. Separate task; out of scope for T045.
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
- [x] T056 [P] [US3] Surface `ServerState` in `PlayerStatus` (closes the T028 indicator
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
- [x] T039 [US4] **Verify**: each sort/filter/search/ignore action yields the same visible set as
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
- [x] T045 [US5] Setup checklist + banner: `get_setup_status`; actions (`open_url`, open Anki
      settings, load dicts, open websocket settings).
- [x] T046 [US5] **Verify**: every setting persists across restart and takes effect; checklist
      reflects true state. — VERIFIED in UI (maintainer, 2026-06-13) across the batch-by-batch
      verifies of T040–T045 (Anki mappings, websocket port, frequency weights, POS filters,
      theme/font, setup checklist all persist + take effect; checklist + banner reflect live
      state). Two known follow-ups remain open, neither blocking US5 sign-off: **T056**
      (ServerState dot sub-states) and the **`load_frequency_dictionaries`** import command (the
      checklist's two "Install Dictionary" buttons + the File-menu entry are stubbed pending it).

### US6 — Frequency analyzer (P3)

- [x] T047 [US6] Analyzer modal: file selection; `start_analysis` (Channel progress with ETA);
      `cancel_analysis`; results-preview table; export form → `export_analysis` (Yomitan/CSV +
      options); consume `analysis-complete`/`analysis-cancelled`/`export-complete`.
- [x] T057 [US6] **Analyzer parity gap (found in T047 review, blocks T048 full parity):** restore
      the two egui analysis options the first pass omitted —
      (a) **Balance corpus by source** checkbox (egui `modal.rs` ~452, hover "Uses trimmed mean
      (10% trimming) to calculate balanced sample sizes"): add a `balance_corpus: bool` param to
      `start_analysis`; when true the backend runs `tools::analysis::CorpusBalancer::new(paths)
      .balance()` on the path list before `analyze_files` (engine API already exists).
      (b) **Top 250 / Bottom 250** radio in the results table (egui `results_table.rs` ~28): the
      current `AnalysisPreview` only carries the top 250, so "Bottom 250" isn't representable —
      extend the preview to also include the bottom slice (or send both ends) and add the radio to
      the modal's results table. Parity-only; do before T048 sign-off. **Backend + frontend.**
- [x] T048 [US6] **Verify**: same ranking + equivalent exported artifacts as egui; cancel works.

### US7 — Knowledge summary (P3)

- [x] T049 [US7] Knowledge summary widget: `get_knowledge_summary` + `knowledge-summary` event;
      JLPT + frequency bands; coverage/estimate toggle (`KnowledgeMode`).
- [x] T050 [US7] **Verify**: JLPT + band values match egui for the same Anki snapshot.

**Checkpoint**: All user stories functional and verified against egui.

---

## Phase D: Package, sign off, CI

- [x] T051 Declare bundled resources in `tauri.conf.json` (fonts handled in-frontend; icon;
      `assets/jlpt_vocab.json` per O1 outcome); confirm runtime downloads (unidic, freq dicts,
      Anki cache) still resolve to `dirs::data_local_dir()/yomine`.
- [x] T052 `cargo tauri build` produces installers on Win/macOS/Linux; smoke-test each artifact.
      *(2026-07-07: Manual Release build_only run green on all 6 jobs; Windows installer
      smoke-tested — install, first launch, shared %LOCALAPPDATA%\yomine data continuity,
      core mining loop, WebSocket bind, Appearance persistence, clean uninstall.)*
- [x] T053 CI: replace/augment `.github/workflows/release*.yml` + `manual-release.yml` with the
      Tauri bundler; update `test.yml` to build the workspace (egui on/off matrix) + run
      `svelte-check`. *(First green run 2026-07-07 on PR #107 after the apt-deps fix,
      tauri-check job split, and least-privilege permissions block — see Progress.)*
- [x] T054 Final parity sign-off: walk the full quickstart.md checklist; tick spec.md
      Success Criteria SC-001..SC-009. (Maintainer sign-off 2026-07-06.)
- [ ] T055 Resolve Open Item O4: decide whether to retire the egui crate/feature or keep it
      gated for a transition; document in README/RELEASES.

---

## Parity gaps (final-inspection 2026-06-14) — PRIORITY GATE

Found in the post-US7 final egui-vs-Tauri inspection (egui `UiAction` catalog + all three menus +
table/term/sentence surfaces). **Maintainer directive: these close the egui→Tauri parity gap and
MUST be addressed before any new-feature / UX / UI work** (e.g. issues #18 theme picker, #68 sorts/
filters, #71 freq breakdown, #89 MPV launcher, #91 mouse-less mining, #100 dict manager). They need
not all land at once, but they jump the queue ahead of anything additive. T056 (asbplayer
sub-states, above) belongs to the same gate.

- [x] T058 [parity] **Open Data Folder** (File menu): egui's `File → Open Data Folder` opens the app
      data dir in the OS file explorer (`top_bar.rs::open_folder` → `get_app_data_dir()`); the Tauri
      menu item is a disabled stub. Wire it via the already-registered `tauri-plugin-opener` (reveal/
      open the `dirs::data_local_dir()/yomine` path the engine uses) and enable the `TopBar.svelte`
      item. Small; backend path helper (command or capability) + frontend onclick.
- [x] T059 [parity] [US4] **Term ignore UX → egui parity.** egui's term cell supports **Ctrl+Click**
      to toggle ignore AND keeps an ignored term **visible but greyed in place** with an inline
      "Remove from ignore list" undo (right-click or Ctrl+Click again); the actual re-filter happens
      on refresh (`table/mod.rs::ui_col_term`). Tauri only has the right-click "Add to ignore list",
      which calls `add_to_ignore_list` → the backend **re-filters immediately** so the row vanishes,
      with no inline undo (un-ignore only via the Ignore List modal). Restore: (a) Ctrl+Click toggle
      on the term cell (keyboard quick-action, obj #4), (b) grey-in-place instead of immediate
      removal, with inline un-ignore + the hover hint ("Ctrl+Click to ignore / UNDO ignore"). Decide
      where the re-filter fires (on refresh, per egui) vs Tauri's current eager `FileLoadResult`
      return — likely defer removal to the next `refresh_terms`. **Backend + frontend.**
- [x] T060 [parity] **`load_frequency_dictionaries` import command.** Both the `File → Load New
      Frequency Dictionaries` menu item AND the setup-checklist "+ Install Dictionary" actions (T045)
      are disabled stubs pending this backend command. Build it: native multi-file picker → parse the
      Yomitan dictionary → register in `frequency_manager` → persist + emit `dictionaries-changed`
      (the store already re-fetches terms + setup status on that event). Enables all three surfaces.
      egui reference: `frequency_utils::load_frequency_dictionaries`. **Backend + frontend (enable the
      two stubbed surfaces).** Largest of the three.
- [x] T061 [parity] [US4] **Sort UI → egui column headers.** T039 maintainer verify (2026-07-06)
      found the combined SORT dropdown a UI regression vs egui's clickable column headers. Restore:
      Sentence/Frequency headers are the sort controls (click to activate/reverse, active-column
      highlight, direction arrow, hover preview arrow, Sentence mode-cycle chip 🕒/#/📊). egui ref:
      `table/header.rs`. **Frontend only.**
- [x] T062 [parity] [US4] **Frequency filter → dual-thumb log slider.** Same T039 verify: the two
      split linear `<input type=range>`s are a regression vs egui's single two-knob logarithmic
      `DoubleSlider` + Min/Max DragValues (`table/controls.rs`). Build a `DualSlider` component
      (log scale, pointer-driven, keyboard-nudge) + numeric Min/Max fields. **Frontend only.**
- [x] T063 [parity] [US3] **Confirmed-timestamp seeks (👁).** Maintainer reversed the T035 "stays
      unmirrored" call (2026-07-06): surface `PlayerManager::get_confirmed_timestamps` in
      `PlayerStatus`; the timestamp button shows egui's 👁 + green (#559449) fill once the player
      acknowledges the seek (`sentence_column.rs::ui_timestamp_button`). **Backend + frontend.**

## Post-parity features

- [x] T068 [feature] **Appearance modal — whole-UI font scale.** `SettingsData.font_scale`
      (default 1.0) applied as CSS zoom by the root layout; Settings → Appearance modal with a
      75–150% slider (live preview, staged save, Restore Default).
- [x] T067 [bugfix] **DualSlider fast-drag 🚫 + startup loading via the overlay popup.**
      `preventDefault` + `user-select: none` stop the native drag that cancelled knob drags; the
      landing screen now renders behind the `$overlay` progress popup instead of an inline
      "Loading language tools…" paragraph.
- [x] T066 [feature] **Load subtitles from asbplayer (issue #105, phase 1).** `get-bound-media` /
      `get-subtitles` over the existing WebSocket (request/response with messageId correlation);
      landing-screen "Load from asbplayer" + File-menu entry open a media picker (title, favicon,
      streaming/local + active badges, track selection); the selected media's subtitles run
      through the normal pipeline with cue timings intact, so seek/👁 work end-to-end.
- [x] T065 [feature] **Segmentation regression suite + numeric/rendaku reading fixes (mission
      objective #3).** First-principles expected-vs-actual fixture suite (`tests/segmentation.rs` +
      `tests/fixtures/segmentation/*.toml`, live UniDic, per-file synthetic frequency dicts), the
      `cargo run --example segment` stage dumper, and the engine fixes it drove: multi-digit
      number readings (`segmentation::numbers`), full-segment display readings, and
      rendaku-tolerant phrase promotion.
- [x] T064 [feature] **In-App Dictionary Manager (issue #100).** Split the frequency-dictionary UI
      into a "Recommended" section (hosted, updateable dictionaries the user can check + download —
      e.g. JPDBv2㋕, Jiten — with version + Up-to-Date/download state) and the rest; likely rename
      "Frequency Weighting" → "Frequency Dictionaries". Builds on T060's import + reload plumbing.

## Ship roadmap (maintainer, 2026-07-07)

- [ ] T069 **UI/UX pass.** Sweep the whole app for polish: spacing/margins, affordances
      (clickable things look clickable), consistency across modals, empty/edge states.
- [x] T070 **Code structure + comments pass (tauri code).** Stores split by concern (index.ts →
      11 focused modules, re-exported); task-ID/date/attribution comments stripped, provenance
      egui refs dropped, behavioral ones kept.
- [x] T071 **Ship the port.** Push → first green CI run (ticks T053), release build + installer
      smoke tests (T052), tag/release.
- [x] T072 **Retire the egui build.** After the shipped port is validated: remove the egui build
      system/feature, move egui code to an `egui` branch, `main` becomes the Tauri port (T055
      decision executed).
- [ ] T073 **Resume the GitHub issue backlog** (#91 mouse-less mining, #3, #71, #68/#92,
      quick wins #94/#106/#82/#14, engine quality #6/#81/#102, #105 phase 2 one-click mining).
- [x] T074 **Update checker.** Launch-time GitHub releases API check (same pattern as the
      recommended-dictionaries manifest check); non-blocking "new version available" notice
      linking to the release. Full `tauri-plugin-updater` auto-update is a separate follow-up
      (needs an updater keypair + latest.json generation in the release workflow).
      *(2026-07-07: `check_for_update` command — GitHub `releases/latest` (excludes prereleases)
      via the engine's UA-bearing `fetch_text` on spawn_blocking, numeric x.y.z compare against
      `CARGO_PKG_VERSION` (unit-tested incl. beta-suffix and short-version cases). Frontend:
      one best-effort check at the end of hydrate — swallowed on failure — sets `updateInfo`,
      shows the toast once, and a green "⬆ vX.Y.Z available" pill sits left of the status
      indicators opening the release page. Verify on Windows: pill + toast appear once a newer
      release exists (or temporarily lower the version to test); clicking opens the release
      page; offline launch shows nothing.)*
- [ ] T075 **Code signing (Windows "unknown publisher").** *(Backburnered 2026-07-08,
      maintainer decision — revisit after feature work.)* Evaluate SignPath.io Foundation
      (free for qualifying OSS) first, Azure Trusted Signing (~$10/mo) as fallback; wire into
      `tauri.conf.json` `signCommand` + the release workflow. macOS Developer ID/notarization
      ($99/yr) is its own decision.
- [x] T076 **Auto-updater (`tauri-plugin-updater`).** *(Verified end-to-end 2026-07-08:
      key setup done, signed release shipped, installed build discovered the update and
      self-updated via download → install → relaunch.)*
      Backend: updater + process plugins registered; `bundle.createUpdaterArtifacts: true`;
      `plugins.updater` config with the GitHub `releases/latest/download/latest.json` endpoint,
      Windows `installMode: passive`, and a PLACEHOLDER pubkey; capabilities gain
      `updater:default` + `process:default`. Frontend: `stores/update.ts` — plugin `check()`
      first (signed latest.json), the T074 GitHub-API check as fallback (pill links to the
      release page when not installable); installable updates download+install with overlay
      progress then `relaunch()`. release.yml passes `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)`
      to both tauri-action steps — tauri-action then also generates + uploads latest.json.
      **Maintainer setup (blocking, in order):** (1) `cargo tauri signer generate -w
      %USERPROFILE%\.tauri\yomine.key` (pick a password, BACK THE KEY UP — losing it orphans
      all installed apps); (2) paste the printed PUBLIC key into `tauri.conf.json`
      `plugins.updater.pubkey`; (3) add repo secrets `TAURI_SIGNING_PRIVATE_KEY` (file
      contents) + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; (4) `pnpm install` in src-tauri/ui
      (two new JS plugins; clears the 3 expected svelte-check errors). NOTE: with
      `createUpdaterArtifacts` on, `cargo tauri build` fails without the signing env — set
      the two vars locally for manual builds; `cargo tauri dev` is unaffected. Verify: release
      vNEXT, then run vCURRENT → pill says "click to download and install" → progress →
      relaunches as vNEXT; a release without latest.json falls back to the link-only pill.
      **Single-source versioning (2026-07-08, maintainer request):** the version now lives in
      exactly one place — `[workspace.package] version` in the root Cargo.toml. Both crates use
      `version.workspace = true`; tauri.conf.json's `version` field was removed (Tauri v2 falls
      back to the crate version, which cargo resolves through the workspace — verified via
      `cargo metadata`: both crates report 0.6.0). `compute-version.js` now reads
      `[workspace.package]` first with the old `[package]` layout as fallback (tested against
      both + the no-version error case). ui/package.json's version is decoupled/cosmetic — it
      no longer needs bumping. Release procedure is now: edit the one line → Manual Release.
      **Release build/notes fixes (2026-07-08):** macOS/Linux release builds failed —
      esbuild 0.28 (vite 8) refuses to down-transpile destructuring below Safari 14.1; fixed
      with `esbuild.supported.destructuring: true` in vite.config.ts (keeps the safari13
      target, same output as the pre-vite-8 toolchain). `.github/release-notes-template.md`
      rewritten for Tauri artifacts (installers per platform, in-app-update callout, unsigned
      SmartScreen/Gatekeeper notes, latest.json/.sig explainer — egui binary instructions
      removed); SHA256SUMS-detailed header mentions updater files; release-helper's
      check-version-status grep now reads `[workspace.package]`. Full pipeline audit:
      README install section rewritten for Tauri installers (+ macOS xattr note, in-app
      updates); docs/RELEASES.md rewritten (no version input — tag computed from Cargo.toml;
      build-only dry run; required secrets incl. MY_PAT rationale); ensure-tests.js now
      gates on the whole Tests workflow-run conclusion instead of the first check run
      matching "tests" (which only covered rust-tests, ignoring tauri-/svelte-check).
- [ ] T077 **One-click mining (#105 P2) + mined-state tracking (#3).** *(Code-complete
      2026-07-08 — Verify on Windows.)* Card content comes from the user's own Yomitan
      config via yomitan-api (github.com/yomidevs/yomitan-api, default
      `http://127.0.0.1:19633`, `settings.yomitan_url`): `/ankiCardFormats` picks
      deck/model/field templates, `/ankiFields` renders the markers (`src/yomitan`, field
      assembly unit-tested incl. cloze-from-sentence + drop-empty-fields). Two paths from
      the ⛏ button in SentenceView (busy-guarded, one at a time): asbplayer sessions
      (`loadedFromAsbplayer` + connected + timestamped) confirmed-seek then WS
      `mine-subtitle` postMineAction 3 — asbplayer supplies sentence/audio/image, so
      empty-rendered sentence markers are dropped; everything else = direct AnkiConnect
      `storeMediaFile` + `addNote` (tag `yomine`, duplicate → "already in Anki" toast, row
      still marked). Mined state (issue #3, both layers): `added:1` terms via field
      mappings + sentence harvest from the existing full-note pass in `get_total_vocab`
      (normalized: tags stripped/whitespace removed — `anki::mined`, cached as
      `anki_mined_sentences.json`; frontend normalization must stay in sync); refresh on
      file load + ~2s after each mine + window focus (5s debounce), no polling.
      `FieldMapping.sentence_field` (optional) + Yomitan API URL/status live in the Anki
      settings modal. Windows verify: mine from a file session (note in Anki with
      glossary+audio), duplicate → toast, asbplayer session seek-then-mine (card with
      audio/image), Yomitan-direct mine + refocus → ⛏ mined, sentence badges on reload
      with a sentence-field mapping, yomitan-api stopped → clean error + Unreachable in
      settings, `pnpm run check`.
      **Refinements (maintainer feedback):** ⛏ moved next to the term (TermTable owns it;
      SentenceView's occurrence index is `$bindable` so the button mines the sentence on
      display) and only renders while yomitan-api is reachable (`yomitanReachable`,
      re-probed on every mined-state refresh + after Anki-settings save + hydrate);
      "✓ sentence mined" badge now also covers session-mined sentences
      (`sessionMinedSentences`) and shows for mined terms, answering "which sentence did
      this card use"; setup checklist gained the optional "Yomitan API Detected" item
      (`SetupStatus.yomitan_connected`, probe capped by a 10s client timeout in
      `yomitan::post`; banner unaffected — it only keys on required items).
      Round 2: fixed `props_invalid_value` crash on file load (Svelte forbids binding an
      undefined record entry onto a `$bindable(0)` fallback — fallback removed, reads
      coalesce; the thrown error had killed the table mount so the app fell back to the
      landing page). Mined look: green pill chip "⛏ mined" + the term itself tints green
      (kept above `.ignored` so grey still wins). Persisted sentence marks no longer
      require a sentence-field mapping for Yomine's own mines —
      `mined::record_mined_sentence` appends to the cache on every successful mine, and
      the `get_total_vocab` harvest MERGES with the cache instead of overwriting it
      (mapping still needed to flag cards created outside Yomine).
      Round 3: asbplayer path was silently skipped — `loadedFromAsbplayer` was a
      frontend writable set only by the manual load flow (follow-mode loads and webview
      reloads left it false → direct path). Now backend-derived:
      `FileLoadResult.loaded_from_asbplayer` = `FileData.asbplayer_media_id.is_some()`,
      frontend store is a `derived` of `fileResult`. UI: dropped the ⛏ glyph (renders as
      a thin scratch on Windows) — mine button is a fixed-footprint `+` chip that swaps
      to a green `✓` chip when mined (no layout shift), sentence mark is a matching
      icon-only `✓` chip in the meta row; term still tints green.
      Round 4 (maintainer corrections): (1) `loadedFromAsbplayer` plumbing REVERTED —
      the asbplayer path keys on the same rule as seeking (player mode `asbplayer` +
      client connected + row has a cue), not on how the file was loaded. (2) Mine order
      inverted to match asbplayer's one-click architecture: `postMineAction: 3` (export)
      failed because asbplayer had no card to build — now Yomine ALWAYS creates the note
      via AnkiConnect first (both paths identical: Yomitan fields + sentence + media),
      then the asbplayer path confirmed-seeks and sends `mine-subtitle` postMineAction 2
      (update last card) to attach audio/screenshot. Enrichment failure → `warning` on
      `MineResultDto` (toasted) instead of failing a mine whose note already exists;
      duplicates skip enrichment ("last card" would hit an unrelated note).
      Round 5 (maintainer feedback): (1) mine streams stage toasts via a
      `Channel<LoadingMessage>` (Rendering with Yomitan → Creating Anki note → Adding
      audio & screenshot via asbplayer) and returns `note_id` — the mined ✓ chip becomes
      a button that opens the card in Anki (`open_in_anki` → guiBrowse nid:) for
      session mines. (2) Anki-side deletions now reflect: Yomine's own mines live in a
      note-id-keyed cache (`yomine_mined_notes.json`) pruned against `findNotes(nid:…)`
      on every `get_mined_state`; the harvest cache is overwritten wholesale again
      (self-heals); the frontend clears its optimistic session sets after each
      successful refresh so backend state is authoritative. Old `anki_mined_sentences`
      recorded entries don't migrate (harvest rebuilds mapped ones). (3) Sentence-field
      guessing: literal "Sentence" → sentence-ish name (not audio/translation) →
      content heuristic; auto-fills the editor like term/reading. (4) Modal fixes:
      `.body { min-height: 0 }` (flexbox refused to scroll, footer pushed off-screen)
      + mapping rows restructured to name-line/fields-line with right-aligned actions.
      Round 6: (1) modals still overflowed under the Appearance zoom — vh/vw don't
      scale with root CSS `zoom` (same class as the T069 scrollbar bug); ALL ten modal
      dialogs now size in % of the backdrop (fixed inset:0 IS zoom-correct).
      (2) Sentence guessing rewritten: space/case-insensitive exact "Sentence" →
      shortest sentence-named field minus derived variants (audio/furigana/meaning/
      card/…) → context-ish names → content sniff; unit-tested against the
      maintainer's two real note types. (3) Searching text that matches inside a
      sentence now jumps the row to that occurrence (TermTable effect on tableSearch
      via the shared `textMatches`). (4) Yomitan indicator added to the top-bar status
      row (green/grey — optional, so grey not red when absent), driven by
      `yomitanReachable`.
      Round 7: Sentence Field editor row gets the guessed-＊ marker + sample Example
      (matching term/reading; the "optional" hint moved to the label tooltip).
      Recording lockout: `playerBusy` blocks seek + mine buttons from mine-click until
      cue duration (`end_secs - start_secs`) + 1.5s buffer after an asbplayer-path mine
      — clicking either mid-recording ruins asbplayer's clip; the post-mine
      mined-state refresh shifts by the same hold. Non-asbplayer/failed/duplicate
      mines unlock immediately.
      Subtitle formatting leak: `STRIP_INLINE_TAGS` whitelisted only b/i/u/font, so
      any other tag (`<c.jp>`, `<span>`, `<em>`, `<ruby>`, …) leaked into sentence
      text; now strips any HTML/WebVTT-style tag (+ existing `{\…}` overrides), and
      `read_txt` runs the tag strip too (copied-from-subtitles text). Unit-tested in
      parser.rs. NOTE: a styled sentence on a mined Anki CARD from the asbplayer path
      is asbplayer's own field write (its overwrite settings), not Yomine's text.
      Round 8: the tags the maintainer saw were Anki sample-note HTML in the settings
      Example previews — now stripped for display (`preview()` wraps all three
      term/reading/sentence examples). Stale sentence marks after deleting a note:
      the harvest cache had no note ids, so only the recorded-mines cache pruned;
      both caches now store `MinedSentence { note_id, sentence }` and
      `mined_sentences_pruned()` prunes BOTH against chunked `findNotes(nid:…)`
      queries (500/batch) on every refresh (offline → untouched). Old harvest cache
      format fails deserialize → empty → rebuilt on next full Anki pass.
      **Draft-until-built releases (maintainer request):** Manual Release now creates a
      DRAFT release, so users/updater never see a binary-less release. Drafts don't fire
      `release: created`, so manual-release invokes release.yml via workflow_call (new
      build-release job); tauri-action gets `releaseDraft: true`; checksums job switched
      robinraju-downloader/softprops-upload → `gh release download/upload` (gh can see
      drafts by tag); new publish-release job (needs checksums) flips draft→live via
      MY_PAT so `published` still triggers auto-release-notes. Failed-run recovery
      (re-run release.yml with same tag) documented in docs/RELEASES.md.

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

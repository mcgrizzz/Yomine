# Implementation Plan: Yomine on a Web-Based UI Shell (Tauri Port)

**Branch**: `feat/tauri-port` | **Date**: 2026-06-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/001-tauri-port/spec.md`

## Summary

Re-platform Yomine's UI from egui/eframe to Tauri v2 + SvelteKit while reusing the Rust
analysis engine unchanged. The migration is incremental and core-first: first make the engine
compile and run with no UI present and make its types serializable, then add a Tauri app that
exposes the engine via async commands and events, then port the UI feature-by-feature to parity
against the still-working egui app, then package and sign off. See [research.md](./research.md)
for stack decisions, [data-model.md](./data-model.md) for the IPC types, and
[contracts/](./contracts/) for the command/event API.

## Technical Context

**Language/Version**: Rust (edition 2021, toolchain 1.91); TypeScript for the frontend.

**Primary Dependencies**: Backend reused as-is — vibrato (segmentation), wana_kana,
jp-deinflector, reqwest (AnkiConnect), tokio, tokio-tungstenite (asbplayer WebSocket), serde,
rayon. New — Tauri v2 (`tauri`, `tauri-build`, plugins as needed: dialog, fs, opener).
Frontend — SvelteKit + `@sveltejs/adapter-static`, Vite, `@tauri-apps/api`.

**Storage**: JSON files in the OS app-data dir via the existing `persistence` module
(`dirs::data_local_dir()/yomine`). Runtime-downloaded unidic + frequency dictionaries and the
Anki vocab cache live in the same dir. No database.

**Testing**: `cargo test` (incl. `src/tests/segmentation_regression.rs`); `cargo build` matrix
(egui feature on/off); `cargo tauri dev` for manual parity checks; frontend `svelte-check`.

**Target Platform**: Desktop — Windows, macOS, Linux (same as today).

**Project Type**: Desktop app — Rust core/engine + Tauri shell + SvelteKit web frontend.

**Performance Goals**: Parity with egui on file processing time; no perceptible UI freeze on
load; responsive sort/filter/search/scroll on a typical episode-sized file (hundreds–low
thousands of terms). Heavy work runs off the UI thread.

**Constraints**: Offline-capable; engine results must match egui exactly; egui build must keep
working until parity; non-serializable runtime state stays in `tauri::State`.

**Scale/Scope**: ~16k LOC repo. ~8k LOC reusable engine (kept). ~8.2k LOC egui UI (rewritten in
Svelte). ~10 modal/feature areas plus the term table to port.

## Constitution Check

*GATE: Must pass before research. Re-check after design.*

- **I. Parity Before Replacement**: PASS — egui stays default-built and functional; Phase 1
  feature-gates rather than deletes it; egui is the verification reference.
- **II. Logic in Rust**: PASS — only client-side sort/filter/search of an already-enriched term
  list runs in Svelte; all analysis stays in the engine.
- **III. Serializable, Versioned IPC**: PASS — `data-model.md` defines the serde DTOs;
  `contracts/` is the stable API; `Tokenizer`/`FrequencyManager`/`IgnoreList` stay in state.
- **IV. Core Independent of UI**: PASS — Phase 1 moves `LanguageTools` and `WebSocketManager`
  out of `gui` and gates `gui`; the lib must build `--no-default-features`.
- **V. Verify Against Reference**: PASS — segmentation regression suite stays green; ported
  features checked against egui on identical input.
- **VI. Surgical Changes**: PASS — decoupling adds derives and moves two types; it does not
  restructure analysis modules.

No violations → Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/001-tauri-port/
├── plan.md              # This file
├── research.md          # Stack & decoupling decisions
├── data-model.md        # Serde DTOs crossing IPC
├── quickstart.md        # Dev/build/verify instructions
├── contracts/
│   ├── commands.md      # Tauri command API
│   └── events.md        # Event / channel payloads
├── checklists/
│   └── requirements.md  # Spec quality checklist
└── tasks.md             # Ordered tasks (Phase 2 of spec-kit)
```

### Source Code (repository root)

Convert the single crate into a Cargo workspace. The existing crate stays as the engine +
egui binary (egui gated behind a default-on feature); a new `src-tauri` crate hosts the Tauri
backend; the SvelteKit frontend lives under `src-tauri/ui`.

```text
Cargo.toml                     # workspace root (members: ".", "src-tauri")
src/                           # existing crate "yomine" (lib + egui bin)
├── core/
│   ├── language_tools.rs      # NEW home of LanguageTools (moved out of gui/app)
│   ├── models.rs              # + serde derives (Term, Sentence, SourceFile, TimeStamp)
│   ├── pipeline.rs            # import path updated (core::LanguageTools)
│   └── tasks/manager.rs       # egui-only; import path updated; gated with gui feature
├── segmentation/word.rs       # POS: + Serialize/Deserialize
├── websocket/manager.rs       # NEW home of WebSocketManager (moved out of gui)
├── player/mod.rs              # import path updated (websocket::WebSocketManager)
├── tools/...                  # + serde derives on result/summary types
├── gui/                       # unchanged behavior; gated behind `gui` feature
├── lib.rs                     # `#[cfg(feature = "gui")] pub mod gui;`
└── main.rs                    # egui binary; gated/required-features = ["gui"]

src-tauri/                     # NEW crate "yomine-tauri" (Tauri v2 backend)
├── Cargo.toml                 # depends on yomine { default-features = false }
├── tauri.conf.json            # window, bundle, resources (fonts, jlpt_vocab.json, icon)
├── build.rs
├── src/
│   ├── main.rs                # Tauri builder, state, command registration
│   ├── state.rs               # AppState (Mutex) holding engine handles + current file
│   ├── commands/              # one module per command group (file, terms, anki, player,
│   │                          #   settings, dictionaries, analyzer, knowledge)
│   ├── events.rs              # event names + payload structs (mirror contracts/events.md)
│   └── background.rs          # spawned loops: anki poll, ws/mpv switch, knowledge summary
└── ui/                        # NEW SvelteKit frontend (adapter-static)
    ├── src/
    │   ├── lib/ipc.ts         # typed wrappers over invoke()/listen()
    │   ├── lib/stores/        # term list, settings, status, player
    │   ├── lib/components/    # TermTable, SentenceView (furigana), modals, top bar
    │   └── routes/+page.svelte
    ├── svelte.config.js
    ├── vite.config.ts
    └── package.json
```

**Structure Decision**: Workspace with the existing `yomine` crate (engine + egui, egui behind
a default feature) plus a new `src-tauri` crate that depends on `yomine` with
`default-features = false`. This keeps the engine in one place (least churn — no mass file
move), guarantees egui is excluded from the Tauri build, and matches Tauri's conventional
`src-tauri/` layout. The only files that move are `LanguageTools` and `WebSocketManager`, which
must leave `gui` to satisfy Principle IV.

## Phased Approach (maps to tasks.md)

- **Phase A — Decouple core** (blocks everything): workspace + `gui` feature gate + move the two
  shared types + serde derives. Egui still default-builds and runs; `--no-default-features`
  builds the engine alone; tests green.
- **Phase B — Tauri scaffold + IPC**: `src-tauri` + SvelteKit; `AppState`; port `TaskManager`
  methods to async commands + events; background loops. Verify a real round trip.
- **Phase C — Port UI to parity**: shell/theme/fonts/top bar; term table (egui inline rows —
  Term+furigana-above │ inline Sentence │ Freq │ POS; columns, sort/filter/search, furigana
  sentences, multi-sentence, ignore, seek; virtualization deferred until a large file needs it);
  all modals; knowledge widget. Verify each against egui.
- **Phase D — Package & sign off**: bundle resources; Tauri bundler in CI replacing egui release
  workflows; parity checklist sign-off; decide egui retirement.

## Complexity Tracking

No constitution violations. No entries.

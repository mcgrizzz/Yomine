# Quickstart: Developing & Verifying the Tauri Port

All work is on `feat/tauri-port`. Prerequisites verified in this environment: `cargo`/`rustc`
**1.96** (egui 0.34.3 needs rustc ≥1.92 — run `rustup update stable` if yours is older),
`node` v22, `pnpm`/`npm`, `uvx`. `cargo-tauri` is **not** installed yet (Phase B installs it).
The egui WIP that was on `main` is parked at `git stash@{0}` (recover with `git stash pop`).

## Build matrix (the parity guardrail)

```bash
# 1. egui app still builds and runs (default features include `gui`)
cargo build
cargo run                      # launches the egui app — the reference implementation

# 2. engine builds with NO ui framework (Constitution IV)
cargo build -p yomine --no-default-features

# 3. analysis behavior unchanged
cargo test                     # includes src/tests/segmentation_regression.rs
```

Phase A is done only when all three pass and the egui app behaves exactly as before.

## Tauri dev loop (Phase B+)

```bash
# one-time
cargo install tauri-cli --version "^2"      # provides `cargo tauri`
cd src-tauri/ui && pnpm install && cd ../..

# run the Tauri app with the SvelteKit dev server + Rust hot-reload
cargo tauri dev
```

`cargo tauri dev` starts Vite (SvelteKit) and the Rust backend together. Frontend-only checks:

```bash
cd src-tauri/ui
pnpm run check        # svelte-check / tsc
pnpm run build        # adapter-static output Tauri will bundle
```

## Packaging (Phase D)

```bash
cargo tauri build     # produces .msi/.exe (Win), .dmg/.app (macOS), .AppImage/.deb (Linux)
```

## Manual parity checklist (run new app side-by-side with `cargo run` egui)

For the **same** input file, Anki state, and enabled dictionaries, confirm equality:

1. **US1 Mining**: open an SRT → identical term count, ordering, readings, POS, frequencies;
   expand a term → sentences render with furigana; drag-and-drop opens the same way.
2. **US2 Anki**: with a mapped note type, same terms hidden, same comprehension %; cached load
   then live background refresh updates in place; works offline.
3. **US3 Player**: asbplayer connected → timestamp click seeks; MPV preferred when both present;
   no-player click reports unavailable.
4. **US4 Refine**: each sort, POS filter, frequency-range filter, search query, and ignore-list
   add/remove yields the same visible set as egui.
5. **US5 Config**: change Anki mapping, port, dict weights, POS defaults, theme, font → restart
   → all persisted and applied; setup checklist reflects true state.
6. **US6 Analyzer**: run over a small fixed file set → same ranking; export Yomitan zip + CSV →
   equivalent artifacts; cancel mid-run works.
7. **US7 Knowledge**: with a cached Anki snapshot, JLPT + frequency-band values match egui; mode
   toggle switches coverage/estimate.

## Where things live

- Reference app: `cargo run` (egui). Engine: `src/{core,dictionary,segmentation,anki,tools,
  player,websocket,parser,persistence}`.
- New backend: `src-tauri/src`. New frontend: `src-tauri/ui/src`.
- Spec artifacts: `specs/001-tauri-port/`. Principles: `.specify/memory/constitution.md`.

## Notes

- Commit only when the maintainer asks (Constitution / repo policy). Spec-kit's optional
  auto-commit hooks are intentionally not run.
- Settings on-disk format is unchanged, so the same `settings.json` works in both apps during
  the transition.

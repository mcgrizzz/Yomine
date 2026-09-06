# Mining accuracy v2 verification

Use branch `feat/mining-accuracy-v2` in the main worktree:
`/mnt/h/documents/dev/rust/japanese-mining/yomine`. Open it with `code .` from WSL,
or open `H:\Documents\Dev\rust\japanese-mining\yomine` in Windows VS Code.
Rebuild/restart the app and reload the subtitle file after changing the backend.

## Matching behavior

- **Known**: an exact normalized card, validated citation, precomputed kana preference,
  or supported lexical family establishes the match. The term filters normally and
  contributes comprehension based on the matched card.
- **Possible**: a plausible card exists, but competing or missing lexical evidence
  prevents establishing identity. The term stays unknown and minable with zero
  known-word comprehension. Hover or focus its question-mark icon for the candidate.
- **Unmatched**: no supported card proposal exists. The term stays unknown without
  an uncertainty icon.

The precomputed policy uses JMdict, Jitendex and independent frequency sources.
It selects a usual dictionary identity per reading and POS, not a sentence's meaning.
See [the policy and reproduction instructions](../../research/lexical/RUNTIME.md).
Anilist Top 500 must never be used as reference data or regression evidence.
Installed dictionaries, including disabled dictionaries and marker entries, still
supply the lexical fallback. Reading-less records supply no reading candidates.
Written homophones remain separate; documented spelling aliases require their stated reading.

## Automated checks

Keep the same Rust toolchain and ignored `Cargo.lock` for this branch and the main
comparison worktree; use `--locked` instead of regenerating dependencies.

```sh
cd /mnt/h/documents/dev/rust/japanese-mining/yomine
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine YOMINE_REQUIRE_UNIDIC=1 cargo test --workspace --lib --test segmentation --test phrase_processing --locked -- --include-ignored
cargo +nightly fmt --all -- --check
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine cargo clippy --workspace --all-targets --locked
python3 -m unittest discover -s research/lexical -p 'test_*.py'
```

From a terminal with Node and pnpm available, run in `src-tauri/ui`:

```sh
pnpm check
node tests/possible-matches.mjs
pnpm build
```

The Rust tests require installed UniDic and use isolated in-memory card snapshots.
They do not fetch live Anki or modify real caches or dictionaries. Every command
reading installed real data must explicitly set `YOMINE_DATA_DIR` as above.
The phrase fixture protects production/corpus differences, highlights and later
batch deinflection; do not regenerate it to accept a refactor regression.

## Manual checks

Use a disposable data/profile copy for interactive card changes. Never replace the
real cache or refresh live Anki against it during these checks.

1. Load 形態化させて. Expect 形態 / 化させて, citation 化する and the full 化させて
   mining highlight. An enclosing 形態化する expression may coexist with its components.
   Check 要らないです, 食べないです, 行かないです and 知らなかった for citations
   要る, 食べる, 行く and 知る. Preserve lexicalized つまらない and intact おばさん.
2. Check なんとなく promotion and its existing surface frequency. A representative
   何と無く should match a 何となく card. Promoted expressions must remain contiguous
   source spans. Empty expressions and pronoun なん must gain no inferred identity.
3. With matching cards, check なし → 無し, みせる → 見せる, まさに → 正に,
   できる → 出来る, こと → 事 and いく → 行く. Include
   今までに算術ができなくて困ったことはありますか？. Wrong-homophone cards must
   not substitute: 見せる and 診せる remain separate; 行く and 往く retain their alias.
4. Use 橋[はし] against ambiguous はし to check uncertainty. “Show uncertain matches”
   defaults on for older settings. Turn the switch off, restart, then turn it back on:
   visibility persists and reverses without changing known counts or comprehension.
   The icon should show details on hover/focus and dismiss them when leaving it.
   Clicking frequency should select the row without selecting the rank text.
5. Remove the matching card and refresh: stale labels and comprehension must clear.
   Add an exact card and refresh: the uncertainty icon clears and the term becomes known.
   Repeat through ignore-list refresh; possible matches must never gain known highlighting.
6. Save several dictionary changes together and repeat while a file is loaded. Expect
   one complete refreshed result per batch, using cleaned text and preserving source
   metadata. Derived segments must not accumulate. Older work must not replace a newer
   result, and changing a display preference must not cancel dictionary refresh.

UniDic/Vibrato remains the runtime tokenizer. Ichiran is a segmentation comparison
only. Performance claims require a controlled release benchmark with identical
subtitles, dictionaries and isolated Anki snapshots.

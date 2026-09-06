# Precomputed kana matching

Work on `feat/mining-accuracy-v2` in the main worktree, `yomine`. Open that folder in
VS Code (`code .` from its WSL terminal). Rebuild/restart the app and reload the file
when changing the bundled reference.

The compiler selects a preferred JMdict identity per reading and coarse POS using
legal JMdict spelling/reading relationships, Jitendex entry coverage, and explicit
kana-marker links from the pinned frequency sources. Jitendex derives from JMdict;
it is not independent corroboration. A usually-kana tag is not required: common
kana usage can be explicitly linked even for entries such as 見せる without that tag.

Anilist Top 500 is excluded from imports, compilation, analysis, source manifests
and regression evidence: it is Yomine-generated output, not independent evidence.
Only JPDB, Jiten, BCCWJ, CC100 and VN Freq contribute frequency inputs. Legacy
databases are filtered as well; the rebuilt independent database contains no Anilist rows.

The offline rule is:

- Require an explicit written-entry/kana-frequency link. A standalone kana rank
  cannot identify its written homophone. Zero ranks and missing readings do not vote.
- Compare ranks only within the same source. Use the better of the written rank
  and an unshared kana rank. A kana rank repeated across different JMdict identities
  supplies no identity-specific rank; the written ranks can still distinguish them.
- Require at least tenfold rank separation from every competing identity in at
  least one comparable source, with support from a source linking the target to kana.
  A linked source ranking a rival equally or better vetoes the selection. Written-only
  sources may fill rival coverage, but cannot independently support or veto it.
- Missing rival coverage remains unresolved. Keep identities separate across shared
  spellings, and exclude shared, search-only, rare, obsolete and no-kanji target forms.
  Competitors are counted before these exclusions or Jitendex coverage checks.

Tenfold separation is a conservative heuristic, not a probability. There is no
threefold kana/written-rank closeness requirement or absolute top-1,000 cutoff.
The reference selects the usual dictionary identity, not a sentence's meaning.
POS comes from ordinary extraction; no extra N-best consensus or sentence pass is added.
Written citations and exact cards still take precedence.

Examples: みせる → 見せる, なし → 無し, まさに → 正に, できる → 出来る,
こと → 事, いく → 行く. はし remains unresolved. A card for 診せる, 梨, 将に,
出切る, 琴 or 逝く does not substitute for the preferred identity. Reverse matching
from a written term to a kana card uses the same preference. These are selections
among entries, not homophone merges or handwritten exception lists.

Runtime classification reads this table before the existing lexical-family fallback.
Runtime frequency-ratio decisions and per-occurrence context bookkeeping are removed.
The bundled decision is independent of cards, installed dictionary enablement, weights,
and subsequent frequency-dictionary changes; changing it requires regeneration/rebuild.
Installed dictionaries still supply the existing lexical fallback and frequency display.

Known matches filter and count toward comprehension. Possible matches remain minable
with zero known comprehension; Unmatched terms have no uncertain label. Both file
filtering and word statistics use the same classifier. Removing/replacing cards clears
stale labels and comprehension. The “Show uncertain matches” switch only changes visibility.

## Storage and reproduction

`assets/kana-preference.bin`: 53,645 records, 3,203,432 bytes, embedded with
`include_bytes!`. Sorted UTF-8 keys contain reading + tab + POS bit. The 12-byte
header contains `KANAIDX2` and a little-endian u32 row count. Each directory record
has four u32 values: absolute key offset, key length, value offset, value length.
Values contain u32 JMdict ID, u16 target count, then targets: u32 POS mask,
u16 UTF-8 length, spelling. POS bits: noun 1, verb 2, i-adjective 4,
adjectival noun 8, adverb 16.

Checked slices and binary search avoid startup deserialization, heap indexing,
decompression and runtime database scans. Lookup folds full-width katakana to hiragana;
it does not guess long vowels or romaji. Tests check every compiled key. No full-app
startup or mining-speed improvement is claimed.

Build the full reference using README.md, then regenerate:

```sh
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine python3 research/lexical/compile_runtime.py --db ../yomine-lexical-research/lexical-reference-independent.sqlite --output assets/kana-preference.bin
python3 -m unittest discover -s research/lexical -p 'test_*.py'
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine YOMINE_REQUIRE_UNIDIC=1 cargo test --workspace --lib --test segmentation --test phrase_processing --locked -- --include-ignored
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine cargo run --release --locked --example kana_index_benchmark
```

The manifest pins JMdict (2026-09-05), Jitendex (2026.08.11.0), every frequency bank,
the generator and output checksums. Source ranks are not bundled. The large research
database is not shipped. Attribution appears in About and `assets/kana-preference.LICENSE.md`.
Real dictionaries and Anki caches are never modified by this compiler or its tests.

## Manual checks

With isolated cards 見せる[みせる], 無し[なし], 正に[まさに], 出来る[できる],
事[こと] and 行く[いく], load おっきな花火を上げてみせるわ, 大喜び 間違いなし,
まさにその通りだ。, 今までに算術ができなくて困ったことはありますか？ and 学校にいく。.
Confirm those terms filter and contribute known comprehension. Remove the cards and
refresh: the terms return without stale labels or known comprehension. Re-add and
refresh to restore established matches. Wrong-homophone cards must not filter them.
Use 橋[はし] against はし to check uncertainty, reversible visibility, unchanged
known counts and the question-mark details control. Repeat dictionary changes/file
reloads to check stale-result rejection.

# Precomputed kana matching

The application selects common kana/written identities from a bundled table. It uses
JMdict spelling/reading restrictions, Jitendex entry coverage and independent frequency
sources. The table is generated offline; installed dictionaries still supply lexical
fallbacks and displayed frequencies. See [manual verification](../../tests/manual/mining-accuracy-v2.md)
for segmentation, card refresh and uncertainty controls.

## Policy

- Require an explicit written-entry/kana-frequency link. A standalone kana rank,
  a zero rank or a missing reading cannot identify a written homophone.
- Compare ranks within each source. Use the better of the written rank and an
  unshared kana rank. A kana rank repeated across JMdict identities supplies no
  identity-specific rank; written ranks can still distinguish them.
- Require tenfold separation from every competitor in at least one comparable
  source, with support from a source linking the target to kana. A linked source
  ranking a rival equally or better vetoes selection. Written-only sources may
  fill rival coverage but cannot independently support or veto a selection.
- Missing competitor coverage stays unresolved. Keep identities separate across
  shared spellings; exclude shared, search-only, rare, obsolete and no-kanji target
  forms. Count competitors before exclusions or Jitendex coverage checks.

Tenfold separation is an ordinal-rank heuristic, not a probability. The table chooses
an identity per reading and coarse POS, not a sentence's meaning. There is no extra
sentence analysis pass, top-1,000 cutoff or kana/written-rank closeness requirement.
Exact cards and validated written citations take precedence. Documented spelling
aliases require their stated reading.

Examples include みせる → 見せる, なし → 無し, まさに → 正に, できる → 出来る,
こと → 事 and いく → 行く. はし stays unresolved. Cards for 診せる, 梨, 将に,
出切る, 琴 or 逝く cannot substitute for the preferred identity. Reverse matching
from a written word to a kana card follows the same preference.

Known matches filter and contribute comprehension. Possible matches remain minable
with zero known comprehension; unmatched words have no uncertainty icon. The
“Show uncertain matches” switch changes visibility only. Dictionary weights,
enabled states and Anki cards cannot change the bundled policy; updating it requires
regeneration and a rebuild.

## Sources and storage

[assets/kana-preference.json](../../assets/kana-preference.json) pins the JMdict
2026-09-05 snapshot, Jitendex 2026.08.11.0, frequency-bank hashes, compiler hash and
source-policy hash. Jitendex derives from JMdict, so coverage is a consistency check,
not independent corroboration. Frequency inputs are JPDB, Jiten, BCCWJ, CC100 and VN Freq.

**Never use Anilist Top 500 as reference, analysis or validation evidence.** Yomine
generated it and it can reproduce past systemic errors. The importer skips its
banks; `source_policy.py` also excludes it when compiling a legacy database.

`assets/kana-preference.bin` contains 53,645 records in 3,203,432 bytes. It is embedded
with `include_bytes!` and searched directly, without startup deserialization or a
heap index. Its 12-byte header contains `KANAIDX2` and a little-endian u32 row count.
Each directory record has four u32 fields: key offset/length and value offset/length.
UTF-8 keys are reading + tab + POS bit. Values contain a u32 JMdict ID, u16 target
count, then targets: u32 POS mask, u16 UTF-8 length and spelling. POS bits are noun 1,
verb 2, i-adjective 4, adjectival noun 8 and adverb 16. Lookup folds full-width
katakana to hiragana without guessing long vowels or romaji. Rust tests check every key.

The offline SQLite database is not shipped. Schema 3 retains legal JMdict pairs,
Jitendex entry/spelling/reading rows, frequency records and provenance. Sense
restrictions and POS inheritance determine legal pairs; unused glosses, tag trees,
portable exports and experimental context classifiers are not maintained here.
The earlier study and its outputs remain in Git history through `55aa638`.

## Reproduction

Use Python 3.10+ and the existing Rust toolchain. The pinned local sources are in
`../yomine-lexical-research/`. New snapshots come from
[JMdict](https://www.edrdg.org/pub/Nihongo/JMdict_e.gz) and
[Jitendex](https://jitendex.org/pages/downloads.html); the importer requires existing
files and records their hashes. Choose a new scratch database path on each rebuild.

```sh
python3 -m unittest discover -s research/lexical -p 'test_*.py'
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine python3 research/lexical/build.py --jmdict ../yomine-lexical-research/JMdict_e.gz --jitendex ../yomine-lexical-research/jitendex-yomitan.zip --frequency-dir /mnt/c/Users/Andrew/AppData/Local/yomine/dictionaries/frequency --output /tmp/new-lexical-reference.sqlite
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine python3 research/lexical/compile_runtime.py --db /tmp/new-lexical-reference.sqlite --output /tmp/new-kana-preference.bin
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine YOMINE_REQUIRE_UNIDIC=1 cargo test --workspace --lib --test segmentation --test phrase_processing --locked -- --include-ignored
```

Review source/version changes and test the production pipeline before replacing the
bundled binary and manifest. For a code-only refactor, the rebuilt binary must match
the committed SHA-256 exactly. Tests use isolated card snapshots. No command writes
installed dictionaries or the real Anki cache. Speed claims require a controlled
release benchmark using identical subtitles, dictionaries and card snapshots.

Derived data is attributed to James William Breen / EDRDG (JMdict) and Stephen Kraus
and contributors (Jitendex), under CC BY-SA 4.0. Retain
[the attribution and licence](../../assets/kana-preference.LICENSE.md) when distributing it.

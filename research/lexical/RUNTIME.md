# Dictionary-backed kana matching

Implemented in `feat/mining-accuracy-v2`, sibling worktree `yomine-mining-accuracy-v2`.
Open it from WSL with `code ../yomine-mining-accuracy-v2` when starting in the original repository.
Rebuild/restart the app and reload the subtitle file to apply the bundled reference.

The bundled JMdict (2026-09-05) and Jitendex (2026.08.11.0) reference adds explicit
usually-kana evidence. Jitendex derives from JMdict; agreement is not independent corroboration.
An extracted kana term must agree with the reference's reading, the sentence-selected
UniDic lexeme and part of speech. The compiler requires exactly one kana-preferred
JMdict identity for that reading before coverage filters. Spelling targets must share
a permitted sense with the lexeme. Search-only, rare, obsolete, no-kanji and unsupported
forms are excluded. No frequency rank is converted into a probability.

This establishes こと → 事, できる → 出来る (including できなかった), and
いく → 行く in the tested contexts. It does not choose 琴, 出切る or 逝く simply
because that card exists. はし with context 橋 or 箸 does not become 嘴.
UniDic can still misinterpret a sentence: this is dictionary-supported inference,
not guaranteed semantic disambiguation. Conflicting or missing occurrence contexts
cannot supply this evidence for a merged row. Promoted phrases do not inherit a head's identity.

Exact-card/citation matching remains first. Existing lexical/frequency matching remains
the fallback when this reference cannot select an interpretation. Uncertain matches remain
minable and contribute zero known comprehension; the visibility toggle only hides their rows.
Removing cards clears stale labels and comprehension. Extracted-term statistics use the
same classifier and context as file filtering. Context-free `word_stats` calls do not invent
sentence evidence; reverse kana-card matching retains the existing lexical/frequency policy.

## Storage and reproduction

`assets/kana-preference.bin`: 10,560 records, 769,332 bytes (751 KiB), embedded using
`include_bytes!`. Sorted UTF-8 keys contain reading + tab + contextual lexeme. A 12-byte
header contains `KANAIDX1` and a little-endian u32 row count; each directory record is four
u32 values (absolute key offset, key length, value offset, value length). Values contain
u32 JMdict ID, u16 target count, then targets (u32 POS mask, u16 UTF-8 length, spelling).
POS bits: noun 1, verb 2, i-adjective 4, adjectival noun 8, adverb 16.
The loader uses checked slices, no unsafe code, no heap index, no decompression and no
runtime database scan. Queries fold full-width katakana to hiragana, without guessing
long vowels or romaji. Unsupported normalization abstains. Asset tests check every key.

Build the full reference using README.md, then regenerate deterministically:

```sh
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine python3 research/lexical/compile_runtime.py --db ../yomine-lexical-research/lexical-reference.sqlite --output assets/kana-preference.bin
python3 -m unittest discover -s research/lexical -p 'test_*.py'
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine cargo run --release --locked --example kana_index_benchmark
```

The manifest pins both source checksums and the generator checksum. The adapted data is
CC BY-SA 4.0; attribution appears in the About dialog and `assets/kana-preference.LICENSE.md`.
The large research database is not shipped. Updating installed frequency dictionaries
continues to replace their lexical cache; updating this bundled reference requires regeneration
and a rebuild. Neither process changes real Anki cards or the source dictionaries.

Warm resident-byte microbenchmark, Rust 1.96 release build on this WSL environment:
JSON byte-array records 2,156,246 bytes / 6.793 ms initialization; bincode 621,483 bytes /
1.138 ms; direct table 769,332 bytes / 0.005 microseconds header initialization and
0.080 microseconds per raw-key lookup. Owned-format measurements include destruction;
lookup excludes Japanese normalization. The same keys and values were used. Direct lookup
was chosen to avoid deserialization, at a 148 KB storage cost over bincode. These are not
cold disk, full app startup, or end-to-end mining performance claims.

## Manual checks

With isolated cards 事[こと], 出来る[できる], 行く[いく], load sentences containing
大事なことだ。, 日本語ができる。, 昨日はできなかった。, 学校にいく。.
Confirm those terms are established and contribute known comprehension. Remove the cards
and refresh: the terms return with zero known comprehension. Re-add cards and refresh:
established status returns without stale uncertain labels. A card 琴[こと] or 出切る[できる]
must not establish the competing common term. Toggle “Show uncertain matches” on remaining
ambiguous rows and verify counts/comprehension do not change. Repeat dictionary changes
and file reloads; existing stale-generation checks must still pass.

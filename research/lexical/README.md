# Offline lexical-reference research

Read [REPORT.md](REPORT.md) for the measured findings and limitations.
The study uses full JMdict and Jitendex, all six installed frequency dictionaries,
and installed UniDic over the cached subtitle corpus. Production matching is unchanged.

## Ready-to-use local artifacts

Artifacts are in the sibling directory `../yomine-lexical-research/` relative to the
worktree root (`yomine-mining-accuracy-v2`). They are intentionally not committed.

- `lexical-reference.jsonl.gz`: complete portable reference, about 14.3 MB compressed.
- `lexical-reference.sqlite`: indexed research database, about 544 MB including frequencies.
- `lexical-reference.manifest.json`: source versions, hashes, bank hashes, entity codes and attribution.
- `artifacts.json`: checksums for the generated artifacts.
- `report-final/summary.json`: full-vocabulary counts and source provenance.
- `report-final/ambiguous_readings.csv`: every ambiguous reading and its entry IDs.
- `report-final/jitendex_disagreements.json`: source/version/scope discrepancies to review.
- `report-final/evaluation.json`: complete probe outcomes and candidate evidence.
- `report-final/corpus_review_sample.json`: local first-100 inspection sample.
- `sentences.jsonl`, `sentences.manifest.json`, `tokens.jsonl`: isolated corpus inputs,
  file hashes/splits, and contextual UniDic output. Subtitle text stays local.

Query an entry from the worktree root:

```sh
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine \
python3 research/lexical/analyze.py \
  --db ../yomine-lexical-research/lexical-reference.sqlite lookup できる
```

The result exposes competing JMdict entries, legal spellings, reading, POS, sense IDs
with explicit kana preference, search-only spellings, and Jitendex consistency.
`findings.json` and `probe-results.json` are small committed snapshots of this run.

## Portable schema

The gzip stream contains newline-delimited JSON. The first record contains `manifest`.
Every later record has `jmdict` (an entry object, or null when unmatched) and `jitendex`
(a list of source rows). All source rows are retained. Source sequence numbers remain
signed: a negative Jitendex redirect is not another JMdict identity.

JMdict entry objects preserve:

- stable entry ID;
- written forms with spelling-info and priority codes;
- readings with `re_restr`, no-kanji status, info and priority codes;
- senses with `stagk`/`stagr`, explicit and effective POS, explicit misc codes,
  fields, dialects, notes and cross-references.

Jitendex rows preserve their term, supplied reading (including absence), tags, word
classes, score, signed source sequence and projected lexical structure. The projection
retains sense-group/sense boundaries, lexical tag codes and redirect targets/readings,
while omitting glosses and examples. Scores are stored, not interpreted as probabilities.
Grouping a redirect with an entry preserves a source reference, not an unconditional alias.

SQLite `pairs` contains only legal entry/reading/spelling/sense combinations. POS
inheritance follows JMdict's DTD. Explicit misc codes stay sense-local; the original
Jitendex group structure is preserved separately. Reading normalization uses NFKC
and katakana-to-hiragana folding, without guessing long vowels or merging homophones.

## Reproduce or update

Python 3.10+ standard library and the existing Rust toolchain are sufficient. Download
`JMdict_e.gz` from <https://www.edrdg.org/pub/Nihongo/JMdict_e.gz> and the full Yomitan
ZIP linked at <https://jitendex.org/pages/downloads.html> into a separate research folder.
The builder requires existing files and records their exact hashes; it does not silently
fetch changing data. Use the manifest to identify the snapshots from this run.

From the worktree root, choose a **new** output filename on every rebuild. A Linux-local
scratch path avoids repeated filesystem round trips during SQLite indexing. Copy the
finished database and manifest into the sibling research folder afterward.

```sh
python3 -m unittest discover -s research/lexical -v

YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine \
python3 research/lexical/build.py \
  --jmdict ../yomine-lexical-research/JMdict_e.gz \
  --jitendex ../yomine-lexical-research/jitendex-yomitan.zip \
  --frequency-dir /mnt/c/Users/Andrew/AppData/Local/yomine/dictionaries/frequency \
  --output /tmp/new-lexical-reference.sqlite

YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine \
python3 research/lexical/analyze.py --db /tmp/new-lexical-reference.sqlite \
  audit --output ../yomine-lexical-research/new-report

YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine \
python3 research/lexical/analyze.py --db /tmp/new-lexical-reference.sqlite \
  export --output ../yomine-lexical-research/new-reference.jsonl.gz

YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine \
python3 research/lexical/prepare_corpus.py \
  --subtitles /mnt/c/Users/Andrew/AppData/Local/yomine/asbplayer_subtitles \
  --probes research/lexical/probes.json \
  --output ../yomine-lexical-research/new-sentences.jsonl

YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine YOMINE_REQUIRE_UNIDIC=1 \
cargo run --locked --example lexical_research -- \
  ../yomine-lexical-research/new-sentences.jsonl ../yomine-lexical-research/new-tokens.jsonl

YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine \
python3 research/lexical/analyze.py --db /tmp/new-lexical-reference.sqlite evaluate \
  --tokens ../yomine-lexical-research/new-tokens.jsonl \
  --probes research/lexical/probes.json \
  --output ../yomine-lexical-research/new-report
```

Review source-version changes, legal-pair coverage, redirect failures and kana-metadata
disagreements before replacing a reference. Do not silently replace a source snapshot
under an existing matching cache. These research commands never read live Anki cards
or write installed dictionaries/caches. The tokenizer must already be installed for
real-data runs; load failure is fatal rather than a silent skip.

## Attribution and scope

Derived lexical data uses JMdict, copyright James William Breen and the Electronic
Dictionary Research and Development Group, and Jitendex, copyright Stephen Kraus
and contributors. It is provided under CC BY-SA 4.0; retain the source and licence
links when distributing derived lexical data:

- <https://www.edrdg.org/edrdg/licence.html>
- <https://jitendex.org/pages/legal.html>

Local frequency snapshots and subtitle text are research inputs, not redistributed
with the portable lexical reference. The full database remains local. Jitendex and
JMdict are related sources, not independent votes. No startup tokenization of the
full vocabulary, runtime dependency on Yomitan, or performance claim is introduced.

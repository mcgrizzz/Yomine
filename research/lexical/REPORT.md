# Lexical-reference study — 2026-09-05

The full dictionaries support replacing the rank-similarity gate with lexical evidence,
but they do not support blindly treating the only kana-preferred entry as known.
A research-only hint combining kana preference, a legal dictionary spelling/reading
pair, Jitendex consistency, and the contextual UniDic lexeme handles the three reported
examples. It abstains on the tested ambiguous はし cases. It is not wired into production.

Source correction (2026-09-06): Anilist Top 500 is excluded because Yomine generated
it. The frequency audit and totals below were regenerated from the five independent
sources. The JMdict/Jitendex-only probe results are unaffected. See RUNTIME.md for
the subsequently implemented matching policy.

## Sources and reproducibility

| Source | Version | Records processed |
| --- | --- | ---: |
| Full JMdict English XML | 2026-09-05 | 218,714 entries |
| Full Jitendex Yomitan export | 2026.08.11.0 | 435,448 rows |
| Installed frequency dictionaries | five independent read-only snapshots | 1,933,609 records |
| Cached subtitle corpus | 47 files | 16,087 captions |

JMdict supplies entry IDs, spelling/reading restrictions, per-sense restrictions,
POS and kana-preference codes. Jitendex supplies its spelling/reading rows, tags,
verb classes, scores, sense-group structure, and explicit redirect targets. Jitendex
is derived from JMdict; agreement is a consistency check, not independent evidence.
The installed Yomitan spot-check used Jitendex 2025-12-02, whereas the full export
above is newer. Versions must not be silently treated as identical.

Source URLs, SHA-256 hashes, frequency-bank hashes, entity-code descriptions,
attribution and builder hash are stored in the reference manifest. The portable
export preserves all 218,714 JMdict entries and all 435,448 Jitendex rows; records
without a current JMdict counterpart remain explicit rather than being dropped.
The approximately 14.3 MB compressed export excludes glosses and example text.
The approximately 518 MB SQLite research database additionally retains all frequency
records and their source bank/row locations. Neither is loaded by the application.

## Full-vocabulary findings

- 225,449 normalized readings; 14,181 belong to more than one JMdict entry.
- 1,169 ambiguous readings have exactly one kana-preferred entry; 191 have several.
- 138,529 Jitendex rows are redirects. Their negative sequence numbers are preserved;
  they are not independent words or permission to merge every spelling. 133,640
  targets validate against a legal JMdict spelling/reading pair; 4,889 remain unresolved.
- 48 spelling/reading groups disagree about the presence of kana-preference metadata
  between these JMdict and Jitendex snapshots. The complete list is in the local report.
  Version differences and sense-group scope need review before interpreting these as errors.
- 34,348 frequency-source/reading/rank groups repeat a kana rank under multiple spellings.
  These include both variants and homophones; repeated ranks cannot establish identity.

| Frequency source | Linked spelling pairs | Kana ranks better | Kana rank >3× better | Kana ≤1,000, written >1,000 |
| --- | ---: | ---: | ---: | ---: |
| JPDBv2㋕ | 59,864 | 14,798 | 4,635 | 354 |
| Jiten | 111,209 | 28,162 | 8,309 | 448 |

These are ordinal-rank distributions, not occurrence ratios or probabilities. They
show that the current gate excludes a substantial class of ordinary kana-preferred
spellings. All comparisons remain within the same source and reading; bare BCCWJ
records contribute no readings or inferred lexical identity.

## Focused evaluation

The candidate hint requires exactly one entry with applicable explicit JMdict `uk`
metadata and a legal corresponding Jitendex row with kana metadata. The stricter
variant also requires the sentence's UniDic lexeme to be a legal spelling of that
entry at the same reading. No Anki cards or frequency thresholds select the entry.
This is a hypothesis test; it does not resolve which sense the sentence uses.

| Set / rule | Correct selected entries | Incorrect selections | Abstentions | Not evaluable |
| --- | ---: | ---: | ---: | ---: |
| 3 development probes, tag only | 3 | 0 | 0 | 0 |
| 3 development probes, tag + lexeme | 3 | 0 | 0 | 0 |
| 17 holdout probes, tag only | 7 | 2 | 6 | 2 |
| 17 holdout probes, tag + lexeme | 7 | 0 | 8 | 2 |

The development cases are こと→事, できる/できなくて→出来る, and いく→行く.
The correct holdout selections include すぐ, もし, すべて, ありがとう, もちろん,
なぜ and うまい. The tag-only rule wrongly selects 嘴 in both はし probes;
contextual agreement rejects those hints. This small, assistant-authored set is
not an independent accuracy benchmark and the zero observed incorrect selections
must not be presented as a guarantee.

Two predeclared controls remain unevaluated: UniDic splits 出切った into 出る + 切る,
and reads 琴 as キン in 琴を弾く. The other eight abstentions remain visible, including
たぶん, わけ, あと and difficult homophones. No expected answers were altered to
turn these outcomes into successes. Raw-token analysis is intentionally separate
from production phrase promotion/citation handling, so phrase-level behavior is
not measured by this experiment.

## Subtitle corpus and card checks

The corpus split is deterministic by whole-file SHA-256 modulo five. It is not a
semantic train/test split; alternate subtitle versions can share content. The 20
focused probes were fixed before evaluating the hypotheses; no thresholds were
fitted to these outcomes.

Across the corpus, 74,689 kana token occurrences have dictionary candidates.
The tag-only rule suggests 18,329 entries; the stricter rule suggests 13,416.
Of those stricter hints, 3,750 occur in the file holdout split. These are coverage
counts, not correct-match counts. A first-100 review sample is stored locally;
it is an inspection aid, not a random or independently labeled evaluation set.

Fifteen tests check reading and sense restrictions, no-kanji readings, POS inheritance,
search-only metadata, script normalization, Jitendex scope and redirects, complete
export retention, and isolated matching/absent/removed/competing/wrong-reading card
fixtures. A card can only match the selected identity; its presence cannot select it.
These fixtures test the research classifier and do not change production comprehension.

## Recommended use in Yomine

1. Use the generated entry/reading/sense reference to establish documented relationships.
   Preserve competing entries, `re_nokanji`, reading restrictions and search-only status.
2. Treat kana preference and morphology as candidate evidence. Keep explicit written
   citations ahead of inferred alternatives; retain unresolved cases rather than hiding them.
3. Use frequency to rank supported interpretations, without requiring written and kana
   spellings to have similar ranks. Do not merge identities through rank or source counts.
4. Before enabling additional automatic filtering, review occurrence-level errors on a
   separate subtitle sample and test the production phrase/citation pipeline against the
   reference. In particular, handle tokenizer segmentation and reading errors explicitly.

The next implementation can query the reference locally. It should version/invalidate
its derived cache by source hash and keep that cache independent of Anki knowledge.
No dictionary vocabulary tokenization, production matching change, live Anki calls,
real-cache writes, or performance claims were introduced by this study.

## Attribution

Derived dictionary data: © James William Breen and EDRDG (JMdict), and Stephen Kraus
and contributors (Jitendex), under CC BY-SA 4.0. Sources and terms:
[JMdict](https://www.edrdg.org/wiki/JMdict-EDICT_Dictionary_Project.html),
[EDRDG licence](https://www.edrdg.org/edrdg/licence.html),
[Jitendex download](https://jitendex.org/pages/downloads.html),
[Jitendex licence](https://jitendex.org/pages/legal.html).

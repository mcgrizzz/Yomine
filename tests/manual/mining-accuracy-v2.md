# Mining accuracy v2 verification

Work in `/mnt/h/documents/dev/rust/japanese-mining/yomine-mining-accuracy-v2`, branch `feat/mining-accuracy-v2`, based on refreshed main `b363d64`. The original `yomine` worktree and `feat/mining-accuracy` branch remain the regression reference. Do not push or rewrite that branch.

## Matching behavior

- **Known**: an exact normalized surface/card pair, a validated citation, or supported lexical-family evidence establishes the card match. These terms filter normally and contribute their card-based comprehension.
- **Possible**: a plausible card exists but competing or missing lexical evidence prevents establishing identity. The term stays unknown and minable, shows “Possible match: …”, and contributes zero known-word comprehension.
- **Unmatched**: no supported card proposal exists. The term stays unknown without a possible-match label.

“Show possible known matches” defaults to enabled for older settings and persists an explicit opt-out. Toggle it off and back on: only row visibility changes. Known counts, comprehension, labels, and source data must stay unchanged. Removing a card clears its previous possible-match label; adding an exact card or establishing all competing families clears the label and updates comprehension.

Dictionary reading evidence includes disabled dictionaries and marker entries. Reading-less frequency records (including BCCWJ) add no reading candidates. Frequency ranks never establish lexical identity. 行く and 逝く retain separate identities. Documented spelling aliases are constrained to their stated reading.

## Automated checks

Use the same ignored `Cargo.lock` copied from the reference worktree for both this branch and the detached `yomine-mining-accuracy-main-check` worktree. Do not regenerate either dependency lock during the comparison.

```sh
cd /mnt/h/documents/dev/rust/japanese-mining/yomine-mining-accuracy-v2
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine YOMINE_REQUIRE_UNIDIC=1 cargo test --workspace --lib --test segmentation --test phrase_processing --locked -- --include-ignored
cargo +nightly fmt --all -- --check
YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine YOMINE_REQUIRE_UNIDIC=1 cargo clippy --workspace --all-targets --locked
cd src-tauri/ui
COREPACK_ENABLE_AUTO_PIN=0 pnpm check
node tests/possible-matches.mjs
COREPACK_ENABLE_AUTO_PIN=0 pnpm build
```

The card tests construct isolated in-memory snapshots. They do not fetch live Anki or modify the real cache or dictionaries. UniDic is required for segmentation, production matching, phrase snapshots, and the explicitly included Tauri refresh case. Ensure the installed dictionary is present before running commands against the real data directory.

`tests/fixtures/phrase_processing.json` records production and corpus outputs from the fourth commit, before their shared helpers were extracted. The test compares full term data, highlights, expression families, corpus surface forms, and later batch deinflection. Do not regenerate it to accept a refactor regression.

## Manual checks with isolated card snapshots

Use a disposable app profile/data copy for live card-change exercises; never replace the real cache. Every command that reads installed real data must explicitly set `YOMINE_DATA_DIR=/mnt/c/Users/Andrew/AppData/Local/yomine`. Keep the card snapshot isolated and never invoke live Anki refresh against the real cache for these checks.

1. Load 形態化させて. Expect segments 形態 / 化させて, citation 化する, and the full 化させて mining highlight. An enclosing 形態化する expression may coexist with both components.
2. Check 要らないです, 食べないです, 行かないです, and 知らなかった. Expect 要る, 食べる, 行く, and 知る without a newly introduced standalone す. Check lexicalized つまらない and intact おばさん.
3. Check なんとなく promotion with a surface frequency and a card written 何となく while its representative spelling is 何と無く. Verify empty expressions and pronoun なん gain no inferred identity.
4. With only one relevant card, exercise ambiguous こと, くる, いく, わけ, うまい, あと, and はし. Confirm possible labels, zero comprehension, card removal, and later established matches.
5. Save several dictionary weight/enabled/hidden changes together. Expect one complete refreshed file result. Repeat changes while a file is loaded: cleaned text and source metadata remain intact, derived segments do not accumulate, and old work cannot replace a newer result. Change a display preference during refresh and verify refresh still completes.
6. Refresh an ignore list and verify possible matches are excluded from known highlighting and known counts.

Keep UniDic/Vibrato at runtime. Ichiran may be used as a segmentation comparison only. Performance measurements are deferred until a controlled release benchmark uses the same subtitle file, dictionaries, and isolated Anki snapshots; this change makes no speed claim.


## Frequency-supported matching adjustment

This supersedes the earlier policy that frequency never affects matching. Lexical families
still remain separate. A dictionary's reading-specific `㋕` entry explicitly links a kana
form to its written family. For automatic matching, both ranks must be at most 1,000,
within a factor of three, and every competing written family must rank more than ten
times lower in the same source. At least one installed source must supply the complete
comparison; any close competitor in another comparable source vetoes the match.
These are conservative rank heuristics, not calibrated probabilities. Missing and zero
ranks do not establish rarity. Weights and enabled flags do not alter this evidence.

With an isolated card snapshot containing 行く/いく, test a dictionary fixture with
行く rank 44, its ㋕ rank 65, and 逝く rank 9,328: kana いく should now filter and count
as known. A kana いく card should likewise match written 行く, but not 逝く. Remove the
card and confirm the term returns with zero known comprehension and no stale label.
Close ranks, missing alternatives, or conflicting dictionaries should retain uncertainty.

The visibility control now says “Show uncertain matches.” Rows display only “Uncertain”;
the candidate is available in the label tooltip instead of cluttering the term column.
Visibility still changes only which rows are shown, not comprehension or known counts.


## Select the expression before checking Anki

Dictionary interpretation now runs independently of card contents. A validated written
citation takes priority; otherwise a unique lexical family or the existing linked-kana
frequency heuristic may select an expression. Anki then checks that selected family.
A card for a rejected homophone no longer produces an uncertain-match label. A kana
card must independently select the same family to match a written expression.

Verify 行く/いく with no cards, only 逝く, only 行く, and both cards in either order.
The selected family must remain 行く in every case. Only the matching card should
establish knowledge. With only 逝く, いく stays minable without a misleading label.
A validated citation 逝く must remain 逝く even when the source is kana and 行く ranks
higher. Unresolved dictionary interpretations retain the existing uncertainty behavior.

This adopts Yomitan's expression-before-duplicate-check ordering. It uses Yomine's
installed lexical evidence and does not claim full Yomitan dictionary lookup parity.

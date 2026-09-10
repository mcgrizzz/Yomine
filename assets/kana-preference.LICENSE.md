# Kana preference data attribution

`kana-preference.bin` is adapted from JMdict (EDRDG / James William Breen)
and Jitendex (Stephen Kraus and contributors). This adapted data is distributed
under [Creative Commons Attribution-ShareAlike 4.0 International](https://creativecommons.org/licenses/by-sa/4.0/).

Sources and notices:
- https://www.edrdg.org/edrdg/licence.html
- https://jitendex.org/pages/legal.html

Changes: selected preferred identities per reading and POS using legal dictionary
relationships and pinned frequency-source kana links; excluded unresolved, shared,
search-only, rare, obsolete and no-kanji target forms; compiled a sorted binary index.
Frequency-source bank checksums are listed in the manifest. Neither definitions nor
source frequency ranks are bundled.
Exact versions, source URLs and checksums are in `kana-preference.json`.
The reproducible generator and its source schema are in `research/lexical`.

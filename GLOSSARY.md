# Yomine mining

Yomine turns Japanese text (subtitles, books) into candidate vocabulary cards for Anki.

## Language

**Term**:
What the engine extracts from a sentence as a possible card: a single word (者) or a phrase
(気が引ける), with its dictionary form and reading.
_Avoid_: word, vocab, entry

**Card**:
A note in Anki.
_Avoid_: term (for something already in Anki)

**Word**:
One of the engine's internal pieces of a sentence before it becomes a term.
_Avoid_: token (UniDic's smaller unit), term

**Derived form**:
A verb one derivation step from its base verb (potential 読める, causative 輝かせる, passive
生まれる) that JMdict lists as a word of its own.
_Avoid_: conjugation, inflection

**Loanword**:
A term written in katakana whose every piece comes from a foreign language (ゲーム, ペットボトル).
Native words written in katakana (クマ, ダメ) and onomatopoeia aren't loanwords. Its source is
English, made-in-Japan English (サラリーマン), or another language (アルバイト, パン).
_Avoid_: gairaigo, katakana word

**Mining option**:
A user setting that changes which terms the engine produces, or which terms auto mode takes.
_Avoid_: advanced setting, toggle

**Skip reason**:
Why auto mode passes over a term: grammar, kanji fragment, speaker name, or a spelling already
in Anki.
_Avoid_: filter (the term table's filters are separate)

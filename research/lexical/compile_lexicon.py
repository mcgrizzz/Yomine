#!/usr/bin/env python3
"""Compile JMdict phrase forms, kanji headwords and per-entry readings into a directly
searchable table."""
import argparse
import collections
import hashlib
import json
import re
import sqlite3
import struct
import unicodedata
from pathlib import Path

from build import normalize
from compile_runtime import encode, normalize_long_vowel
from source_policy import source_manifest

MAGIC = b"LEXIDX01"
PHRASE_POS = {"exp", "adv", "conj", "prt"}
# Nouns, verbs and i-/na-adjectives; pre-noun adjectives (こういう), interjections and the
# copula would take the type of their last piece when joined.
CONTENT_POS = re.compile(r"(n|n-.*|v.*|adj-i|adj-ix|adj-na|adj-no)")
# A word's type code (jmdict_lexicon.rs reads them) and the JMdict tags that give it, best
# first. vs is left out: a noun that takes する is still a noun (お迎え).
WORD_TYPES = [("v", r"v[1245knrz].*|vs-[ics]"), ("i", r"adj-ix?"), ("a", r"adj-na"),
              ("n", r"n|n-.*"), ("o", r"pn"), ("d", r"adv.*"), ("c", r"conj"), ("j", r"int"),
              ("p", r"adj-pn")]
# Rarely used, search-only and outdated kanji forms.
RARE_KANJI = {"rK", "sK", "oK"}


def has_kanji(text):
    return any("㐀" <= c <= "鿿" or c == "々" for c in text)


def word_type(pos):
    """An expression stays one (ことになる is exp and v5r); otherwise the best type the tags give."""
    if "exp" in pos:
        return "e"
    return next((code for code, tags in WORD_TYPES if any(re.fullmatch(tags, p) for p in pos)), None)


def best_type(codes):
    order = [code for code, _ in WORD_TYPES] + ["e"]
    return min((c for c in codes if c), key=order.index, default=None)


def compile_rows(db):
    """`p\\tform`: a JMdict expression, adverb, conjunction or particle, valued by how many
    senses its entries give it (one byte, capped at 255), then its type code.
    `w\\tspelling`: a JMdict spelling with kanji, of any part of speech (二日酔い), and the
    kana reading of a content word JMdict says is usually written in kana (もったいない) or
    spells only with rare kanji (こたび, 此度), unless it is an expression. Other
    kana-only forms are left out: a run of kana tokens spells some JMdict reading too often
    (いいか). Valued by its type code.
    A form's type code is `word_type` of each entry's first sense that applies to it, the best
    across entries; empty when no tag gives one.
    `a\\treading`: a JMdict i-adjective ending in ない, which UniDic reads as a verb and ない
    (つまらない as つまる).
    `e\\tspelling\\treading`: entries listing the spelling under that reading, kept only
    for spellings an entry reads more than one way (明日 as あした and あす)."""
    # A pair has one row per sense that applies to it.
    pair_senses = collections.Counter()
    phrase_pairs = set()
    readings = collections.defaultdict(set)
    words = set()
    nai_adjectives = set()
    first_sense = {}
    rare_kanji_only = {}
    for entry, reading, spelling, pos, misc, spelling_info in db.execute(
            "SELECT entry_id,reading,spelling,pos,misc,spelling_info FROM pairs ORDER BY rowid"):
        reading = normalize_long_vowel(reading)
        # Unfolded, so に+カット can't pass for にかっと.
        pair = (entry, reading, normalize_long_vowel(unicodedata.normalize("NFKC", spelling)))
        pair_senses[pair] += 1
        usually_kana = "uk" in json.loads(misc)
        # How directly the entry writes the form: as spelled, as the reading of a word
        # usually written in kana, or only as a reading (どうだい for 同大).
        for form, how in {(pair[2], 0), (reading, 0 if reading == pair[2] else 1 if usually_kana else 2)}:
            first_sense.setdefault((form, entry, how), word_type(json.loads(pos)))
        if has_kanji(pair[2]):
            words.add(pair[2])
        content = any(CONTENT_POS.fullmatch(p) for p in json.loads(pos))
        if usually_kana and content and len(reading) >= 3:
            words.add(reading)
        if content and "exp" not in json.loads(pos) and has_kanji(pair[2]) and len(reading) >= 3:
            rare = bool(set(json.loads(spelling_info)) & RARE_KANJI)
            rare_kanji_only[(entry, reading)] = rare_kanji_only.get((entry, reading), True) and rare
        if set(json.loads(pos)) & PHRASE_POS:
            phrase_pairs.add(pair)
        if "adj-i" in json.loads(pos) and reading.endswith("ない"):
            nai_adjectives.add(reading)
        spelling = normalize(spelling)
        if has_kanji(spelling):
            readings[(spelling, entry)].add(reading)
    words.update(reading for (_, reading), rare in rare_kanji_only.items() if rare)
    entry_senses = collections.defaultdict(int)
    for pair in phrase_pairs:
        entry, reading, spelling = pair
        for form in {reading, spelling}:
            entry_senses[(form, entry)] = max(entry_senses[(form, entry)], pair_senses[pair])
    phrases = collections.Counter()
    for (form, _), senses in entry_senses.items():
        phrases[form] += senses
    entries = collections.defaultdict(set)
    for (spelling, entry), variants in readings.items():
        if len(variants) > 1:
            for reading in variants:
                entries[(spelling, reading)].add(entry)
    # The entries that write a form most directly decide its type: はいはい is an interjection,
    # though 這い這い (crawling) is also read はいはい.
    types = collections.defaultdict(lambda: collections.defaultdict(set))
    for (form, _, how), code in first_sense.items():
        types[form][how].add(code)
    type_code = lambda form: (best_type(types[form][min(types[form])]) or "").encode()
    rows = [(("p\t" + form).encode(), bytes([min(senses, 255)]) + type_code(form))
            for form, senses in phrases.items()]
    rows += [(("w\t" + form).encode(), type_code(form)) for form in words]
    rows += [(("a\t" + reading).encode(), b"") for reading in nai_adjectives]
    rows += [(f"e\t{spelling}\t{reading}".encode(), b"".join(struct.pack("<I", e) for e in sorted(ids)))
             for (spelling, reading), ids in entries.items()]
    return sorted(rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    db = sqlite3.connect(args.db.resolve().as_uri() + "?mode=ro", uri=True)
    rows = compile_rows(db)
    data = encode(rows, MAGIC)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(data)
    source = source_manifest(db)
    manifest = {"format": MAGIC.decode(), "records": len(rows), "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
                "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "jmdict": {k: source["jmdict"][k] for k in ["url", "sha256", "created"]},
                "phrase_pos": sorted(PHRASE_POS),
                "license": "CC-BY-SA-4.0", "attribution": source["attribution"]}
    args.output.with_suffix(".json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({k: manifest[k] for k in ["records", "bytes", "sha256"]}, indent=2))


if __name__ == "__main__":
    main()

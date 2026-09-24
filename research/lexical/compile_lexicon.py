#!/usr/bin/env python3
"""Compile JMdict phrase forms and per-entry readings into a directly searchable table."""
import argparse
import collections
import hashlib
import json
import sqlite3
import struct
import unicodedata
from pathlib import Path

from build import normalize
from compile_runtime import encode, normalize_long_vowel
from source_policy import source_manifest

MAGIC = b"LEXIDX01"
PHRASE_POS = {"exp", "adv", "conj", "prt"}


def has_kanji(text):
    return any("㐀" <= c <= "鿿" or c == "々" for c in text)


def compile_rows(db):
    """`p\\tform`: a JMdict expression, adverb, conjunction or particle.
    `e\\tspelling\\treading`: entries listing the spelling under that reading, kept only
    for spellings an entry reads more than one way (明日 as あした and あす)."""
    phrases = set()
    readings = collections.defaultdict(set)
    for entry, reading, spelling, pos in db.execute("SELECT entry_id,reading,spelling,pos FROM pairs"):
        reading = normalize_long_vowel(reading)
        if set(json.loads(pos)) & PHRASE_POS:
            # Unfolded, so に+カット can't pass for にかっと.
            phrases.add(normalize_long_vowel(unicodedata.normalize("NFKC", spelling)))
            phrases.add(reading)
        spelling = normalize(spelling)
        if has_kanji(spelling):
            readings[(spelling, entry)].add(reading)
    entries = collections.defaultdict(set)
    for (spelling, entry), variants in readings.items():
        if len(variants) > 1:
            for reading in variants:
                entries[(spelling, reading)].add(entry)
    rows = [(("p\t" + form).encode(), b"") for form in phrases]
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

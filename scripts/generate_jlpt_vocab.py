#!/usr/bin/env python3
"""
~~~VIBE CODED SCRIPT~~~

Generate assets/jlpt_vocab.json from the per-level word lists, attaching a primary POS.

The per-level CSVs (assets/n5.csv .. n1.csv, columns: jmdict_seq, kana, kanji,
waller_definition) are the source of truth for *which* words belong to *which* level. They are
read-only here. For each row we find its JMdict entry — by jmdict_seq when present, otherwise by
kanji/kana — and map its primary sense's part-of-speech onto yomine's POS enum variant names
(see src/segmentation/word.rs), writing the result to assets/jlpt_vocab.json.

Usage:
    python scripts/generate_jlpt_vocab.py --jmdict path/to/jmdict-eng-3.x.x.json

Download the JMdict data (jmdict-eng-*.json) from:
    https://github.com/scriptin/jmdict-simplified/releases
"""

import argparse
import csv
import json
import sys
from pathlib import Path

# Repo paths relative to this script (scripts/ -> repo root -> assets/).
REPO_ROOT = Path(__file__).resolve().parent.parent
ASSETS = REPO_ROOT / "assets"
JLPT_PATH = ASSETS / "jlpt_vocab.json"  # generated output

# Per-level source word lists, easiest first (this is also the output order).
LEVELS = ["N5", "N4", "N3", "N2", "N1"]

# Fallback POS for entries with no JMdict match or no mappable part-of-speech. Most JLPT
# vocabulary are plain nouns, so this is the least-surprising default; unmatched entries are
# reported so they can be hand-corrected.
DEFAULT_POS = "Noun"

# Exact JMdict pos-code -> yomine POS enum variant name. Verb classes (v1, v5*, vk, ...) are
# handled by prefix in map_code(); transitivity-only markers (vi/vt) are skipped there.
POS_MAP = {
    "n": "Noun",
    "adj-no": "Noun",
    "pn": "Pronoun",
    "adj-i": "Adjective",
    "adj-ix": "Adjective",
    "adj-ku": "Adjective",
    "adj-shiku": "Adjective",
    "adj-nari": "Adjective",
    "adj-t": "Adjective",
    "adj-f": "Adjective",
    "adj-na": "AdjectivalNoun",
    "adj-pn": "Determiner",
    "adv": "Adverb",
    "adv-to": "Adverb",
    "vs": "SuruVerb",
    "vs-i": "SuruVerb",
    "vs-s": "SuruVerb",
    "vs-c": "SuruVerb",
    "cop": "Copula",
    "cop-da": "Copula",
    "prt": "Postposition",
    "aux": "Postposition",
    "aux-v": "Postposition",
    "aux-adj": "Postposition",
    "conj": "Conjunction",
    "int": "Interjection",
    "ctr": "Counter",
    "num": "Number",
    "pref": "Prefix",
    "suf": "Suffix",
    "exp": "Expression",
    "unc": "Unknown",
}


def map_code(code):
    """Map a single JMdict pos code to a yomine POS variant, or None if not mappable."""
    if code in ("vi", "vt"):  # transitivity markers, never the primary POS on their own
        return None
    if code in POS_MAP:
        return POS_MAP[code]
    if code.startswith("v"):  # remaining verb classes: v1, v1-s, v5*, v2*, v4*, vk, vz, vn, vr
        return "Verb"
    if code.startswith("n"):  # n-suf, n-pref, n-t, n-adv — noun at the core ("num" handled above)
        return "Noun"
    return None


def primary_pos(word):
    """First mappable POS across the word's senses (in JMdict order), or None."""
    for sense in word.get("sense", []):
        for code in sense.get("partOfSpeech", []):
            mapped = map_code(code)
            if mapped is not None:
                return mapped
    return None


def build_indexes(words):
    """id, (kanji, kana), kanji, and kana lookups (first occurrence wins)."""
    by_id = {}
    by_kanji_kana = {}
    by_kanji = {}
    by_kana = {}
    for word in words:
        by_id.setdefault(word["id"], word)
        kana_texts = [k["text"] for k in word.get("kana", [])]
        kanji_texts = [k["text"] for k in word.get("kanji", [])]
        for kanji in kanji_texts:
            by_kanji.setdefault(kanji, word)
        for kana in kana_texts:
            by_kana.setdefault(kana, word)
            for kanji in kanji_texts:
                by_kanji_kana.setdefault((kanji, kana), word)
    return by_id, by_kanji_kana, by_kanji, by_kana


def match_word(entry, by_kanji_kana, by_kanji, by_kana):
    """Find the JMdict word for a JLPT entry, or None."""
    kanji = entry["kanji"]
    kana = entry["kana"]
    if kanji:
        # Prefer an exact kanji+reading match; fall back to the kanji alone, since some entries
        # list an on-reading (割 → かつ) that differs from JMdict's headword reading (割 → わり).
        return by_kanji_kana.get((kanji, kana)) or by_kanji.get(kanji)
    # Kana-only entry: match on the reading alone.
    return by_kana.get(kana)


def read_csv_entries():
    """All JLPT entries from the read-only per-level CSVs, in N5 -> N1 order."""
    entries = []
    for level in LEVELS:
        path = ASSETS / f"{level.lower()}.csv"
        with path.open(encoding="utf-8", newline="") as f:
            for row in csv.DictReader(f):
                entries.append(
                    {
                        "seq": (row["jmdict_seq"] or "").strip(),
                        "kana": (row["kana"] or "").strip(),
                        "kanji": (row["kanji"] or "").strip(),
                        "level": level,
                    }
                )
    return entries


def write_jlpt(entries):
    """Write entries one-per-line, preserving the existing compact array layout."""
    lines = [json.dumps(e, ensure_ascii=False, separators=(",", ":")) for e in entries]
    JLPT_PATH.write_text("[\n" + ",\n".join(lines) + "\n]\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--jmdict", required=True, help="Path to a jmdict-simplified jmdict-eng-*.json file")
    args = parser.parse_args()

    entries = read_csv_entries()
    jmdict = json.loads(Path(args.jmdict).read_text(encoding="utf-8"))
    by_id, by_kanji_kana, by_kanji, by_kana = build_indexes(jmdict["words"])

    enriched = []
    dropped = []
    unmatched = []
    for entry in entries:
        # Prefer the exact JMdict sequence id from the CSV; fall back to kanji/kana when a row
        # has no seq (some single-kanji N1 entries).
        word = by_id.get(entry["seq"]) if entry["seq"] else None
        if word is None:
            word = match_word(entry, by_kanji_kana, by_kanji, by_kana)
        pos = primary_pos(word) if word else None
        # Single-kanji entries with no standalone JMdict word (e.g. 依/い, 働/どう) are kanji
        # readings / bound morphemes, not vocabulary — drop them rather than guess a POS.
        if word is None and len(entry["kanji"]) == 1:
            dropped.append(entry)
            continue
        if pos is None:
            unmatched.append(entry)
            pos = DEFAULT_POS
        # Fixed key order: kanji, kana, level, pos.
        enriched.append(
            {"kanji": entry["kanji"], "kana": entry["kana"], "level": entry["level"], "pos": pos}
        )

    write_jlpt(enriched)

    print(f"Enriched {len(enriched)} entries -> {JLPT_PATH.relative_to(REPO_ROOT)}")
    if dropped:
        print(f"Dropped {len(dropped)} single-kanji entries with no standalone JMdict word:")
        for entry in dropped:
            print(f"  {entry['kanji']} ({entry['kana']}) [{entry['level']}]")
    if unmatched:
        print(
            f"{len(unmatched)} entries had no JMdict match / mappable POS "
            f"(defaulted to {DEFAULT_POS}):",
            file=sys.stderr,
        )
        for entry in unmatched[:50]:
            print(f"  {entry['kana']} ({entry['kanji'] or '—'}) [{entry['level']}]", file=sys.stderr)
        if len(unmatched) > 50:
            print(f"  ... and {len(unmatched) - 50} more", file=sys.stderr)


if __name__ == "__main__":
    main()

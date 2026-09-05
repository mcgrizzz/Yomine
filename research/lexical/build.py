#!/usr/bin/env python3
"""Build an offline research reference; never writes to installed dictionary data."""
import argparse
import gzip
import hashlib
import io
import json
import re
import sqlite3
import unicodedata
import xml.etree.ElementTree as ET
import zipfile
from urllib.parse import parse_qs, urlsplit
from pathlib import Path

SCHEMA_VERSION = 2
SOURCE_URLS = {
    "jmdict": "https://www.edrdg.org/pub/Nihongo/JMdict_e.gz",
    "jitendex": "https://github.com/stephenmk/stephenmk.github.io/releases/latest/download/jitendex-yomitan.zip",
}


def normalize(text):
    # NFKC and katakana folding only: no prolonged-vowel guesses or homophone merging.
    text = unicodedata.normalize("NFKC", text)
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in text)


def packed(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def values(node, tag):
    return [x.text or "" for x in node.findall(tag)]


def entry_record(entry):
    spellings = [{"text": k.findtext("keb"), "info": values(k, "ke_inf"),
                  "priority": values(k, "ke_pri")} for k in entry.findall("k_ele")]
    readings = [{"text": r.findtext("reb"), "restrictions": values(r, "re_restr"),
                 "no_kanji": r.find("re_nokanji") is not None,
                 "info": values(r, "re_inf"), "priority": values(r, "re_pri")}
                for r in entry.findall("r_ele")]
    senses = []
    inherited_pos = []
    for number, sense in enumerate(entry.findall("sense"), 1):
        explicit_pos = values(sense, "pos")
        inherited_pos = explicit_pos or inherited_pos
        senses.append({"number": number, "pos": inherited_pos, "pos_explicit": explicit_pos,
                       "spellings": values(sense, "stagk"), "readings": values(sense, "stagr"),
                       "misc": values(sense, "misc"), "field": values(sense, "field"),
                       "dialect": values(sense, "dial"), "notes": values(sense, "s_inf"),
                       "cross_references": values(sense, "xref")})
    return {"id": int(entry.findtext("ent_seq")), "spellings": spellings,
            "readings": readings, "senses": senses}


def legal_pairs(record):
    """Keep entry, reading, spelling AND sense restrictions; never form global aliases."""
    for reading in record["readings"]:
        candidates = [k for k in record["spellings"]
                      if not reading["restrictions"] or k["text"] in reading["restrictions"]]
        if reading["no_kanji"] or not record["spellings"]:
            candidates = [{"text": reading["text"], "info": [], "priority": []}]
        for spelling in candidates:
            for sense in record["senses"]:
                if sense["readings"] and reading["text"] not in sense["readings"]:
                    continue
                if sense["spellings"] and spelling["text"] not in sense["spellings"]:
                    continue
                yield (record["id"], normalize(reading["text"]), spelling["text"],
                       sense["number"], packed(sense["pos"]), packed(sense["misc"]),
                       int("uk" in sense["misc"]), int(reading["no_kanji"]),
                       packed(spelling["info"]), packed(reading["info"]),
                       packed(sorted(set(spelling["priority"] + reading["priority"]))))


def parse_jmdict(path):
    raw = gzip.decompress(path.read_bytes())
    header = raw[:raw.index(b"<entry>")]
    entities = dict(re.findall(rb'<!ENTITY\s+([\w-]+)\s+"([^"]*)"\s*>', header))
    # Keep stable entity codes instead of translated descriptions. No external entities.
    raw = re.sub(rb'<!ENTITY\s+([\w-]+)\s+"[^"]*"\s*>',
                 lambda m: b'<!ENTITY ' + m[1] + b' "' + m[1] + b'">', raw)
    date = re.search(rb"JMdict created: ([0-9-]+)", header)
    metadata = {"created": date[1].decode() if date else None,
                "entities": {k.decode(): v.decode() for k, v in entities.items()}}
    def records():
        events = ET.iterparse(io.BytesIO(raw), events=("start", "end"))
        _, root = next(events)
        for event, node in events:
            if event == "end" and node.tag == "entry":
                yield entry_record(node)
                root.clear()
    return metadata, records()


def kana_paths(value, path=""):
    """Record exact metadata paths; do not spread a sense-local tag to every sense."""
    if isinstance(value, dict):
        if value.get("title") == "word usually written using kana alone":
            yield path
        for key, child in value.items():
            yield from kana_paths(child, path + "/" + key)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from kana_paths(child, path + "/" + str(index))


def lexical_structure(value):
    """Preserve Jitendex sense-group inheritance without copying glosses/examples."""
    if isinstance(value, list):
        return [item for child in value for item in lexical_structure(child)]
    if not isinstance(value, dict):
        return []
    data = value.get("data", {})
    kind = data.get("content")
    if kind in {"glossary", "extra-info", "attribution", "forms-table"}:
        return []
    if kind in {"misc-info", "part-of-speech-info", "field-info", "dialect-info"}:
        return [{"kind": kind, "code": data.get("code"), "title": value.get("title")}]
    if kind == "redirect-glossary":
        def links(node):
            if isinstance(node, dict):
                if node.get("tag") == "a" and node.get("href", "").startswith("?query="):
                    query = parse_qs(urlsplit(node["href"]).query)
                    yield {"kind": "redirect", "target": query.get("query", [None])[0],
                           "reading": query.get("primary_reading", [None])[0]}
                for child in node.values():
                    yield from links(child)
            elif isinstance(node, list):
                for child in node:
                    yield from links(child)
        return list(links(value))
    children = lexical_structure(value.get("content", []))
    if kind in {"sense-groups", "sense-group", "sense"}:
        return [{"kind": kind, "children": children}]
    return children


def manifest_file(path, url=None):
    return {"file": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "bytes": path.stat().st_size, "url": url}


def build(jmdict, jitendex, frequency_dir, output):
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite {output}; use a new output path")
    output.parent.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(output)
    db.executescript("""
    CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE entries(id INTEGER PRIMARY KEY, data TEXT NOT NULL);
    CREATE TABLE pairs(entry_id INTEGER, reading TEXT, spelling TEXT, sense INTEGER,
        pos TEXT, misc TEXT, kana_preferred INTEGER, no_kanji INTEGER,
        spelling_info TEXT, reading_info TEXT, priority TEXT);
    CREATE TABLE jitendex(sequence INTEGER, term TEXT, reading TEXT, definition_tags TEXT,
        word_classes TEXT, score REAL, kana_paths TEXT, sense_metadata TEXT, source_bank TEXT, source_row INTEGER);
    CREATE TABLE frequencies(dictionary TEXT, term TEXT, reading TEXT, rank INTEGER,
        kana_marker INTEGER, source_bank TEXT, source_row INTEGER);
    """)
    metadata, records = parse_jmdict(jmdict)
    manifest = {"schema_version": SCHEMA_VERSION, "builder_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), "jmdict": manifest_file(jmdict, SOURCE_URLS["jmdict"]),
                "jitendex": manifest_file(jitendex, SOURCE_URLS["jitendex"]), "frequency_sources": [],
                "license": "CC-BY-SA-4.0", "attribution": "JMdict: EDRDG / James William Breen; Jitendex: Stephen Kraus and contributors",
                "license_urls": ["https://www.edrdg.org/edrdg/licence.html", "https://jitendex.org/pages/legal.html"]}
    manifest["jmdict"].update(metadata)
    count = 0
    for record in records:
        db.execute("INSERT INTO entries VALUES (?,?)", (record["id"], packed(record)))
        db.executemany("INSERT INTO pairs VALUES (?,?,?,?,?,?,?,?,?,?,?)", legal_pairs(record))
        count += 1
    db.commit()
    print(f"JMdict: {count} entries", flush=True)
    with zipfile.ZipFile(jitendex) as archive:
        manifest["jitendex"]["index"] = json.loads(archive.read("index.json"))
        for bank in sorted(archive.namelist()):
            if not re.fullmatch(r"term_bank_\d+\.json", bank):
                continue
            batch = []
            for number, row in enumerate(json.loads(archive.read(bank))):
                if len(row) != 8:
                    raise ValueError(f"Unexpected Jitendex term format: {bank}:{number}")
                term, reading, tags, classes, score, glossary, sequence, _ = row
                batch.append((sequence, term, normalize(reading), tags, classes, score,
                              packed(list(kana_paths(glossary))), packed(lexical_structure(glossary)), bank, number))
            db.executemany("INSERT INTO jitendex VALUES (?,?,?,?,?,?,?,?,?,?)", batch)
        db.commit()
    print("Jitendex: complete export indexed", flush=True)
    for directory in sorted(frequency_dir.iterdir()):
        if not directory.is_dir():
            continue
        source = {"dictionary": directory.name, "banks": []}
        for bank in sorted(directory.glob("term_meta_bank_*.json")):
            source["banks"].append(manifest_file(bank))
            rows = []
            for number, (term, kind, data) in enumerate(json.loads(bank.read_text(encoding="utf-8"))):
                if kind != "freq" or data is None:
                    continue
                reading = normalize(data["reading"]) if isinstance(data, dict) and "reading" in data else None
                frequency = data.get("frequency", data) if isinstance(data, dict) else data
                rank = frequency.get("value") if isinstance(frequency, dict) else frequency
                display = frequency.get("displayValue") or "" if isinstance(frequency, dict) else ""
                try:
                    rank = int(rank)
                except (ValueError, TypeError):
                    raise ValueError(f"Non-numeric frequency: {bank}:{number}") from None
                rows.append((directory.name, term, reading, rank, int("㋕" in str(display)), bank.name, number))
            db.executemany("INSERT INTO frequencies VALUES (?,?,?,?,?,?,?)", rows)
        if source["banks"]:
            manifest["frequency_sources"].append(source)
        db.commit()
        print(f"Frequency: {directory.name}", flush=True)
    db.executescript("""
    CREATE INDEX pairs_reading ON pairs(reading, entry_id);
    CREATE INDEX pairs_spelling ON pairs(spelling, reading);
    CREATE INDEX jitendex_pair ON jitendex(sequence, reading, term);
    CREATE INDEX frequency_pair ON frequencies(term, reading, dictionary);
    CREATE INDEX frequency_reading ON frequencies(reading);
    """)
    db.execute("INSERT INTO metadata VALUES ('manifest',?)", (packed(manifest),))
    db.commit()
    assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
    db.close()
    output.with_suffix(".manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    print(f"Reference: {output} ({output.stat().st_size:,} bytes)", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jmdict", type=Path, required=True)
    parser.add_argument("--jitendex", type=Path, required=True)
    parser.add_argument("--frequency-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    build(args.jmdict, args.jitendex, args.frequency_dir, args.output)


if __name__ == "__main__":
    main()

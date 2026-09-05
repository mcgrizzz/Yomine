#!/usr/bin/env python3
"""Compile the verified lexical study into a sorted, directly searchable binary table."""
import argparse
import collections
import hashlib
import json
import sqlite3
import struct
from pathlib import Path
from build import normalize

# Stable format bits, deliberately independent of Rust enum discriminants.
POS_BITS = {"n":1, "n-adv":1, "v1":2, "vs":2, "vs-i":2, "vs-s":2,
            "vk":2, "vz":2, "adj-i":4, "adj-ix":4, "adj-na":8, "adv":16, "adv-to":16}


def pos_mask(codes):
    return sum(set(2 if code.startswith("v5") else POS_BITS.get(code, 0) for code in codes))


def compile_rows(db):
    # Count preferred identities BEFORE applying Jitendex coverage or spelling filters.
    # Missing coverage must not turn two competing identities into a unique one.
    unique = {r for r, in db.execute("SELECT reading FROM pairs WHERE kana_preferred=1 GROUP BY reading HAVING count(DISTINCT entry_id)=1")}
    supported = set(db.execute("""SELECT DISTINCT j.sequence,j.reading FROM jitendex j
        WHERE j.kana_paths!='[]' AND EXISTS(SELECT 1 FROM pairs p WHERE p.entry_id=j.sequence
        AND p.reading=j.reading AND p.spelling=j.term AND p.kana_preferred=1)"""))
    senses = collections.defaultdict(list)
    for entry, reading, spelling, sense, pos, ki, ri, no_kanji in db.execute("SELECT entry_id,reading,spelling,sense,pos,spelling_info,reading_info,no_kanji FROM pairs WHERE kana_preferred=1"):
        if reading not in unique or (entry,reading) not in supported or no_kanji:
            continue
        if set(json.loads(ki)) & {"sK", "rK", "oK"} or set(json.loads(ri)) & {"sk", "rk", "ok", "gikun"}:
            continue
        mask = pos_mask(json.loads(pos))
        if mask:
            senses[(entry,reading,sense)].append((normalize(spelling), mask))
    rows = {}
    for (entry,reading,_), forms in senses.items():
        for lexeme, _ in forms:
            key = (reading + "\t" + lexeme).encode()
            identity, targets = rows.setdefault(key, (entry, {}))
            assert identity == entry
            for spelling, mask in forms:
                targets[spelling] = targets.get(spelling,0) | mask
    result = []
    for key,(identity,targets) in sorted(rows.items()):
        value = bytearray(struct.pack("<IH",identity,len(targets)))
        for spelling,mask in sorted(targets.items()):
            raw=spelling.encode()
            value.extend(struct.pack("<IH",mask,len(raw)))
            value.extend(raw)
        result.append((key,bytes(value)))
    return result


def encode(rows):
    offset=12+16*len(rows)
    directory=bytearray()
    payload=bytearray()
    for key,value in rows:
        directory.extend(struct.pack("<IIII",offset,len(key),offset+len(key),len(value)))
        payload.extend(key);payload.extend(value)
        offset+=len(key)+len(value)
    return b"KANAIDX1"+struct.pack("<I",len(rows))+directory+payload


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args()
    db=sqlite3.connect(args.db.resolve().as_uri()+"?mode=ro",uri=True)
    rows=compile_rows(db)
    data=encode(rows)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_bytes(data)
    source=json.loads(db.execute("SELECT value FROM metadata WHERE key='manifest'").fetchone()[0])
    manifest={"format":"KANAIDX1","records":len(rows),"bytes":len(data),
              "sha256":hashlib.sha256(data).hexdigest(),
              "generator_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "jmdict":{k:source["jmdict"][k] for k in ["url","sha256","created"]},
              "jitendex":{k:source["jitendex"][k] for k in ["url","sha256"]},
              "jitendex_revision":source["jitendex"]["index"]["revision"],
              "license":"CC-BY-SA-4.0","attribution":source["attribution"]}
    args.output.with_suffix(".json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n")
    print(json.dumps(manifest,ensure_ascii=False,indent=2))


if __name__ == "__main__":
    main()

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
from source_policy import prepare_frequencies, source_manifest, policy_manifest

# Stable format bits, deliberately independent of Rust enum discriminants.
POS_BITS = {"n":1, "n-adv":1, "v1":2, "vs":2, "vs-i":2, "vs-s":2,
            "vk":2, "vz":2, "adj-i":4, "adj-ix":4, "adj-na":8, "adv":16, "adv-to":16}


def pos_mask(codes):
    return sum(set(2 if code.startswith("v5") else POS_BITS.get(code, 0) for code in codes))


def choose_preferred(candidates, ranks, linked):
    """Ranks select an existing identity, never create or merge one. No probabilities."""
    winners = []
    for target in candidates:
        if not linked.get(target):
            continue
        rivals = set(candidates) - {target}
        covered = set()
        supported = False
        contradicted = False
        for source, scores in ranks.items():
            if target not in scores:
                continue
            target_rank = scores[target]
            comparisons = {r: scores[r] for r in rivals if r in scores}
            covered.update(r for r, rank in comparisons.items() if rank >= 10 * target_rank)
            if source in linked[target]:
                contradicted |= any(rank <= target_rank for rank in comparisons.values())
                supported |= bool(comparisons) and all(rank >= 10 * target_rank for rank in comparisons.values())
        if not rivals:
            supported = True
        if supported and not contradicted and covered == rivals:
            winners.append(target)
    return winners[0] if len(winners) == 1 else None


def compile_rows(db):
    prepare_frequencies(db)
    readings = {r for r, in db.execute("SELECT DISTINCT reading FROM research_frequencies WHERE kana_marker=1 AND reading IS NOT NULL")}
    identities = collections.defaultdict(set)
    groups = collections.defaultdict(lambda: collections.defaultdict(dict))
    for entry, reading, spelling, pos, ki, ri, no_kanji in db.execute("SELECT entry_id,reading,spelling,pos,spelling_info,reading_info,no_kanji FROM pairs"):
        if reading not in readings:
            continue
        spelling = normalize(spelling)
        identities[(reading, spelling)].add(entry)
        mask = pos_mask(json.loads(pos))
        usable = not no_kanji and not (set(json.loads(ki)) & {"sK", "rK", "oK"} or set(json.loads(ri)) & {"sk", "rk", "ok", "gikun"})
        for bit in (1, 2, 4, 8, 16):
            if mask & bit:
                forms = groups[(reading, bit)][entry]
                if usable:
                    forms[spelling] = bit
    supported = set(db.execute("SELECT DISTINCT sequence,reading FROM jitendex WHERE sequence>0"))
    frequency = collections.defaultdict(list)
    for source, term, reading, rank, marker in db.execute("SELECT dictionary,term,reading,rank,kana_marker FROM research_frequencies WHERE reading IS NOT NULL AND rank>0"):
        if reading in readings:
            term = normalize(term)
            # A lone kana frequency cannot identify which homophone it describes.
            if term != reading:
                frequency[reading].append((source, term, rank, marker))
    result = []
    for (reading, bit), candidates in sorted(groups.items()):
        written = collections.defaultdict(dict)
        markers = collections.defaultdict(set)
        for source, term, rank, marker in frequency[reading]:
            owners = identities.get((reading, term), {"unresolved:" + term})
            if marker:
                markers[(source, rank)].update(owners)
            else:
                for owner in owners:
                    written[source][owner] = min(written[source].get(owner, rank), rank)
        linked = collections.defaultdict(set)
        ranks = {source: dict(scores) for source, scores in written.items()}
        for (source, rank), owners in markers.items():
            for owner in owners:
                linked[owner].add(source)
            # Repeated ranks under distinct JMdict entries provide no identity signal.
            if len(owners) == 1:
                owner = next(iter(owners))
                scores = ranks.setdefault(source, {})
                scores[owner] = min(scores.get(owner, rank), rank)
        rivals = set(candidates)
        rivals.update(owner for scores in ranks.values() for owner in scores if isinstance(owner, str))
        rivals.update(owner for owner in linked if isinstance(owner, str))
        selected = choose_preferred(rivals, ranks, linked)
        if selected not in candidates or not candidates[selected] or (selected, reading) not in supported:
            continue
        targets = {s: mask for s, mask in candidates[selected].items()
                   if identities[(reading, s)] == {selected}}
        if not targets:
            continue
        value = bytearray(struct.pack("<IH", selected, len(targets)))
        for spelling, mask in sorted(targets.items()):
            raw = spelling.encode()
            value.extend(struct.pack("<IH", mask, len(raw)))
            value.extend(raw)
        result.append(((reading + "\t" + str(bit)).encode(), bytes(value)))
    return sorted(result)


def encode(rows):
    offset=12+16*len(rows)
    directory=bytearray()
    payload=bytearray()
    for key,value in rows:
        directory.extend(struct.pack("<IIII",offset,len(key),offset+len(key),len(value)))
        payload.extend(key);payload.extend(value)
        offset+=len(key)+len(value)
    return b"KANAIDX2"+struct.pack("<I",len(rows))+directory+payload


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
    source=source_manifest(db)
    manifest={"format":"KANAIDX2","records":len(rows),"bytes":len(data),
              "sha256":hashlib.sha256(data).hexdigest(),
              "generator_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "jmdict":{k:source["jmdict"][k] for k in ["url","sha256","created"]},
              "jitendex":{k:source["jitendex"][k] for k in ["url","sha256"]},
              "jitendex_revision":source["jitendex"]["index"]["revision"],
              "frequency_sources":source["frequency_sources"],
              "policy":"explicit kana links; unshared kana ranks or written ranks; 10x rival separation; linked-source contradictions veto",
              "license":"CC-BY-SA-4.0","attribution":source["attribution"]}
    manifest.update(policy_manifest())
    args.output.with_suffix(".json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n")
    print(json.dumps(manifest,ensure_ascii=False,indent=2))


if __name__ == "__main__":
    main()

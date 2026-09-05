#!/usr/bin/env python3
"""Audit the complete reference and evaluate experimental hints, never production matches."""
import argparse
import collections
import csv
import functools
import gzip
import hashlib
import json
import itertools
import sqlite3
from pathlib import Path
from build import normalize, packed


def open_reference(path):
    db = sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    return db


@functools.lru_cache(maxsize=32768)
def candidates(db, reading):
    result = {}
    for row in db.execute("SELECT * FROM pairs WHERE reading=?", (normalize(reading),)):
        entry = result.setdefault(row["entry_id"], {"entry_id": row["entry_id"], "reading": normalize(reading), "spellings": set(),
                "senses": set(), "kana_senses": set(), "pos": set(), "search_only": set()})
        entry["spellings"].add(row["spelling"])
        entry["senses"].add(row["sense"])
        if row["kana_preferred"]:
            entry["kana_senses"].add(row["sense"])
        entry["pos"].update(json.loads(row["pos"]))
        if "sK" in json.loads(row["spelling_info"]):
            entry["search_only"].add(row["spelling"])
    for entry in result.values():
        for key, value in list(entry.items()):
            if isinstance(value, set):
                entry[key] = sorted(value)
        entry["jitendex_kana"] = bool(db.execute("SELECT 1 FROM jitendex j WHERE sequence=? AND reading=? AND kana_paths!='[]' AND EXISTS(SELECT 1 FROM pairs p WHERE p.entry_id=j.sequence AND p.reading=j.reading AND p.spelling=j.term) LIMIT 1", (entry["entry_id"], normalize(reading))).fetchone())
    return list(result.values())


def select_hint(entries, lexeme=None):
    """Experimental hypothesis only. A kana tag is a preference, not a sense decision."""
    favored = [e for e in entries if e["kana_senses"] and e["jitendex_kana"]]
    if len(favored) != 1:
        return None
    selected = favored[0]
    if lexeme is not None and normalize(lexeme) not in {normalize(s) for s in selected["spellings"]}:
        return None
    return selected


def card_result(entries, selected, card):
    """Isolated spelling+reading snapshot; no live Anki calls or interval assumptions."""
    if card is None:
        return "Unmatched"
    spelling, reading = card
    eligible = [e for e in entries if spelling in e["spellings"] and normalize(reading) == e["reading"]]
    if not eligible:
        return "Unmatched"
    if selected:
        return "Known" if selected["entry_id"] in {e["entry_id"] for e in eligible} else "Unmatched"
    return "Possible"


def audit(db, output):
    output.mkdir(parents=True, exist_ok=True)
    summary = {t: db.execute("SELECT count(*) FROM " + t).fetchone()[0] for t in ["entries", "pairs", "jitendex", "frequencies"]}
    by_reading = collections.defaultdict(list)
    for row in db.execute("SELECT reading, entry_id, max(kana_preferred) uk FROM pairs GROUP BY reading,entry_id"):
        by_reading[row["reading"]].append((row["entry_id"], row["uk"]))
    summary["readings"] = len(by_reading)
    summary["ambiguous_readings"] = sum(len(v) > 1 for v in by_reading.values())
    summary["ambiguous_with_one_kana_entry"] = sum(len(v) > 1 and sum(x[1] for x in v) == 1 for v in by_reading.values())
    summary["ambiguous_with_multiple_kana_entries"] = sum(len(v) > 1 and sum(x[1] for x in v) > 1 for v in by_reading.values())
    with (output / "ambiguous_readings.csv").open("w", newline="") as stream:
        writer = csv.writer(stream)
        writer.writerow(["reading", "entry_ids", "kana_preferred_entry_ids"])
        for reading, entries in sorted(by_reading.items()):
            if len(entries) > 1:
                writer.writerow([reading, packed([x[0] for x in entries]), packed([x[0] for x in entries if x[1]])])
    summary["jitendex_redirect_rows"] = db.execute("SELECT count(*) FROM jitendex WHERE sequence<0").fetchone()[0]
    redirects = collections.Counter()
    for row in db.execute("SELECT sequence,sense_metadata FROM jitendex WHERE sequence<0"):
        targets = [x for x in json.loads(row["sense_metadata"]) if x.get("kind") == "redirect"]
        if not targets:
            redirects["without_extracted_target"] += 1
        elif all(t.get("reading") and db.execute("SELECT 1 FROM pairs WHERE entry_id=? AND spelling=? AND reading=? LIMIT 1", (abs(row["sequence"]),t["target"],normalize(t["reading"]))).fetchone() for t in targets):
            redirects["target_and_reading_validated"] += 1
        else:
            redirects["unresolved_target_or_reading"] += 1
    summary["jitendex_redirect_validation"] = dict(redirects)
    summary["jitendex_rows_with_jmdict_entry"] = db.execute("SELECT count(*) FROM jitendex j WHERE EXISTS(SELECT 1 FROM entries e WHERE e.id=j.sequence)").fetchone()[0]
    summary["jitendex_rows_with_legal_pair"] = db.execute("SELECT count(*) FROM jitendex j WHERE EXISTS(SELECT 1 FROM pairs p WHERE p.entry_id=j.sequence AND p.reading=j.reading AND p.spelling=j.term)").fetchone()[0]
    disagreements = db.execute("""SELECT j.sequence,j.term,j.reading,j.kana_paths!='[]' jitendex_kana,
        max(p.kana_preferred) jmdict_kana FROM jitendex j JOIN pairs p
        ON p.entry_id=j.sequence AND p.reading=j.reading AND p.spelling=j.term
        GROUP BY j.sequence,j.term,j.reading,j.kana_paths
        HAVING jitendex_kana != jmdict_kana""").fetchall()
    summary["kana_metadata_disagreements"] = len(disagreements)
    (output / "jitendex_disagreements.json").write_text(json.dumps([dict(r) for r in disagreements], ensure_ascii=False, indent=2) + "\n")
    distributions = collections.defaultdict(collections.Counter)
    for row in db.execute("""SELECT dictionary,term,reading,
            min(CASE WHEN kana_marker=0 THEN rank END) written,
            min(CASE WHEN kana_marker=1 THEN rank END) kana
            FROM frequencies WHERE rank>0 AND reading IS NOT NULL
            GROUP BY dictionary,term,reading HAVING written IS NOT NULL AND kana IS NOT NULL"""):
        d = distributions[row["dictionary"]]
        d["linked_pairs"] += 1
        d["kana_more_common"] += row["kana"] < row["written"]
        d["kana_rank_over_three_times_better"] += row["kana"] * 3 < row["written"]
        d["kana_top_1000_written_outside"] += row["kana"] <= 1000 < row["written"]
    summary["frequency_distributions"] = dict(distributions)
    summary["shared_kana_rank_groups"] = db.execute("SELECT count(*) FROM (SELECT dictionary,reading,rank FROM frequencies WHERE reading IS NOT NULL AND kana_marker=1 GROUP BY dictionary,reading,rank HAVING count(DISTINCT term)>1)").fetchone()[0]
    summary["sources"] = json.loads(db.execute("SELECT value FROM metadata WHERE key='manifest'").fetchone()[0])
    (output / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({k:v for k,v in summary.items() if k != "sources"}, ensure_ascii=False, indent=2))


def evaluate(db, tokens_path, probes_path, output):
    probes = json.loads(probes_path.read_text())
    probe_map = {p["id"]: p for p in probes}
    results = []
    totals = collections.defaultdict(collections.Counter)
    corpus_examples = []
    for line in tokens_path.open():
        sentence = json.loads(line)
        for token in sentence["tokens"]:
            is_kana = bool(token["surface"]) and all("ぁ" <= c <= "ヿ" or c == "ー" for c in token["surface"])
            if not is_kana and sentence["source"] != "probe":
                continue
            entries = candidates(db, token["reading"])
            if not entries:
                continue
            raw = select_hint(entries)
            contextual = select_hint(entries, token["lexeme"])
            totals[sentence["split"]]["analyzed_token_occurrences_with_entries"] += 1
            totals[sentence["split"]]["tag_only_hints"] += raw is not None
            totals[sentence["split"]]["tag_and_lexeme_hints"] += contextual is not None
            if sentence["source"] == "probe":
                probe = probe_map[sentence["id"]]
                if normalize(token["reading"]) != normalize(probe["reading"]):
                    continue
                expected = probe["expected"]
                def judgement(selected):
                    if selected is None:
                        return "abstain"
                    return "correct" if expected in selected["spellings"] else "incorrect"
                results.append({"id": probe["id"], "split": probe["split"], "text": probe["text"],
                    "expected": expected, "token": token, "candidates": entries,
                    "tag_only": judgement(raw), "tag_and_lexeme": judgement(contextual),
                    "selected_entry": contextual["entry_id"] if contextual else None})
            elif contextual and len(corpus_examples) < 100:
                corpus_examples.append({"source":sentence["source"], "id":sentence["id"],
                    "text":sentence["text"], "token":token, "selected":contextual})
    present = {r["id"] for r in results}
    missing = [p["id"] for p in probes if p["id"] not in present]
    report = {"probes_sha256":hashlib.sha256(probes_path.read_bytes()).hexdigest(),
              "counts":dict(totals), "probes":results, "missing_probes":missing,
              "note":"Hypothesis evaluation, not measured production accuracy. Tokenizer agreement is not independent ground truth."}
    output.mkdir(parents=True, exist_ok=True)
    (output / "evaluation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    (output / "corpus_review_sample.json").write_text(json.dumps(corpus_examples, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"counts":dict(totals), "missing_probes":missing,
        "probe_results":[{k:r[k] for k in ["id","expected","tag_only","tag_and_lexeme"]} for r in results]}, ensure_ascii=False, indent=2))


def export_reference(db, output):
    if output.exists():
        raise FileExistsError(output)
    manifest = json.loads(db.execute("SELECT value FROM metadata WHERE key='manifest'").fetchone()[0])
    manifest.pop("frequency_sources", None)
    rows = db.execute("SELECT sequence,term,reading,definition_tags,word_classes,score,sense_metadata FROM jitendex ORDER BY abs(sequence),sequence,term,reading")
    groups = iter(itertools.groupby(rows, key=lambda r:abs(r["sequence"])))
    group = next(groups, None)
    count = 0
    exported_jitendex = 0
    def project(group_rows):
        return [{"source_sequence":r["sequence"],"term":r["term"],"reading":r["reading"],
                 "tags":r["definition_tags"],"word_classes":r["word_classes"],"score":r["score"],
                 "sense_metadata":json.loads(r["sense_metadata"])} for r in group_rows]
    with output.open("wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as out:
        def write(record):
            out.write((packed(record)+"\n").encode())
        write({"manifest":manifest})
        for row in db.execute("SELECT id,data FROM entries ORDER BY id"):
            while group is not None and group[0] < row["id"]:
                data = project(group[1])
                write({"jmdict":None,"jitendex":data})
                exported_jitendex += len(data)
                group = next(groups, None)
            jitendex = []
            if group is not None and group[0] == row["id"]:
                jitendex = project(group[1])
                group = next(groups, None)
            write({"jmdict":json.loads(row["data"]),"jitendex":jitendex})
            exported_jitendex += len(jitendex)
            count += 1
        while group is not None:
            data = project(group[1])
            write({"jmdict":None,"jitendex":data})
            exported_jitendex += len(data)
            group = next(groups, None)
    assert exported_jitendex == db.execute("SELECT count(*) FROM jitendex").fetchone()[0]
    print(f"Exported {count} JMdict entries and {exported_jitendex} Jitendex rows: {output.stat().st_size:,} bytes")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=Path, required=True)
    sub = parser.add_subparsers(dest="command", required=True)
    a = sub.add_parser("audit")
    a.add_argument("--output", type=Path, required=True)
    q = sub.add_parser("lookup")
    q.add_argument("reading")
    x = sub.add_parser("export")
    x.add_argument("--output", type=Path, required=True)
    e = sub.add_parser("evaluate")
    e.add_argument("--tokens", type=Path, required=True)
    e.add_argument("--probes", type=Path, required=True)
    e.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    db = open_reference(args.db)
    if args.command == "audit":
        audit(db, args.output)
    elif args.command == "export":
        export_reference(db, args.output)
    elif args.command == "lookup":
        print(json.dumps(candidates(db, args.reading), ensure_ascii=False, indent=2))
    else:
        evaluate(db, args.tokens, args.probes, args.output)


if __name__ == "__main__":
    main()

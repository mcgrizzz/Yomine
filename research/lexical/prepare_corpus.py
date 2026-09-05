#!/usr/bin/env python3
"""Read cached SRT files; produce isolated, reproducible sentence input for UniDic."""
import argparse
import hashlib
import html
import json
import re
from pathlib import Path


def prepare(directory, probes, output):
    if output.exists():
        raise FileExistsError(output)
    manifest = []
    with output.open("w") as out:
        for path in sorted(directory.glob("*.srt")):
            raw = path.read_bytes()
            digest = hashlib.sha256(raw).hexdigest()
            split = "corpus_holdout" if int(digest[:8], 16) % 5 == 0 else "corpus_development"
            text = raw.decode("utf-8-sig").replace("\r\n", "\n")
            count = 0
            for block in re.split(r"\n\s*\n", text):
                lines = block.strip().splitlines()
                time = next((i for i, line in enumerate(lines) if "-->" in line), None)
                if time is None:
                    continue
                sentence = html.unescape(re.sub(r"<[^>]*>|\{[^}]*\}", "", "".join(lines[time+1:])))
                if not sentence.strip():
                    continue
                out.write(json.dumps(dict(source=path.name, id=count, split=split, text=sentence),ensure_ascii=False)+"\n")
                count += 1
            manifest.append(dict(file=path.name, sha256=digest, split=split, sentences=count))
        for p in json.loads(probes.read_text()):
            out.write(json.dumps(dict(source="probe",id=p["id"],split="probe_"+p["split"],text=p["text"]),ensure_ascii=False)+"\n")
    output.with_suffix(".manifest.json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n")
    print(f"{len(manifest)} subtitle files, {sum(x['sentences'] for x in manifest)} captions")


if __name__ == "__main__":
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--subtitles",type=Path,required=True)
    p.add_argument("--probes",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    a=p.parse_args()
    prepare(a.subtitles,a.probes,a.output)

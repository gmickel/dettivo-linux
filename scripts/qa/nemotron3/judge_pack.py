#!/usr/bin/env python3
"""Build blinded speaker-attribution packets for a model judge (fn-64), locally.

For each staged meeting alias, the product's own timed transcript segments (read-only
from the Dettivo database) get a speaker from each system's system-track output. With
`--rule overlap` (the first fn-64 pack) that is the single turn overlapping the segment
most, none when no turn overlaps. With `--rule product` it is segment_assign.py: the
speaker pass's own rule for turns, and the segment-based adapter for a `+segment`
system. Microphone segments are `You`. Windows of consecutive segments are sampled with
a fixed seed; in each, the systems are shown as columns A, B, ... in a random order,
with labels renumbered R1, R2, ... by first appearance so no system is recognisable.
The column key goes to `<out>-key.json`, a separate file the judge never reads.

Everything written here holds transcript text and stays in the protected directory.
"""
import argparse
import json
import random
import sqlite3
import string
from pathlib import Path

import segment_assign

DB = Path.home() / ".local/share/dettivo/dettivo.db"


def assign(segment, turns):
    best, speaker = 0, None
    for t in turns:
        shared = min(segment["end_ms"], t["end_ms"]) - max(segment["start_ms"], t["start_ms"])
        if shared > best:
            best, speaker = shared, str(t["speaker"])
    return speaker


def relabel(labels):
    order, out = [], []
    for label in labels:
        if label in ("You", None):
            out.append(label or "?")
            continue
        if label not in order:
            order.append(label)
        out.append(f"R{order.index(label) + 1}")
    return out


def stamp(ms):
    return f"{ms // 60000:02d}:{ms // 1000 % 60:02d}"


def meeting_segments(db, directory):
    (raw,) = db.execute("select segments from meetings where id = ?", (Path(directory).name,)).fetchone()
    return sorted(json.loads(raw), key=lambda s: s["start_ms"])


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--runs", type=Path, required=True, help="directory with <alias>.system.<system>.json")
    parser.add_argument("--systems", required=True, help="two or more systems, comma-separated")
    parser.add_argument("--rule", choices=("overlap", "product"), default="overlap")
    parser.add_argument("--probs", type=Path, help="directory with <alias>.system.npy, for +segment systems")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--windows", type=int, default=3)
    parser.add_argument("--size", type=int, default=60)
    parser.add_argument("--seed", type=int, default=64)
    args = parser.parse_args()
    systems = args.systems.split(",")
    rng = random.Random(args.seed)
    args.out.mkdir(parents=True, exist_ok=True)
    key = {}
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    for alias, directory in json.loads(args.selection.read_text()).items():
        segments = meeting_segments(db, directory)
        labels = {}
        for s in systems:
            if args.rule == "product":
                labels[s] = segment_assign.label_segments(segments, s, f"{alias}.system", args.runs, args.probs)
            else:
                turns = json.loads((args.runs / f"{alias}.system.{s}.json").read_text())["turns"]
                labels[s] = [("You" if seg["source_type"] == "microphone" else assign(seg, turns))
                             for seg in segments]
        starts = [i for i in range(0, max(len(segments) - args.size, 1), args.size)
                  if sum(seg["source_type"] == "system" for seg in segments[i:i + args.size]) >= 20]
        for w, start in enumerate(sorted(rng.sample(starts, min(args.windows, len(starts))))):
            name = f"{alias}-w{w + 1}"
            order = systems[:]
            rng.shuffle(order)
            letters = string.ascii_uppercase[:len(order)]
            key[name] = dict(zip(letters, order))
            span = slice(start, start + args.size)
            cols = [relabel(labels[s][span]) for s in order]
            lines = [f"{i + 1:>3} [{stamp(seg['start_ms'])}] "
                     + " ".join(f"{c}:{label:<3}" for c, label in zip(letters, row))
                     + f" | {seg['text'].strip()}"
                     for i, (seg, *row) in enumerate(zip(segments[span], *cols))]
            (args.out / f"{name}.txt").write_text("\n".join(lines) + "\n")
    args.out.with_name(f"{args.out.name}-key.json").write_text(json.dumps(key, indent=1))
    print(f"{len(key)} windows")


if __name__ == "__main__":
    main()

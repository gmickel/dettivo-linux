#!/usr/bin/env python3
"""How close the cached Whisper word times on AMI sit to the AMI manual word times: the
median and p95 of start, end and pooled offsets (docs/diarization-bench.md).

Reads the ASR stage's cached result for each AMI recording under the configured Whisper
engine (run `just diar-bench --full` first) and the reference words with their text from
the AMI manual annotations. A
hypothesis word pairs with a reference word of the same normalised text inside the
matching blocks of difflib over the two time-ordered word lists; only blocks of at
least MIN_BLOCK words count, so a lone "the" never pairs across a minute. Prints pooled
figures only.
"""
import argparse
import difflib
import json
import math
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from cache import Cache, key  # noqa: E402
from data import AMI_DEV, AMI_TEST, config, eval_dir  # noqa: E402
import engines  # noqa: E402

MIN_BLOCK = 3


def normalise(text):
    return "".join(c for c in text if c.isalnum()).lower()


def reference(z, meeting, agents):
    out = []
    for agent in agents:
        for w in ET.fromstring(z.read(f"words/{meeting}.{agent}.words.xml")).iter("w"):
            if w.get("punc") == "true" or w.get("starttime") is None or w.get("endtime") is None:
                continue
            text = normalise(w.text or "")
            if text:
                start = round(float(w.get("starttime")) * 1000)
                out.append((text, start, max(start, round(float(w.get("endtime")) * 1000))))
    return sorted(out, key=lambda x: (x[1], x[2]))


def hypothesis(segments):
    out = []
    for s in segments:
        for w in s.get("words", []):
            text = normalise(w["text"])
            if text:
                out.append((text, w["start_ms"], w["end_ms"]))
    return out


def pairs(ref, hyp):
    matcher = difflib.SequenceMatcher(None, [r[0] for r in ref], [h[0] for h in hyp], autojunk=False)
    for a, b, n in matcher.get_matching_blocks():
        if n >= MIN_BLOCK:
            for k in range(n):
                yield ref[a + k], hyp[b + k]


def rank(values, p):
    """Nearest-rank percentile, as dettivo_qa::alignment computes it."""
    values = sorted(values)
    return values[min(len(values), max(1, math.ceil(p * len(values)))) - 1] if values else None


def main():
    parser = argparse.ArgumentParser(prog="word_timing.py", description=__doc__.splitlines()[0])
    parser.add_argument("--heldout", action="store_true", help="measure AMI test instead of AMI dev")
    args = parser.parse_args()
    root = eval_dir()
    cfg, cache = config(root), Cache(root)
    names = AMI_TEST if args.heldout else AMI_DEV
    with zipfile.ZipFile(root / "ami/ami_public_manual_1.6.2.zip") as z:
        meetings = {m.get("observation"): [s.get("nxt_agent") for s in m.iter("speaker")]
                    for m in ET.fromstring(z.read("corpusResources/meetings.xml")).iter("meeting")}
        starts, ends, ref_words, hyp_words, files = [], [], 0, 0, 0
        for name in names:
            wav = root / "ami" / f"{name}.wav"
            if not wav.exists():
                continue
            k, segments = engines.asr_result(cache, cfg["asr"], wav, False)
            if k != key(cache.file_hash(wav), engines.asr_identity(cache, cfg["asr"])):
                print(f"{name}: no cached words for this Whisper build (run `just diar-bench --full`)")
                continue
            ref, hyp = reference(z, name, meetings[name]), hypothesis(segments)
            files, ref_words, hyp_words = files + 1, ref_words + len(ref), hyp_words + len(hyp)
            for r, h in pairs(ref, hyp):
                starts.append(abs(r[1] - h[1]))
                ends.append(abs(r[2] - h[2]))
    cache.save()
    both = starts + ends
    print(json.dumps({"split": "ami-test" if args.heldout else "ami-dev", "files": files,
                      "reference_words": ref_words, "whisper_words": hyp_words, "paired": len(starts),
                      "start": {"median_ms": rank(starts, 0.5), "p95_ms": rank(starts, 0.95)},
                      "end": {"median_ms": rank(ends, 0.5), "p95_ms": rank(ends, 0.95)},
                      "combined": {"median_ms": rank(both, 0.5), "p95_ms": rank(both, 0.95)}}, indent=1))


if __name__ == "__main__":
    main()

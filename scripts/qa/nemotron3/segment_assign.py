"""Give transcript segments a speaker, the way the product would, for fn-64 R6.

Two rules label a segment (start_ms, end_ms):

- `product`, for a system that returns turns: the speaker pass's rule in
  crates/dettivo-meeting/src/diarize.rs. The segment gets the speaker with the most
  overlap when at least `min_coverage` (0.25) of its span lies inside diarized speech and
  that speaker holds at least `min_speaker_share` (0.6) of it; otherwise none.
- `segment`, the Nemotron-specific adapter: the mean per-speaker probability over the
  segment's 10 ms frames, and the speaker with the highest mean, with no 0.5 threshold.
  Only the channels the recorder post-processing keeps as speakers compete (clusters
  under max(4 s, 4 % of speech) are absorbed), so the adapter never adds a speaker the
  turns would not have. A segment gets none only when it has no frames.

A system named `<nemotron system>+segment` uses the adapter on `<probs>/<key>.npy`
(written by `nemotron3_diarize.py --probs`); any other system uses the product rule on
`<runs>/<key>.<system>.json`. Experiment only; nothing in the product calls this.
"""
from pathlib import Path
import json

import numpy as np

import nemotron3_diarize

FRAME_MS = 10
SEGMENT = "+segment"


def product_label(start, end, turns, min_coverage=0.25, min_speaker_share=0.6):
    if end <= start:
        return None
    pieces = sorted((max(t["start_ms"], start), min(t["end_ms"], end)) for t in turns)
    covered, reach = 0, start
    for a, b in pieces:
        a = max(a, reach)
        if b > a:
            covered, reach = covered + b - a, b
    if covered / (end - start) < min_coverage:
        return None
    per_speaker = {}
    for t in turns:
        shared = min(end, t["end_ms"]) - max(start, t["start_ms"])
        if shared > 0:
            per_speaker[str(t["speaker"])] = per_speaker.get(str(t["speaker"]), 0) + shared
    if not per_speaker:
        return None
    best = max(per_speaker, key=per_speaker.get)
    return best if per_speaker[best] / sum(per_speaker.values()) >= min_speaker_share else None


def kept_channels(probs):
    """The channels the recorder keeps as speakers in automatic count mode."""
    return sorted(nemotron3_diarize.kept(nemotron3_diarize.segments(probs)))


def segment_label(start, end, probs, channels):
    first = start // FRAME_MS
    frames = probs[first:max(first + 1, -(-end // FRAME_MS))]
    if not len(frames) or not channels:
        return None
    means = frames[:, channels].astype(np.float32).mean(axis=0)
    return str(channels[int(np.argmax(means))])


def labeller(system, key, runs, probs):
    """A function (start_ms, end_ms) -> speaker label or None for one recording."""
    if system.endswith(SEGMENT):
        p = np.load(Path(probs) / f"{key}.npy")
        channels = kept_channels(p.astype(np.float32))
        return lambda start, end: segment_label(start, end, p, channels)
    turns = json.loads((Path(runs) / f"{key}.{system}.json").read_text())["turns"]
    return lambda start, end: product_label(start, end, turns)


def label_segments(segments, system, key, runs, probs, room_audio=False):
    """Labels for `segments` (dicts with start_ms, end_ms and, for two-track meetings,
    source_type); microphone segments of a two-track meeting are `You`."""
    label = labeller(system, key, runs, probs)
    return [("You" if not room_audio and s.get("source_type") == "microphone"
             else label(s["start_ms"], s["end_ms"])) for s in segments]

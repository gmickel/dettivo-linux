#!/usr/bin/env python3
"""Score diarization of a meeting's mix against the weak reference its two tracks give.

A Dettivo meeting records the microphone (the local speaker) and the system track (the
remote participants) separately. Per 10 ms frame, a track is active above -45 dBFS and
quiet below -60 dBFS, with active runs bridged over 200 ms gaps and runs under 200 ms
dropped. The 15 dB between the two is the bleed margin: a frame is `local` when the
microphone is active and the system track quiet, `remote` for the reverse, and `overlap`
when both are active. Frames in the margin are not scored.

Each hypothesis speaker is mapped to the side holding most of its single-side frames
(many-to-one, since several remote voices share one side). Reported:
- side_confusion: share of single-side frames under hypothesis speech whose speaker maps
  to the other side, a lower bound on wrong-speaker time (remote-remote swaps are invisible);
- side_missed: share of single-side frames with no hypothesis speech;
- overlap_resolved: share of overlap frames where speakers of both sides are active;
- clusters: hypothesis speakers, and those holding at least 5 % of either side's frames.
"""
import argparse
import json
import wave

import numpy as np

ACTIVE_DB, QUIET_DB, BRIDGE, MIN_RUN = -45.0, -60.0, 20, 20


def frame_db(path):
    with wave.open(path, "rb") as w:
        x = np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float32) / 32768.0
    n = len(x) // 160
    rms = np.sqrt((x[: n * 160].reshape(n, 160) ** 2).mean(axis=1))
    return 20 * np.log10(rms + 1e-9)


def runs(mask):
    edges = np.flatnonzero(np.diff(np.concatenate([[0], mask.astype(np.int8), [0]])))
    return list(zip(edges[::2], edges[1::2]))


def smooth(mask):
    out = mask.copy()
    spans = runs(mask)
    for (_, b), (c, _) in zip(spans, spans[1:]):
        if c - b < BRIDGE:
            out[b:c] = True
    for a, b in runs(out):
        if b - a < MIN_RUN:
            out[a:b] = False
    return out


def reference(mic_wav, system_wav):
    mic, system = frame_db(mic_wav), frame_db(system_wav)
    n = min(len(mic), len(system))
    mic, system = mic[:n], system[:n]
    mic_on, sys_on = smooth(mic > ACTIVE_DB), smooth(system > ACTIVE_DB)
    mic_quiet, sys_quiet = ~mic_on & (mic < QUIET_DB), ~sys_on & (system < QUIET_DB)
    return mic_on & sys_quiet, sys_on & mic_quiet, mic_on & sys_on


def hypothesis(turns, n):
    labels = sorted({str(t["speaker"]) for t in turns})
    active = np.zeros((len(labels), n), bool)
    for t in turns:
        a, b = int(t["start_ms"]) // 10, min(int(t["end_ms"]) // 10, n)
        active[labels.index(str(t["speaker"])), a:b] = True
    return labels, active


def score(turns, mic_wav, system_wav):
    local, remote, overlap = reference(mic_wav, system_wav)
    n = len(local)
    labels, active = hypothesis(turns, n)
    on_local, on_remote = active[:, local].sum(axis=1), active[:, remote].sum(axis=1)
    is_remote = on_remote > on_local
    speech = active.any(axis=0)
    single = local | remote
    ref_remote = remote[None, :]
    wrong = (active & (is_remote[:, None] != ref_remote) & single[None, :]).any(axis=0)
    right = (active & (is_remote[:, None] == ref_remote) & single[None, :]).any(axis=0)
    covered = single & speech
    return {
        "frames_local": int(local.sum()), "frames_remote": int(remote.sum()), "frames_overlap": int(overlap.sum()),
        "frames_unscored_speech": int(((speech) & ~single & ~overlap).sum()),
        "side_confusion": float((wrong & ~right & covered).sum() / max(covered.sum(), 1)),
        "side_missed": float((single & ~speech).sum() / max(single.sum(), 1)),
        "local_missed": float((local & ~speech).sum() / max(local.sum(), 1)),
        "overlap_resolved": float(((active & ~is_remote[:, None]).any(axis=0) & (active & is_remote[:, None]).any(axis=0)
                                   & overlap).sum() / max(overlap.sum(), 1)),
        "clusters": len(labels),
        "clusters_material": int(((on_local >= 0.05 * max(local.sum(), 1)) | (on_remote >= 0.05 * max(remote.sum(), 1))).sum()),
        "clusters_local": int((~is_remote & (on_local > 0)).sum()),
        "clusters_remote": int(is_remote.sum()),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("input", help="native JSON with turns, from the mix")
    parser.add_argument("--microphone", required=True)
    parser.add_argument("--system", required=True)
    args = parser.parse_args()
    with open(args.input) as f:
        turns = json.load(f)["turns"]
    print(json.dumps(score(turns, args.microphone, args.system), indent=2))


if __name__ == "__main__":
    main()

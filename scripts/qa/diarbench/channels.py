"""The local/remote proxy for two-track meetings, from the tracks' levels.

fn-64's weak reference (scripts/qa/nemotron3/channel_score.py) marks each 10 ms frame
`local` (microphone active, system track quiet), `remote` (the reverse) or `overlap`.
The frames are cached per meeting, so a run reads no audio once they exist. The engine
diarizes the summed tracks, each of its speakers maps to the side holding most of its
frames, and the proxy is the share of single-side speech given to the other side.
"""
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "nemotron3"))
import channel_score  # noqa: E402

from cache import key  # noqa: E402


def frames(cache, mic, system):
    """(local, remote, overlap) bool arrays at 10 ms, cached by both tracks' hashes."""
    k = key(cache.file_hash(mic), cache.file_hash(system), cache.file_hash(channel_score.__file__))
    path = cache.path("channels", k, ".npz")
    if path.exists():
        cache.note("channels", "cached")
        with np.load(path) as z:
            return k, (z["local"], z["remote"], z["overlap"])
    local, remote, overlap = channel_score.reference(str(mic), str(system))
    tmp = path.with_name(path.stem + ".tmp.npz")
    np.savez_compressed(tmp, local=local, remote=remote, overlap=overlap)
    tmp.replace(path)
    cache.note("channels", "computed")
    return k, (local, remote, overlap)


def mix_proxy(turns, local, remote, overlap):
    s = channel_score.score(turns, None, None, frames=(local, remote, overlap))
    single = s["frames_local"] + s["frames_remote"]
    return {"mix_single": single, "mix_confused": s["side_confusion"] * single}

#!/usr/bin/env python3
"""Stage retained meetings for fn-64 under an alias, outside the repository.

--selection is a local JSON object {"<alias>": "<meeting directory>"}; it stays in the
protected working directory, so meeting ids never meet the aggregate numbers. For each
alias this links system.wav (the speaker pass's input) and microphone.wav and writes
mix.wav (the two summed and clipped), the input where local and remote voices can be
confused. Only completed single-take meetings of equal track length are accepted.
"""
import argparse
import json
import wave
from pathlib import Path

import numpy as np


def pcm(path):
    with wave.open(str(path), "rb") as w:
        assert (w.getframerate(), w.getnchannels(), w.getsampwidth()) == (16000, 1, 2), path
        return np.frombuffer(w.readframes(w.getnframes()), dtype="<i2")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    for alias, directory in json.loads(args.selection.read_text()).items():
        src = Path(directory)
        takes = json.loads((src / "takes.json").read_text())
        system_takes = json.loads((src / "system-takes.json").read_text())
        if (src / "live-checkpoint.json").exists() or len(takes.get("takes", takes)) != 1:
            raise SystemExit(f"{alias}: not a finished single-take meeting")
        if len(system_takes.get("takes", system_takes)) != 1:
            raise SystemExit(f"{alias}: more than one system take")
        mic, system = pcm(src / "microphone.wav"), pcm(src / "system.wav")
        if len(mic) != len(system):
            raise SystemExit(f"{alias}: track lengths differ")
        dest = args.out / alias
        dest.mkdir(parents=True, exist_ok=True)
        for name in ("system.wav", "microphone.wav"):
            link = dest / name
            if not link.exists():
                link.symlink_to(src / name)
        mix = np.clip(mic.astype(np.int32) + system.astype(np.int32), -32768, 32767).astype("<i2")
        with wave.open(str(dest / "mix.wav"), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(16000)
            w.writeframes(mix.tobytes())
        print(f"{alias}: {len(mic) / 16000 / 60:.1f} min staged")


if __name__ == "__main__":
    main()

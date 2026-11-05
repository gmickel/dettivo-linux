#!/usr/bin/env python3
"""Render the term-recall fixture set from its definition.

`crates/dettivo-qa/fixtures/vocabulary/set.json` lists every clip: its id,
the Piper voice that speaks it and `say`, the text the voice reads (a term
is spelt the way it is pronounced, `Sapien X T` for SapienXT). This script
synthesises each clip with sherpa-onnx's offline TTS and writes
`<id>.wav` beside the definition, 16 kHz mono 16-bit, silence under the
floor trimmed from both ends and a short lead and tail added back. A clip
whose `<id>.wav` exists is left alone: synthesis is not byte-reproducible,
and the benchmark stays comparable across runs only while the existing
clips stay the same. Delete a clip's file to render it again.
`dettivo-qa bench vocabulary` reads the clips (ADR 0087).

The voices are the rig's: the Piper `vits-piper-en_US-amy-low` and
`vits-piper-en_US-ryan-low` sets from sherpa-onnx's tts-models release,
one directory each under <voices-dir>. sherpa-onnx comes from PyPI as
`pip install "sherpa-onnx<1.13"`: the 2.x wheels carry no espeak-ng
frontend for the Piper voices.

Usage: make-vocabulary-fixture.py <voices-dir> [<fixture-dir>]
"""

import array
import json
import os
import sys
import wave

import sherpa_onnx

RATE = 16_000
FLOOR = 250
LEAD_MS = 300
TAIL_MS = 400


def voice(voices_dir, name):
    root = os.path.join(voices_dir, f"vits-piper-en_US-{name}-low")
    config = sherpa_onnx.OfflineTtsConfig(
        model=sherpa_onnx.OfflineTtsModelConfig(
            vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                model=os.path.join(root, f"en_US-{name}-low.onnx"),
                tokens=os.path.join(root, "tokens.txt"),
                data_dir=os.path.join(root, "espeak-ng-data"),
            ),
            num_threads=2,
            provider="cpu",
        ),
    )
    return sherpa_onnx.OfflineTts(config)


def pcm(samples):
    out = array.array("h")
    for s in samples:
        out.append(max(-32768, min(32767, int(s * 32767))))
    return out


def trimmed(samples):
    start = 0
    while start < len(samples) and abs(samples[start]) < FLOOR:
        start += 1
    end = len(samples)
    while end > start and abs(samples[end - 1]) < FLOOR:
        end -= 1
    return samples[start:end]


def write(path, samples):
    padded = array.array("h", [0] * (RATE * LEAD_MS // 1000))
    padded.extend(samples)
    padded.extend([0] * (RATE * TAIL_MS // 1000))
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(padded.tobytes())


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    voices_dir = sys.argv[1]
    here = os.path.dirname(os.path.abspath(__file__))
    default = os.path.join(here, "..", "..", "crates", "dettivo-qa", "fixtures", "vocabulary")
    fixture_dir = sys.argv[2] if len(sys.argv) == 3 else default
    with open(os.path.join(fixture_dir, "set.json")) as f:
        clips = json.load(f)["clips"]
    engines = {}
    for clip in clips:
        path = os.path.join(fixture_dir, f"{clip['id']}.wav")
        if os.path.exists(path):
            continue
        name = clip["voice"]
        if name not in engines:
            engines[name] = voice(voices_dir, name)
        audio = engines[name].generate(clip["say"], sid=0, speed=1.0)
        if audio.sample_rate != RATE:
            sys.exit(f"{name}: {audio.sample_rate} Hz, want {RATE}")
        write(path, trimmed(pcm(audio.samples)))
        print(clip["id"])


if __name__ == "__main__":
    main()

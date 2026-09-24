#!/usr/bin/env python3
"""Assemble the diarization fixture from six synthesised utterances.

The fixture `crates/dettivo-qa/fixtures/diarization/two-speakers.wav` is
two Piper voices (en_US-amy-low as speaker A, en_US-ryan-low as speaker B)
taking six turns, alternating, with one overlap where A starts before B
has finished. `two-speakers.turns.json` beside it carries the expected
turns on the same clock and the speaker count, which
`dettivo-qa pipeline diarization` scores the engine against.

The utterances come from sherpa-onnx's offline TTS with the Piper voices
(`sherpa-onnx-offline-tts --vits-model=.../en_US-amy-low.onnx ...`), one
16 kHz mono WAV per line of the script below, named a1, r1, a2, r2, a3, r3
in a directory this script reads. Silence under the floor is trimmed from
both ends of every utterance so the turn boundaries are tight.

Usage: make-diarization-fixture.py <utterance-dir> <out-dir>
"""

import array
import json
import os
import sys
import wave

RATE = 16_000
FLOOR = 250
LEAD_MS = 400
GAP_MS = 300
OVERLAP_MS = 600
TAIL_MS = 400

# (file, speaker, text, overlap with the previous turn)
SCRIPT = [
    ("a1", "A", "Good morning everyone, thanks for joining the weekly sync.", False),
    ("r1", "B", "Morning. The API gateway rollout moves to Thursday.", False),
    ("a2", "A", "That works for me. Did the retry loop fix land in the uploader?", False),
    ("r2", "B", "It did, and the latency dropped to forty milliseconds.", False),
    ("a3", "A", "Great, then we can close the ticket today.", True),
    ("r3", "B", "Agreed. I will update the notes after the call.", False),
]


def read(path):
    with wave.open(path, "rb") as w:
        assert w.getframerate() == RATE, path
        assert w.getnchannels() == 1, path
        assert w.getsampwidth() == 2, path
        samples = array.array("h")
        samples.frombytes(w.readframes(w.getnframes()))
    return samples


def trimmed(samples):
    start = 0
    while start < len(samples) and abs(samples[start]) < FLOOR:
        start += 1
    end = len(samples)
    while end > start and abs(samples[end - 1]) < FLOOR:
        end -= 1
    return samples[start:end]


def ms(n):
    return n * 1000 // RATE


def main(src, out):
    mix = array.array("h", [0] * (RATE * LEAD_MS // 1000))
    turns = []
    prev_end = len(mix)
    for name, speaker, text, overlap in SCRIPT:
        voice = trimmed(read(os.path.join(src, f"{name}.wav")))
        if overlap:
            start = prev_end - RATE * OVERLAP_MS // 1000
        else:
            start = prev_end + RATE * GAP_MS // 1000
        end = start + len(voice)
        if end > len(mix):
            mix.extend([0] * (end - len(mix)))
        for i, v in enumerate(voice):
            mixed = mix[start + i] + v
            mix[start + i] = max(-32768, min(32767, mixed))
        turns.append(
            {"speaker": speaker, "start_ms": ms(start), "end_ms": ms(end), "text": text}
        )
        prev_end = end
    mix.extend([0] * (RATE * TAIL_MS // 1000))
    os.makedirs(out, exist_ok=True)
    with wave.open(os.path.join(out, "two-speakers.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(mix.tobytes())
    doc = {
        "speakers": 2,
        "duration_ms": ms(len(mix)),
        "voices": {"A": "en_US-amy-low", "B": "en_US-ryan-low"},
        "turns": turns,
    }
    with open(os.path.join(out, "two-speakers.turns.json"), "w") as f:
        json.dump(doc, f, indent=2)
        f.write("\n")
    print(f"wrote {out}/two-speakers.wav ({ms(len(mix))} ms) and two-speakers.turns.json")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])

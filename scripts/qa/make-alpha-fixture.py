#!/usr/bin/env python3
"""Render the alpha meeting fixture: two voices, one per source.

`crates/dettivo-qa/fixtures/meetings/alpha.mic.wav` is Alice (Piper
en_US-amy-low) and `alpha.system.wav` is Ben (Piper en_US-ryan-low), the
two sides of a short call on one clock: Alice speaks first on the
microphone track, Ben answers on the system track once she has finished,
and both files run for the same length so the two play together through
the audio rig or the mock variables. `alpha.tokens.json` beside them
names the words each source must carry through capture, transcription
and diarization, which the `meeting_token_coverage` scenario asserts
(docs/qa.md).

The voices come from sherpa-onnx's offline TTS with the Piper models the
diarization fixture used (`vits-piper-en_US-amy-low`,
`vits-piper-en_US-ryan-low` from the sherpa-onnx `tts-models` release).
Silence under the floor is trimmed from both ends of each utterance so
the fixture's clock is tight, and the audio is resampled to 16 kHz mono.

Usage: make-alpha-fixture.py <amy-voice-dir> <ryan-voice-dir> <out-dir>
(with `sherpa_onnx` and `numpy` importable)
"""

import array
import json
import os
import sys
import wave

RATE = 16_000
FLOOR = 250
LEAD_MS = 500
GAP_MS = 700
TAIL_MS = 800

# The script: one line per source, with the tokens each must carry.
SCRIPT = [
    ("microphone", "amy", "Hi everyone, Alice here. Today we walk through the launch checklist."),
    ("system", "ryan", "Thanks Alice, Ben speaking. The pricing decision is made, so we ship on Friday."),
]
TOKENS = {
    "microphone": ["alice", "launch", "checklist"],
    "system": ["ben", "pricing", "decision", "ship", "friday"],
}


def synthesise(voice_dir, text):
    import numpy as np
    import sherpa_onnx

    name = os.path.basename(voice_dir.rstrip("/")).replace("vits-piper-", "")
    config = sherpa_onnx.OfflineTtsConfig(
        model=sherpa_onnx.OfflineTtsModelConfig(
            vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                model=os.path.join(voice_dir, f"{name}.onnx"),
                lexicon="",
                tokens=os.path.join(voice_dir, "tokens.txt"),
                data_dir=os.path.join(voice_dir, "espeak-ng-data"),
            ),
            num_threads=2,
        )
    )
    tts = sherpa_onnx.OfflineTts(config)
    audio = tts.generate(text, sid=0, speed=1.0)
    samples = np.asarray(audio.samples, dtype=np.float32)
    rate = audio.sample_rate
    if rate != RATE:
        positions = np.arange(0, len(samples), rate / RATE)
        samples = np.interp(positions, np.arange(len(samples)), samples)
    pcm = np.clip(samples * 32767.0, -32768, 32767).astype(np.int16)
    return trim(pcm.tolist())


def trim(samples):
    start = 0
    while start < len(samples) and abs(samples[start]) < FLOOR:
        start += 1
    end = len(samples)
    while end > start and abs(samples[end - 1]) < FLOOR:
        end -= 1
    return samples[start:end]


def ms(n):
    return n * RATE // 1000


def write(path, samples):
    with wave.open(path, "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(array.array("h", samples).tobytes())


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    amy, ryan, out_dir = sys.argv[1:]
    os.makedirs(out_dir, exist_ok=True)
    voices = {"amy": amy, "ryan": ryan}
    utterances = [(source, synthesise(voices[voice], text)) for source, voice, text in SCRIPT]
    # Alice from the lead, Ben once she has finished plus the gap, both
    # files padded to the same length.
    alice = utterances[0][1]
    ben = utterances[1][1]
    ben_start = ms(LEAD_MS) + len(alice) + ms(GAP_MS)
    total = ben_start + len(ben) + ms(TAIL_MS)
    mic = [0] * ms(LEAD_MS) + alice
    mic += [0] * (total - len(mic))
    system = [0] * ben_start + ben
    system += [0] * (total - len(system))
    write(os.path.join(out_dir, "alpha.mic.wav"), mic)
    write(os.path.join(out_dir, "alpha.system.wav"), system)
    turns = [
        {"source": "microphone", "speaker": "Alice", "start_ms": LEAD_MS, "end_ms": LEAD_MS + len(alice) * 1000 // RATE, "text": SCRIPT[0][2]},
        {"source": "system", "speaker": "Ben", "start_ms": ben_start * 1000 // RATE, "end_ms": (ben_start + len(ben)) * 1000 // RATE, "text": SCRIPT[1][2]},
    ]
    tokens = {
        "voices": {"microphone": "en_US-amy-low", "system": "en_US-ryan-low"},
        "duration_ms": total * 1000 // RATE,
        "turns": turns,
        "tokens": [{"token": t, "source": source} for source, ts in TOKENS.items() for t in ts],
    }
    with open(os.path.join(out_dir, "alpha.tokens.json"), "w") as f:
        json.dump(tokens, f, indent=2)
        f.write("\n")
    print(f"alpha: {total * 1000 // RATE} ms, Alice {len(alice) * 1000 // RATE} ms, Ben {len(ben) * 1000 // RATE} ms from {ben_start * 1000 // RATE} ms")


if __name__ == "__main__":
    main()

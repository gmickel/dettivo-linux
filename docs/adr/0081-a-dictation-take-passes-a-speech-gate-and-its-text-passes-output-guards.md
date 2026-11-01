# 0081. A dictation take passes a speech gate before the engine, and its text passes output guards after it

Status: Accepted 2026-11-01. Amends 0048 (a take's silence).

## What this gives you

An empty or noisy take ends as "Nothing heard" instead of pasting "Thank you.", a vocabulary is never pasted back as if it had been spoken, and a decoder loop leaves the words before it instead of a page of repeats.

## Situation

Until fn-79 a take counted as silence only when its single loudest sample stayed under `[dictation] silence_peak_threshold` (0.01, about -40 dBFS). One click or keyboard thump passed that check, and the engine then invented text for the rest of the take. The catalogue carried the Silero voice activity model (`whisper/silero-vad`) and the Whisper engine accepted a `vad_model`, but every caller passed none. The meeting pipeline dropped known silence hallucinations and blank markers (`dettivo_transcribe::filters`), and dictation never ran that filter. The vocabulary prompt had no length cap, and Whisper keeps the end of an overlong prompt, so the first terms of a long list were lost.

## Decision

- **Stage one, the level.** `dettivo_speech::gate` measures the loudest 30 ms window of the take in dBFS, with the take's mean (its DC offset) removed. A take under `[dictation] silence_dbfs` (-50) is silence. The gate reads the samples the audio source delivered before the session stopped it, the ones still queued when the stop arrived included. What the source sends after it was told to stop never counts. A take shorter than one window is silence.
- **Stage two, a voice.** When the level passes and the Silero model is downloaded and verifies, the take goes to a voice activity pass with threshold 0.4 and a 160 ms minimum stretch of speech, on the CPU. No speech is silence. The pass runs in `dettivo-engine-whisper` loaded with only the VAD model (`load` with an empty `model` and a `vad_model`), which answers the new protocol request `detect_speech` with `speech_ms`. The supervisor keeps that process in its own slot (`dettivo-engine-whisper/vad`), so it never evicts a loaded Whisper model, and the daemon does not publish its transitions as `engine.state`. Whisper and Parakeet takes go through the same pass. Without the model, or when the pass fails, the level stage decides alone and dictation keeps working.
- **A gated take** never reaches the engine and ends `cancelled` with `reason` `silence: level` or `silence: voice`. The OSD shows "Nothing heard" for it. Nothing is stored in history.
- **Padding.** A take reaches the engine at least 1.5 s long, and a Parakeet take carries 400 ms of trailing silence. When the padded take comes back with no text, the session recognizes the take once more as captured, so padding never empties a take. The gate never trims a take that passes.
- **Output guards**, after recognition and before the text pipeline, per segment (`dettivo-session` `guards`). A decoder loop, an n-gram of up to 8 words repeated at least 4 times in a row over at least 8 words, is cut back to the text before it. A segment the meeting filter drops (a known silence hallucination or a blank marker) goes. A segment, or the whole text, whose normalised form (lower case, punctuation stripped, spelled digits as digits) equals the whole prompt goes, when the engine took a prompt of two or more terms. Dictating the one listed term still pastes it. The engine's own text stands when no segment changed.
- **The prompt** keeps whole terms in list order until the next one would take it past 500 characters, and stays terminated.
- `silence_peak_threshold` is retired. A file that still sets it loads, and the daemon ignores the value. `silence_dbfs` takes its row on Settings / General.

## Consequences

- The VAD process costs one more engine process while dictation is warm, unloaded after `[engines] stt_idle_seconds` like the others. Measured on the jfk fixture through the protocol, the pass reports 7600 ms of speech for the full 11 s, 6710 ms with the audio 24 dB down, 480 ms for the first 0.75 s, and 0 for 3 s of silence, steady noise at 0.09 of full scale and a 100 Hz hum.
- The level stage gates a click or thump whose 30 ms energy stays under -50 dBFS. A louder transient passes the level and relies on the voice stage, so a machine without the Silero model still pastes for a loud click.
- Downloading a speech model through `speech.models.download` queues `whisper/silero-vad` too. `dettivo doctor` reports the model on its `voice gate` row and, when it is missing, the `dettivo speech download --model silero-vad` command.
- The 400 ms tail did not change Parakeet v2's output on any jfk cut measured on 2026-11-01. On Parakeet v3 it emptied two of three short takes that came back whole without it (the fixture from 8.5 s to 10.48 s returned nothing with 300, 400, 500 or 1000 ms of trailing silence and the full sentence with 0, 100, 200, 700 or 1500 ms). The second recognition recovers those takes and costs one more inference only when the first came back empty. v3 also returned nothing for a 1.18 s take with and without padding. That sensitivity in parakeet.cpp's v3 decode is open.
- A take that says exactly the whole vocabulary, in order, pastes nothing when the vocabulary has two or more terms.

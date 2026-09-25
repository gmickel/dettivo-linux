Stub. Experiment only. Do not replace the current speaker pass until this measures a real win on Dettivo Linux meetings.

## Goal & Context

Jan Kees van Weert shipped Meeting Recorder 1.1 with NVIDIA Nemotron 3 Diarization. On two real meetings the wrong-speaker rate went from 17% and 54% to 2% and 6%, in about a quarter of the time, with overlap handling up to eight speakers.

We need the same comparison on Dettivo Linux's current diarize engine (Sherpa-ONNX / ggml path, CPU default, optional CUDA from fn-42), not a tweet-shaped rewrite. If Nemotron is actually that much better here, a later spec can swap or add it. This spec only answers whether it is.

Source: https://x.com/jankeesvw/status/2103235751739793688
Release: https://github.com/jankeesvw/omarchy-meeting-recorder/releases/tag/v1.1.0

## Approach

- Keep the current diarize engine as the baseline.
- Run Nemotron 3 Diarization as a side-by-side post-pass on the same retained meeting audio.
- Score wrong-speaker rate and wall time on at least two real Dettivo meetings (mic + system tracks where we have them).
- Report numbers. No product default change in this spec.

## Quick commands

- `just build test lint`

## Acceptance

- **R1:** Baseline wrong-speaker rate is measured with the current diarize engine on at least two retained real meetings, with the scoring method written down.
- **R2:** The same meetings are scored with Nemotron 3 Diarization on this machine, including wall time versus baseline.
- **R3:** A short evidence note records both rates, both times, model/runtime identity (CPU vs CUDA), and whether overlap/crosstalk improved.
- **R4:** Shipping default stays the current diarize engine unless a follow-up spec, with these numbers, says otherwise.

## Boundaries

- No replacement of `dettivo-engine-diarize` in the daemon or package.
- No AUR/release, no Settings/config default change.
- No confidential transcript text in the spec or git; keep audio and transcripts in protected local storage.
- macOS comparison is a sibling spec on `dettivo` (`fn-83-evaluate-nemotron-3-diarization-against`), not this checkout.

## Decision Context

Triggered by the Meeting Recorder 1.1 claim. Linux already has a speaker pass and a CUDA drop-in; the only question is whether Nemotron 3 beats that enough on our meetings to justify a later integration spec.

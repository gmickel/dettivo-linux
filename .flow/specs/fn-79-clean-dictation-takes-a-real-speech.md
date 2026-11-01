# Clean dictation takes: a real speech gate and engine output guards

## Goal & Context

An empty or noisy take should never paste "Thank you." A short take should never lose its last word, and a vocabulary should never be pasted back as if it had been spoken.

Today a take counts as silence only when its single highest sample stays under `[dictation] silence_peak_threshold` (0.01, about −40 dBFS). One click or keyboard thump passes that check, and the engine is then free to invent text. The catalogue already carries Silero VAD and the Whisper engine accepts a `vad_model`, but every caller passes `None`. The meeting path's hallucination filter (known phrases, blank markers) never runs on dictation. There is no minimum length or end padding, so Parakeet can drop the last word or its full stop on a short take. The Whisper vocabulary prompt has no length cap, and Whisper keeps the end of an overlong prompt, which loses the first and most important terms.

This spec puts a two-stage speech gate in front of the engine, pads takes the way the engines need, and guards engine output against prompt echo, decoder loops and known hallucinations.

## Approach

**Stage 1, level gate (cheap, every take).** Measure the loudest 30 ms window, not the loudest sample. Remove the DC offset first, and leave out the post-release tail so a stop cue cannot count as speech. Silence is under −50 dBFS.

```rust
/// Loudest 30 ms window in dBFS, DC removed. `None` when the take is shorter than a window.
fn loudest_window_dbfs(pcm: &[i16], sample_rate: u32) -> Option<f32> {
    let win = (sample_rate as usize * 30) / 1000;           // 480 at 16 kHz
    if pcm.len() < win { return None; }
    let mean = pcm.iter().map(|&s| s as f64).sum::<f64>() / pcm.len() as f64;
    let mut best = 0.0f64;
    let mut acc: f64 = pcm[..win].iter().map(|&s| { let x = s as f64 - mean; x * x }).sum();
    best = best.max(acc);
    for i in win..pcm.len() {                                // sliding sum of squares
        let add = pcm[i] as f64 - mean; let sub = pcm[i - win] as f64 - mean;
        acc += add * add - sub * sub;
        best = best.max(acc);
    }
    let rms = (best / win as f64).sqrt() / i16::MAX as f64;
    Some(20.0 * rms.max(1e-9).log10() as f32)
}
const SILENCE_DBFS: f32 = -50.0;
```

**Stage 2, VAD (only when stage 1 passes).** Run the catalogued Silero model on the take, on the CPU, with threshold 0.4 and minimum speech 160 ms. These values keep whispers and a one-word "Ok". No speech found means silence. The VAD is optional: when its model is missing, the gate falls back to stage 1 alone and `dettivo doctor` says so. Download it with the speech model it serves. whisper.cpp's built-in VAD params can run it inside the Whisper engine; Parakeet needs a VAD pass before `recognize` (in the daemon or the engine, plan decides).

```rust
pub struct SpeechGate { pub silence_dbfs: f32, pub vad: Option<VadParams> }
pub struct VadParams { pub threshold: f32 /* 0.4 */, pub min_speech_ms: u32 /* 160 */ }

enum GateVerdict { Speech, Silence { reason: SilenceReason } } // Level | NoVoice
```

**Padding.** Pad every take to at least 1.5 s, and add 400 ms of trailing silence for Parakeet, so it emits the final word and its full stop.

```rust
fn pad_for_engine(mut pcm: Vec<i16>, engine: EngineKind, sr: u32) -> Vec<i16> {
    if matches!(engine, EngineKind::Parakeet) { pcm.extend(std::iter::repeat(0).take(sr as usize * 400 / 1000)); }
    let min = sr as usize * 3 / 2;
    if pcm.len() < min { pcm.resize(min, 0); }
    pcm
}
```

**Output guards (after recognition, before the text pipeline).**

- **Prompt cap.** `vocabulary_prompt` adds whole terms in list order until the next one would pass 500 characters. The terminating full stop rule stays.
- **Prompt echo.** Drop a segment whose normalised text equals the whole vocabulary prompt. Normalise by lowercasing, stripping punctuation and mapping spelled digits. Apply this only when the prompt has two or more terms, so dictating the one listed name still pastes it.
- **Decoder loops.** When a segment repeats an n-gram past a threshold, keep the text before the loop instead of failing or pasting the loop.
- **Known hallucinations.** Run the meeting hallucination filter on dictation output too: phrases the engines emit for silence, and blank markers.

```rust
fn is_prompt_echo(segment: &str, prompt_terms: &[String]) -> bool {
    prompt_terms.len() >= 2 && normalise(segment) == normalise(&prompt_terms.join(" "))
}
```

**Reporting.** A gated take ends `cancelled` with `reason = "silence"` and the stage (`level` or `voice`). It never reaches the engine, and the pill shows the existing "nothing heard" state.

## Acceptance

- **R1:** The level gate measures the loudest 30 ms window with DC removed, excludes the post-release tail, and treats a take under −50 dBFS as silence. A recorded click or keyboard thump in a silent room is gated; a quiet spoken word at −45 dBFS is not.
- **R2:** With Silero VAD available, a take that passes the level gate but has no voice (fan, music bed, breathing) is gated before the engine. A whispered sentence and a single "Ok" are not. Parameters: threshold 0.4, minimum speech 160 ms, CPU.
- **R3:** Without the VAD model the gate runs on the level stage alone, dictation keeps working, and `dettivo doctor` names the missing model and its download command.
- **R4:** Takes reach the engine padded to at least 1.5 s; Parakeet takes carry 400 ms of trailing silence. A fixture where the last word ends at the release keeps that word and its full stop.
- **R5:** The Whisper vocabulary prompt never exceeds 500 characters, keeps whole terms in list order, and stays terminated. A unit test covers a list that overflows the cap.
- **R6:** A take of silence with a two-term or longer vocabulary never pastes the vocabulary. A take that says the one listed term pastes it.
- **R7:** A decoder loop yields the text before the loop. Known silence hallucinations and blank markers are removed from dictation output. Both have unit tests on recorded engine output.
- **R8:** `silence_peak_threshold` is replaced by the new gate. Any config migration or deprecation is documented in `docs/config.md` and `docs/dictation.md`, and a changelog entry is added.

## Boundaries

- The gate decides only whether a take reaches the engine. It never trims speech out of a take that passes.
- Meetings keep their own windowing and filter. This spec only reuses the filter for dictation.
- No new user-facing knob beyond what replaces `silence_peak_threshold`.

## Quick commands

- `cargo test -p dettivo-session -p dettivo-speech -p dettivo-transcribe`
- `just build test lint`

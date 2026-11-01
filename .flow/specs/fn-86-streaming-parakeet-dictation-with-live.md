# Streaming Parakeet dictation with live text

## Goal & Context

Release-to-text time is the metric people feel. Today the whole take is transcribed after the release, so a 60 s dictation waits for 60 s of audio to go through the engine. Transcribing while the key is held leaves only the last second or so for after the release. The pill could also show the words as they form.

This spec first measures whether long takes are slow enough to matter. If they are, it streams Parakeet on a GPU during the hold, with the whole-take path kept as a guaranteed fallback.

## Approach

**Gate: measure first.**
- From history and the QA rig, plot `timings.transcribe_ms` against take length per engine and backend.
- Build streaming only if release-to-text for 30 s or longer takes exceeds 1 s on the reference GPU.
- Otherwise record the numbers in an ADR, close this spec as not needed, and stop.

**Streaming during the hold (GPU backends only).**
- A CPU stream re-encodes overlapping windows and costs more than it saves.
- Feed the engine 16 kHz audio from the take, which includes the pre-roll from the capture spec, in blocks of at least 160 ms.
- Parakeet buffers its own context: 5.6 s of left context, 1.04 s chunks, 1.04 s of lookahead.
- Start the stream only after the 300 ms minimum hold, so taps never reach the engine.

**Pause capping.** In the stream only, cap long silences to a 2.5 s head and a 0.3 s tail. Parakeet can drop the first word after a pause longer than its left context. The offline take keeps every sample.

**Finish on release.**
- Feed only the audio not yet sent, then ask for the final text.
- Fall back to whole-take offline recognition when any of these hold:
  - the stream failed;
  - it returned no text;
  - it missed a 1.5 s finish deadline;
  - it was fed more than 250 ms past the take's end.

```rust
enum StreamOutcome { Final(String), Fallback(FallbackReason) }   // Failed | Empty | Deadline | Overfed

async fn finish(stream: &mut EngineStream, take: &Take) -> String {
    match stream.finish(Duration::from_millis(1500)).await {
        StreamOutcome::Final(t) if !t.trim().is_empty() => t,
        other => { log_fallback(&other); engine.recognize(take.offline_request()).await }
    }
}
```

**Live text.**
- Partials are split into `stable` (will not change) and `tentative` (may change).
- They go out as a bounded `dictation.partial` event, which the pill renders in two weights.
- Partials are never inserted. Only the final text, after the mode step, is delivered.
- Partial events follow the transcript-in-events rule of `first_words`: bounded, never logged.

**Protocol.** Add `stream.open` / `stream.feed` / `stream.finish` / `stream.cancel` requests to the engine protocol for engines that report a `streaming` capability. Whisper does not; it keeps the whole-take path.

**Priority.** Dictation streams always come first. Meeting live windows yield while a dictation stream runs, and their backlog is capped.

## Acceptance

- **R1:** An ADR records `transcribe_ms` against take length per engine and backend. If the gate is not met, the spec closes there with that ADR.
- **R2:** With the gate met, a 60 s Parakeet take on the reference GPU reaches release-to-text in under 1 s at p50, measured by the QA rig. Short takes are no slower than today.
- **R3:** Every fallback condition (failure, empty, deadline, overfeed) yields the offline transcript, and the history item records `fallback = <reason>`. Each condition has a test.
- **R4:** A pause longer than 6 s mid-take does not lose the next word. A fixture covers it.
- **R5:** The pill shows stable and tentative partial text during the hold. Partial text never reaches insertion, history or logs.
- **R6:** Taps under 300 ms never open a stream. CPU backends never stream.
- **R7:** `docs/engines.md`, `docs/dictation.md` and `docs/osd.md` describe streaming, the fallback and partial events. A changelog entry is added.

## Boundaries

- Whisper dictation is unchanged.
- Meeting live transcription keeps its own windowing. It only gains the yield-to-dictation rule.

## Quick commands

- `cargo test -p dettivo-engine-parakeet -p dettivo-session -p dettivo-speech`
- `just build test lint`

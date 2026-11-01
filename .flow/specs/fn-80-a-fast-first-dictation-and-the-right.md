# A fast first dictation and the right model for each language

## Goal & Context

The first dictation of a session is the one people judge Dettivo by. Today the daemon preloads the selected model but runs no inference, so the first take pays for compiling the Vulkan shaders. The speech engine also ends after `[engines] stt_idle_seconds = 300`, so the first take after a five-minute break pays for a spawn, a load and the shader compile as well. Separately, first run picks a model without regard to the user's language, although the catalogue's models differ widely by language.

This spec makes the first take as fast as the tenth, sets the idle timeout from measurement, and recommends a model per language at first run.

## Approach

**Warm-up after load.** After every successful `load`, the supervisor sends one silent `recognize` (1.5 s of zeros) and discards the result. For an engine that streams (see the streaming spec), the warm-up also feeds a silent stream long enough to compile the streaming kernels. Skip it when the engine recognised anything in the last 60 s. The warm-up never blocks a real request: a real request that arrives first cancels the warm-up or queues ahead of it.

```rust
async fn after_load(&self, engine: &EngineHandle) {
    if engine.last_recognize_age() < Duration::from_secs(60) { return; }
    let req = RecognizeRequest { pcm: vec![0; 16_000 * 3 / 2], language: "en".into(), prompt: None, timestamps: false };
    let _ = engine.recognize_low_priority(req).await;     // result discarded, timing logged
    engine.mark_warm();
}
```

`engine.state` gains a `warm` transition (or a `warm: bool` field), so `dettivo speech status` and `doctor` show whether the next take will be cold.

**Idle timeout from data.** Measure, per engine and backend:
- first-take latency cold (spawn, load, warm-up);
- first-take latency warm-loaded but not warmed up;
- latency fully warm;
- VRAM held while idle.

Pick the `stt_idle_seconds` default from those numbers and record the decision in an ADR. Candidates are 300 s, 1800 s and "never unload while the session is graphical". The key stays configurable.

**GPU thread count.** When a speech engine runs on a GPU backend, measure the CPU thread count. Only the decoder runs on the CPU there, so extra spinning workers can stall it. Set the default from the measurement.

**Language-aware model recommendation.**
- Run FLEURS (or the QA rig's language set) on the exact model files in the catalogue: whisper, the three Parakeet models, and any model added later.
- Compute WER per language and treat differences under 1.5 points as ties, preferring the faster model.
- Store the result as a table in the catalogue: `recommended_for = [{ language = "de", rank = 1 }, ...]`.
- First run and `dettivo speech providers --language <code>` read it: "For German, parakeet-v3 is recommended (WER 4.1), large-v3-turbo is close (4.9)".

```toml
[[model]]
id = "parakeet-v3"
# ...
[[model.recommended_for]]
language = "de"
wer = 4.1
```

## Acceptance

- **R1:** After a model loads, one silent warm-up runs before the first real take unless the engine recognised within 60 s. A real request is never delayed by more than the warm-up already in flight.
- **R2:** On the reference machine (Vulkan), the first dictation after daemon start is within 20% of a warm dictation's release-to-text time. The QA rig measures this and records it in the benchmark report.
- **R3:** `dettivo speech status` and `dettivo doctor` show whether the engine is cold, loaded or warm.
- **R4:** An ADR records the measured cold, loaded and warm latencies and idle VRAM per engine. It sets the `stt_idle_seconds` default; the config docs follow.
- **R5:** The GPU-backend thread default is set from a measurement recorded in the benchmark report.
- **R6:** The catalogue carries per-language recommendations from a recorded benchmark run. First run preselects the recommended model for the user's chosen language, and `dettivo speech providers --language <code>` lists the ranking with WER.

## Boundaries

- No change to which model is selected for an existing install. Recommendations apply at first run and in listings only.
- The warm-up never produces history items, events with transcript text, or log lines with text.

## Quick commands

- `cargo test -p dettivo-speech -p dettivod`
- `just build test lint`

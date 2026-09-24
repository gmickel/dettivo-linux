# 0003. Inference runs in supervised engine processes

Status: Accepted 2026-09-03

## What this gives you

A crash inside a speech or language model never takes your hotkeys down, GPU memory comes back the moment an engine goes idle, and a CUDA build is a drop-in binary rather than a rebuild of the whole product.

## Situation

whisper.cpp and llama.cpp each vendor their own copy of ggml and collide on symbol names when linked into one binary. Native inference libraries can segfault on a bad model file or driver. The Windows port fought idle VRAM retention for weeks because a loaded CUDA context stays resident inside the process that created it. Every engine also needs a way to be run from the command line for benchmarks and fixtures.

## Decision

Each engine is its own binary, spawned and supervised by the daemon: `dettivo-engine-whisper`, `dettivo-engine-parakeet`, `dettivo-engine-llm`, `dettivo-engine-diarize`. They speak one length-prefixed JSON protocol with binary PCM attachments over a socketpair, defined once in `dettivo-engine-proto`. The daemon starts an engine on first need, keeps it warm, and terminates it after an idle timeout (5 minutes for speech, 10 for the language model, both configurable). Every engine binary also accepts `--wav` or `--prompt` and prints the protocol's JSON.

## Consequences

- IPC cost per request is one copy of 16 kHz PCM, under a millisecond for a 15 second dictation.
- A crashed engine surfaces as a session error with its last stderr lines redacted; the supervisor restarts it with backoff and marks it degraded in `system.health` after three crashes in a row.
- Engine binaries are discovered on `PATH` or a configured directory, so a `dettivo-engines-cuda` package can override the Vulkan defaults file by file.
- Benchmarks, WER fixtures and `dettivo doctor` run the exact inference path the daemon uses.
- The cost is a supervisor with its own tests and a soak check for leaked engine processes.

## Automatic Whisper load recovery (2026-09-22)

Meetings can continue on CPU when an automatic Whisper model load kills the GPU engine. The supervisor retries a crashed process or broken load response once in a fresh process with the CPU backend. Normal returned Vulkan load errors still use the engine's existing CPU fallback.

The supervisor remembers that automatic model selection for subsequent chunks. It tries the GPU again after idle unload, a different selection, or daemon restart. An explicit GPU backend remains strict. CPU retry failures are returned; malformed load responses count toward the existing crash backoff. Engine status reports the CPU fallback reason. This covers model loading; a crash during recognition still fails that request.

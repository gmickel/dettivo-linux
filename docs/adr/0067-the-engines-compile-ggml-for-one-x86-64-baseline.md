# 0067. The engines compile ggml for one x86-64 baseline

Status: Accepted 2026-09-24

## What this gives you

The speech engines in the release package run on every x86-64 machine made since 2013, whichever CI runner built them. A build restored from a cache runs on the next runner too.

## Situation

The first CI runs on the public repository failed the audio-import checks on some runs and passed them on others. The daemon's log showed `dettivo-engine-whisper` crashing right after it reported ready, three times in a row, with nothing on stderr: an illegal instruction. whisper-rs-sys leaves `GGML_NATIVE` at ggml's default, which is on, so ggml was compiled for the CPU of whichever runner built it. The compiled-build cache then restored that build on runners with older CPUs. The release tarball comes from the same kind of build, so `dettivo-bin` would have shipped a Whisper engine that crashes on any CPU older than the build runner's.

parakeet-cpp-sys already set `GGML_NATIVE=OFF`. ggml also turns every instruction-set option off when `SOURCE_DATE_EPOCH` is set, which `makepkg` does, so the AUR source build would have fallen back to plain SSE.

## Decision

`.cargo/config.toml` sets `GGML_NATIVE=OFF` with `GGML_AVX`, `GGML_AVX2`, `GGML_FMA`, `GGML_F16C` and `GGML_BMI2` on, for every cargo build in the workspace. whisper-rs-sys and parakeet-cpp-sys forward `GGML_*` variables to CMake, so the Whisper and Parakeet engines get the same baseline on a laptop, in CI and under `makepkg`. AVX-512 stays off.

## Consequences

A machine with AVX-512 no longer gets it from a local build, so the Whisper and Parakeet engines give up whatever that instruction set was worth on that machine. A CPU without AVX2, meaning older than 2013's Haswell, cannot run the engines.

llama-cpp-sys-2 sets the instruction sets itself, after the environment and from the Rust target features, so the language-model engine keeps the plain x86-64 baseline it had before. Raising it would mean raising the Rust target for the whole workspace, and this record leaves that open.

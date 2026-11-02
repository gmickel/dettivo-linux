# whisper-rs-sys 0.15.0, patched

This is the crates.io release of [`whisper-rs-sys`](https://crates.io/crates/whisper-rs-sys) 0.15.0 (Unlicense), with the whisper.cpp and ggml sources it bundles (MIT). The workspace uses it through `[patch.crates-io]` in the root `Cargo.toml`, so `whisper-rs` 0.16 builds against it unchanged. [ADR 0080](../../docs/adr/0080-whisper-cpp-carries-a-dtw-median-filter-guard.md) records why.

## What differs from the release

- **The DTW guard.** `whisper_exp_compute_token_level_timestamps_dtw` in `whisper.cpp/src/whisper.cpp` returns early, leaving the window's tokens with `t_dtw = -1`, when a decoding window has no more audio tokens than the median filter is wide. whisper.cpp's `median_filter` asserts on such a window and aborts the process, which any clip of about 0.10 to 0.14 s of speech, or a chunk whose last window is that short, triggers. Upstream `master` still carries the assertion. The patch is the block marked `Dettivo patch`.
- **The VAD stream.** `whisper_vad_detect_speech` runs its body with `reset`, and a new `dettivo_vad_detect_speech_more` runs it without one: the Silero LSTM state carries over and the new windows' probabilities are appended, so audio fed in whole 512-sample windows ends with the probabilities one call over all of it computes. The dictation speech check uses it to hear a take while it is captured ([ADR 0097](../../docs/adr/0097-the-speech-check-hears-the-take-while-it-is-captured.md)). The functions are marked `Dettivo patch (ADR 0097)`; the new one is defined `extern "C"` in `whisper.cpp` and declared in `crates/dettivo-engine-whisper/src/vad.rs`, so `whisper.h` and the bindings are unchanged.
- **Fewer ggml backends.** The backend directories this workspace never builds are left out: `ggml-cann`, `ggml-cuda`, `ggml-hexagon`, `ggml-hip`, `ggml-metal`, `ggml-musa`, `ggml-opencl`, `ggml-rpc`, `ggml-sycl`, `ggml-webgpu`, `ggml-zdnn` and `ggml-zendnn`. ggml's CMake enters a backend directory only when that backend is switched on, and the Whisper engine builds the CPU backend and, with its `vulkan` feature, Vulkan.

- **The patched file rebuilds.** The release's `build.rs` copies the sources into `OUT_DIR` once; this copy also watches `whisper.cpp/src/whisper.cpp` and refreshes it there whenever it differs, so an edit to the patch is built without `cargo clean`.

- **Quiet lints.** `Cargo.toml` allows rustc and clippy warnings, which a path dependency would otherwise print on every build.

Everything else is the release as published.

## Moving to a new release

Copy the new release's files over this directory, drop the same backend directories, apply the `Dettivo patch` blocks again (the DTW guard unless upstream has fixed the assertion), and run `cargo test -p dettivo-engine-whisper`: its `a_window_too_short_for_the_dtw_filter_does_not_abort` test exercises the guard, and `a_take_heard_as_it_is_captured_finds_the_whole_take_speech` the VAD stream.

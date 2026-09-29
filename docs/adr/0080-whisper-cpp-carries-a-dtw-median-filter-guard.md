# 0080. whisper.cpp carries a DTW median-filter guard, and one bad chunk no longer fails a transcript

Status: Accepted 2026-09-29, amends [0074](0074-whisper-times-every-word-by-dtw-for-speaker-labelling.md)

## What this gives you

A meeting, an import or a re-run finishes its transcript even when a stretch of the audio trips the speech engine. A clip whose last decoding window lasted between about 0.10 and 0.14 seconds used to abort the Whisper engine; the patched engine transcribes it. If the engine still dies twice on the same stretch, that stretch becomes one line, "[Not transcribed: speech recognition failed on this part]", at its place in the transcript, and the rest of the meeting is kept. An engine that crashes on the GPU, for instance while a game holds the GPU's memory, carries on on the CPU. Every engine crash now logs the engine's last stderr lines.

## Situation

On 2026-09-29 a 57-minute two-track meeting failed its transcript at chunk 193 of 262 with `engine crashed`, twice, and was left `failed` with no transcript. The audio was intact.

The core dump, symbolized against a build with line tables, stopped in `median_filter` inside `whisper_exp_compute_token_level_timestamps_dtw`. [ADR 0074](0074-whisper-times-every-word-by-dtw-for-speaker-labelling.md) turned on whisper.cpp's DTW word timing in 0.4.0. That code asserts that a decoding window holds more audio tokens than the median filter is wide (seven), and a window of 0.10 to 0.14 seconds holds fewer. The assertion aborts the process through whisper.cpp's log, which the engine does not print, so the crash left no message. It depends only on the audio, so the retry crashed on the same chunk. About 0.12 seconds of the JFK fixture reproduces it on the CPU and on Vulkan, and upstream `master` still carries the assertion. `whisper-rs-sys` builds whisper.cpp only from the copy it ships, with no way to substitute a source.

The same investigation found two smaller gaps. The transcription job retried a crashed chunk once and then failed the whole transcript. The supervisor moved a Whisper engine to the CPU only when its GPU load failed, not when it died mid-request, which is what ggml does when a game takes the GPU's memory.

## Decision

- **Vendor and patch `whisper-rs-sys`.** `third_party/whisper-rs-sys` is the 0.15.0 release, without the ggml backends the workspace never builds, used through `[patch.crates-io]`. Its whisper.cpp returns from the DTW function before allocating anything when `n_frames/2 <= medfilt_width`, leaving that window's tokens with `t_dtw = -1`. Its README records the patch and how to move to a new release. `dettivo-engine-whisper`'s `dtw_short_window` test cuts short clips from the JFK fixture and runs them through the engine; without the patch it aborts with signal 6.
- **Time words without a DTW time from their segment.** In DTW timing, a word whose tokens have no DTW time takes its segment's span, kept in order and never past the next word.
- **Mark a chunk the engine cannot transcribe.** When a chunk's retry crashes too, the job gives the chunk one segment carrying `GAP_TEXT` as a single word, placed between the chunk's overlaps so the merger keeps it whole, and goes on. A job allows three such gaps; the fourth fails the job as before, because four chunks lost to crashes point at a broken engine. Other engine errors still fail the job. The live transcript already skipped a failed window and is unchanged.
- **Fall back to the CPU after a crash on the GPU.** When an engine loaded on a GPU with the `auto` backend crashes on a request, the supervisor marks that load for the CPU, as it already did for a failed Whisper load, so the retry and the requests after it run on the CPU until the engine unloads or the selection changes. An explicit `vulkan` or `cuda` choice is never moved.
- **Log a crash's stderr.** The supervisor's crash warnings carry the redacted tail of the engine's stderr.

## Consequences

- The repository carries about 7 MB of third-party C, C++ and Rust under `third_party/`. The file-length lint allows `third_party/**`, the notice lint leaves path crates out of its crate table, and `NOTICE.md` names the vendored crate. Moving `whisper-rs` to a release that bundles a newer whisper.cpp means refreshing the copy and reapplying the patch, which the README spells out and the `dtw_short_window` test checks.
- The words of a window whisper.cpp could not align carry their segment's span, a coarser time than DTW gives. Such windows are at most 0.14 seconds of audio.
- A transcript can now complete with up to three marked gaps where it used to fail; the gap line shows in the app, in exports and in the analysis's input.
- After a GPU crash, the engine stays on the CPU until it unloads, which is slower than the GPU for the rest of that run.

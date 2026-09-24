# 0022. Imports decode in-process through Symphonia and every long transcription runs through one chunked pipeline with a timestamp merger

Status: Accepted 2026-09-04, amended 2026-09-06 by 0048 (the near-silence guard judges frames; the seam rule holds only inside shared audio)

Amended by [0050](0050-private-transfers-authenticated-adapters-and-isolated-qa-processes.md), which enforces private transfer directory and file modes independently of the caller's umask.

## What this gives you

`dettivo history import call.m4a` turns an hour of audio in any of the contract's formats into one history item with segments and word timestamps, with a progress line per five-minute chunk, and no `ffmpeg` on the machine. The same pipeline transcribes a re-run, and the meeting spec inherits a merger that is already proven against goldens, so the offline half of meetings does not start from zero.

## Situation

The history spec landed `transcripts.import` as a single-take path: WAV read directly, anything else through an `ffmpeg` child process, one `recognize` call for the whole file. Whisper's context is thirty seconds and its attention degrades on long inputs; the macOS port cuts long audio into 300 s chunks with 2 s of overlap and a 5 s safety margin (`ChunkedTranscriptionConfig`) and drops the overlap of the later chunk, keeps a list of filler phrases Whisper writes into silence (`MeetingTranscriptHallucinationFilter`), and gates speech at an RMS floor of 0.0065 (`speechActivationFloor`). FR-S6 fixes the content types (`audio/wav`, `audio/mpeg`, `audio/mp4`, `audio/m4a`, `audio/aac`, `audio/x-caf`, `audio/caf`, `audio/aiff`, plus `audio/flac` and `audio/ogg` on Linux), and a daemon that shells out to a decoder it cannot pin is a dependency it cannot prove in CI. Two questions were open: where decoding lives, and how two chunks that both heard the same two seconds become one transcript without a repeated or a lost word.

## Decision

`crates/dettivo-audio/src/decode.rs` decodes every FR-S6 type in-process through Symphonia 0.6 (features `wav`, `aiff`, `caf`, `isomp4`, `aac`, `alac`, `mp3`, `flac`, `ogg`, `vorbis`, `pcm`) and resamples to 16 kHz mono through a windowed-sinc `rubato` resampler, packet by packet, so a one-hour file never sits in memory whole. The declared content type picks the probe hint and the set of containers it may turn out to be; a file whose container is not in that set is refused naming both (`the upload declares audio/flac but its container is mp3`). `ffmpeg` is out of the daemon. Opus inside Ogg has no decoder here and is refused naming the codec.

`crates/dettivo-transcribe` is the pipeline every import and re-run runs through, and the meeting spec reuses its merger:

- The chunker plans fixed windows of `[transcribe] chunk_seconds` (300), each starting `overlap_seconds` (2) before the previous one ended, and cuts each window at the centre of the quietest 50 ms inside its last `safety_margin_seconds` (5), so a chunk rarely ends mid-word. A shorter file is one chunk on the same path.
- The merger reconciles consecutive chunks by timestamp: inside the overlap the later chunk's words win from the midpoint (the earlier chunk keeps the words that start before it, the later chunk those that start at or after it), a word the later chunk repeats within 250 ms of one the earlier chunk kept is dropped, a run of up to twelve words at the seam that the earlier chunk's tail already said is dropped once, and segment boundaries snap to the words that remain. An engine without word timestamps (Whisper) gets words spread evenly over each segment so the same rule applies; those synthetic words never leave the merger, and `transcripts.get` carries `words` only when the engine aligned them (Parakeet).
- The filters are the macOS lists verbatim, canonicalised the macOS way, applied per segment before the text is stored; the near-silence guard keeps a chunk whose RMS is under `silence_rms_floor` (0.0065) away from the engine, so a silent hour stores an empty transcript with `notice = silent` and never invented text.
- The job checks `supports_timestamps` before the first chunk and fails naming the provider when it is false or when a chunk comes back with text but no segments; a crashed engine is retried once per chunk, a second crash fails the item naming the chunk (`chunk 7 of 12 failed`) and keeps the earlier chunks' text; `transcripts.cancel { ref }` stops the job between chunks and leaves a cancelled item with what was finished.

The daemon stores segments and words on the item through migration `0002-segments` (a JSON column, absent on session dictations), decodes into the item's own `microphone.wav` when audio is retained and into a temporary file otherwise, refuses a file over `[history] max_import_seconds` (14400) after the probe and again during decoding when the container lied, and reports `job.progress` with `chunks_done`, `chunks_total` and `stage` (`decoding`, `transcribing`, `merging`, `done`, `failed`, `cancelled`).

## Consequences

- The decoder is a pinned Rust dependency built into every binary and exercised by checked-in fixtures in every format (`scripts/fixtures/gen-decode-fixtures.sh` made them once from one tone); a format Symphonia does not decode is a compile-time fact, not a machine-dependent one. The cost is 2 to 3 MB of binary and Symphonia's Mozilla Public License 2.0, compatible with this repository's MIT license as a linked dependency.
- The merger's rule is the spec's, not macOS's drop-overlap: it keeps a word the earlier chunk cut off and drops the fragment the later chunk started with. `crates/dettivo-transcribe/fixtures/overlap/three-chunks.golden.json` pins it, and `dettivo-qa pipeline import-merge` proves it against the real engine in CI. Its bound is the timestamp precision of the engine: ADR 0018's measurement (Parakeet p95 of 308 to 347 ms) is why the seam run rule exists beside the 250 ms window.
- One job runs at a time per import slot and the engine is busy for the whole run; a five-minute chunk of tiny.en on a CPU runner takes seconds, of large-v3-turbo on a GPU tens of seconds, so the CLI follows progress instead of waiting on one call, and the MCP `import_audio` tool answers at once with the running job as the contract states.
- `transcripts.cancel` is a Linux addition (registered with the `job.progress` fields and the `words` field in `docs/api/linux-deltas.md`) because the contract has no verb to stop a job; a cancelled item reports `cancelled` in `transcripts.list` and carries `error_code = cancelled`.
- The chunked pipeline does not run the live dictation session: a take is bounded by `[dictation] max_duration_seconds` (300) and stays one `recognize` call.

## Speech-aware recognition windows (2026-09-23)

Automatic recognition can follow language changes without selecting a language from a long silent opening. The default `transcribe.chunk_seconds` is now 30; an explicit value remains honored. Existing two-second overlap and quiet-point cuts keep context across window boundaries.

A window still needs a 200 ms frame at the configured speech floor to reach the engine. Its quiet edges are then trimmed at one tenth of that floor, retaining 400 ms of padding so quiet onsets and tails around speech survive. Original duration, chunk progress, and absolute segment and word timestamps remain intact. The merger uses the retained audio ranges when deciding whether two results overlap.

Each window receives the requested language: `auto` detects again, while an explicit language remains explicit. No meeting-wide language lock is introduced. A switch inside one window can still be imperfect. The live three-second preview keeps its existing path; this change applies to final meeting transcription and file imports/re-runs. Stored meetings are unchanged until explicitly reprocessed.

Shorter windows add recognition and language-detection calls. CPU fallback can therefore take longer on speech-heavy input; silence trimming can offset the extra work on GPU. The window length remains configurable, and performance claims require measurements on the chosen backend rather than audio duration alone.

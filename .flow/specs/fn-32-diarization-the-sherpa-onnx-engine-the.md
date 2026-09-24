# Diarization: the sherpa-onnx engine process, the post-meeting speaker pass and speaker names

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-25 row: "`diarization-engine-sherpa-onnx-post-pass` | phase 4 | depends on S-24, S-05 | FR-G4, G5, T6 | Diarization fixture with expected speaker turns; re-run test; engine crash isolation test"
> masterplan FR-G4: "Diarization runs as a post-meeting pass on the system-audio track in the diarization engine process with a downloadable model set, labels the microphone track `You`, and assigns a remote speaker label only when diarized coverage of the row is at least 0.25 and the winning speaker holds at least 0.60 of its speech, otherwise the row stays unlabeled. Microphone-only meetings are treated as room audio and diarize the whole track. It can be re-run from meeting actions with chunk-based progress notices."
> masterplan FR-G5: "Speakers can be renamed; renames apply across the meeting and are remembered for future suggestions."
> masterplan T6: "Sherpa-ONNX only for diarization, CPU ... No ggml diarization exists; the pyannote plus 3D-Speaker pipeline is proven on Windows and small (about 47 MB); CPU is fine for a post-meeting pass. Isolated in its own engine process so ONNX Runtime never touches the daemon or the ggml engines."
> masterplan 8.8: "Binaries: `dettivo-engine-whisper`, `dettivo-engine-parakeet`, `dettivo-engine-llm`, `dettivo-engine-diarize` ... Messages: `load`, `unload`, `recognize`, `generate`, `diarize`, `cancel`, `status`; events: `progress`, `partial`, `loaded`, `error` ... an engine crash surfaces as a session error ... three crashes in a row mark the engine degraded ... every engine binary accepts `--wav <file> --model <path> --json`."
> masterplan Appendix C: "Diarization | `diarization` | sherpa-onnx-pyannote-segmentation-3-0.tar.bz2 plus 3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx (upstream path typo is real) | about 47 MB | pyannote segmentation 3.0 MIT; 3D-Speaker Apache-2.0"
> masterplan NFR-4: "Meeting throughput, GPU tier | ≥ 10× realtime transcription; diarization ≥ 4× realtime | Engine benchmark fixtures"
> masterplan 8.10 Meeting, live: "Speakers are labelled by source now. Names arrive after the diarization pass."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

A finished meeting learns who spoke. `dettivo-engine-diarize`, today a stub that exits 2, becomes the fourth engine process: sherpa-onnx's offline speaker diarization (pyannote segmentation 3.0 for the turns, 3D-Speaker ERes2Net for the embeddings, CPU only per T6) behind the engine protocol's existing `diarize` message, with the model set in the catalogue, downloaded and verified like every other model. When a meeting's finalisation completes, the daemon runs the post-pass over the system track (the whole microphone track for a room-audio meeting), assigns a speaker to each finalised segment under the macOS coverage and share rule or leaves it unlabeled, labels microphone segments `You`, stores the speakers with their talk time, and reports chunk progress. Speakers can be renamed across the meeting through `meetings.speakers.rename`, and the names are remembered as suggestions for the next meeting. A diarization fixture with expected turns, a re-run test and an engine crash that never reaches the daemon are the evidence. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Engine (`crates/dettivo-engine-diarize`): the sherpa-onnx offline speaker diarization pipeline through its C API, bound by the `sherpa-rs` crate pinned to one release (an in-repo `sherpa-onnx-sys` if that binding's build proves unfit, decided in the ADR, the way ADR 0026 chose `llama-cpp-2`); ONNX Runtime stays linked into this binary alone. `load` names the model directory (the segmentation model and the embedding model inside it) and the thread count; `diarize` takes the `pcm16k` attachment with `DiarizeParams.speakers` as the cluster count when known, streams `progress` events per segmentation chunk (the pipeline's own chunking, reported as `completed` and `total`), and answers `DiarizeResult.turns` (`start_ms`, `end_ms`, `speaker` as `SPEAKER_00`, `SPEAKER_01`, ...); `cancel` ends a run between chunks; `status` and `unload` as the other engines. CLI mode `--wav <file> --model <dir> --json [--speakers n]` prints the same result, so the fixture, the benchmark and `dettivo doctor` share the path. `DETTIVO_FORCE_CPU` is honoured trivially; `loaded` reports `cpu`. The binary runs through `dettivo-engine-proto::host` like the speech engines (one request at a time, progress events between chunks). [paraphrase]
- Supervisor (`crates/dettivo-speech`): a `DiarizeEngine` proxy beside `WhisperEngine` and `ParakeetEngine` over the same `Supervisor` slot mechanics (spawn on first need, `stt_idle_seconds` reaping, crash recorded, backoff, degraded after three crashes, `engine.state` transitions); the binary is discovered by name on `PATH` or `[engines] directory`. `speech.engines` and `dettivo doctor` gain the diarize row with its model readiness. [paraphrase]
- Post-pass (`crates/dettivo-meeting`, `diarize.rs`): a job started by the daemon when finalisation lands `completed` and `[meetings.diarization] auto` is on, or by `meetings.diarize` (re-run). It reads the system track (or every microphone take of a room-audio meeting, concatenated on the meeting clock with the gaps as silence), sends it in one `diarize` request, and assigns per finalised remote segment: coverage is the fraction of the segment's span inside any turn; the winner is the speaker with the most overlap; the label is assigned only when coverage ≥ `min_coverage` (0.25) and the winner's share of the covered span ≥ `min_speaker_share` (0.60), otherwise the segment stays unlabeled. Microphone segments get `You` (speaker id `you`); in a room-audio meeting they are assigned like remote ones. The resulting speakers (id, label, colour index in order of first appearance, `talk_ms` summed over assigned segments, coverage of the track) land on the row; `job.progress` carries `stage = diarizing` with the chunk counts. [paraphrase]
- Store: migration `0006-speakers`: `meeting_speakers` (meeting id, `speaker_id`, `name`, `color_index`, `talk_ms`), the segment columns `speaker_id` and `speaker_confidence` (the winner's share) beside the existing `speaker`, a `diarization` JSON on the row (`status` `none|running|ready|failed|unavailable`, `coverage`, `engine`, `model`, `ran_at`, `error`), and `speaker_names` (name, `last_used_at`, `uses`) for suggestions. A rename updates the speaker row and every segment's `speaker` text so exports and FTS carry the name. [inferred]
- Catalogue: `crates/dettivo-speech/catalogue/v1.toml` gains provider `diarize`, model `diarization`, `kind = "diarization"`, with a `files` list (the segmentation tarball, extracted to its `model.onnx`, and the embedding `.onnx`, each with its own SHA-256, size and license); the downloader learns a multi-file model and a `tar.bz2` unpack step, verifying every file. `speech.models.status|download|cancel|delete` accept `provider = "diarize"` and the speech selection methods keep hiding it from the dictation choice, as they hide the LLM models. [inferred]
- QA: `crates/dettivo-qa/fixtures/diarization/two-speakers.wav` (two TTS voices, alternating, six turns, one overlap) with `two-speakers.turns.json` (expected turns and the speaker count); `dettivo-qa pipeline diarization` runs the engine in CLI mode and scores the diarization error rate against the turns (pass under 0.20 with exactly two speakers), writing `diarization-report.json` under `qa-evidence/<run>/diarization/`; the daemon test `crates/dettivod/tests/meetings_diarize.rs` finalises a mock meeting with that fixture as the system track and asserts the labels, the coverage rule on a segment that straddles two turns, the `You` labels, the room-audio path, the re-run and the rename; `dettivo-qa pipeline diarization --bench` records realtime factor for NFR-4. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Linux additions with fixtures under `crates/dettivo-proto/fixtures/meetings/`: `meetings.diarize { meeting_id, speakers? }` (a job; `CONFLICT` `diarizationRunning` while one runs, `CONFLICT` `meetingNotCompleted` before finalisation, `NOT_FOUND` `modelMissing` naming the download command); `meetings.speakers.list { meeting_id }`; `meetings.speakers.rename { meeting_id, speaker_id, name }` (empty name restores the label; `INVALID_PARAMS` over 64 characters); `meetings.speakers.suggest { prefix?, limit? }`. `meetings.start` and `transcripts.import { target_kind: "meeting" }` gain `expected_speakers?` and `diarize?` (default from config). [inferred]
- `meetings.get` and `transcripts.get { kind: meeting }` carry `speakers[]` (`speaker_id`, `name`, `color_index`, `talk_ms`), `diarization` (the block above) and per segment `speaker`, `speaker_id`, `speaker_confidence`; `meetings.list` items carry `speaker_count`. The `meeting.state` payload gains `diarization_status`; `job.progress` gains `stage`. Registered in `docs/api/linux-deltas.md`. [inferred]
- Engine protocol: `load` gains `threads?`; `progress` on `diarize` is registered with a fixture under `crates/dettivo-engine-proto/fixtures/`; `DiarizeParams`, `SpeakerTurn`, `DiarizeResult` stay as defined. [paraphrase]
- Config: `[meetings.diarization] enabled = true`, `auto = true`, `model = "diarization"`, `min_coverage = 0.25`, `min_speaker_share = 0.60`, `max_speakers = 0` (0 lets the clustering decide), `clustering_threshold = 0.5`; `[engines.diarize] threads = 0` (0 is the core count, capped at 4). [inferred]
- CLI: `dettivo meetings diarize <id> [--speakers n]`, `dettivo meetings speakers <id>`, `dettivo meetings speakers rename <id> <speaker-id> <name>`, `dettivo speech download --provider diarize --model diarization`, `dettivo-engine-diarize --wav --model --json`. [inferred]

## Edge Cases & Constraints

- Model not downloaded: the post-pass records `unavailable` with the download command on the row and the meeting stays `completed`; nothing blocks the transcript. [inferred]
- Engine crash mid-pass: the supervisor records it, the job fails with the engine's last redacted stderr line, the row says `failed`, the daemon and its socket keep running, and `meetings.diarize` retries after the backoff; three crashes mark the engine degraded in `system.health`. [paraphrase]
- One speaker on the remote track: every remote segment gets `SPEAKER_00`; the coverage rule still leaves silence-heavy segments unlabeled. [inferred]
- Overlapping speech: the winner rule decides; a segment covered 50/50 stays unlabeled. [paraphrase]
- Re-run after renames: new labels map onto existing speaker rows by id when the count matches, otherwise renames are kept as suggestions and the row's speakers are rebuilt. [inferred]
- A meeting deleted while diarizing: the job is cancelled through `cancel` and the row goes per policy. [inferred]
- Long meetings: one request, chunk progress; `cancel` returns within one chunk. [paraphrase]

## Acceptance Criteria

- **R1:** `dettivo-engine-diarize` in CLI mode and through the protocol (fixtures for `load`, `diarize` with `progress`, `cancel`, `status`, `unload`) diarizes `two-speakers.wav` under a diarization error rate of 0.20 with two speakers, on the CPU in CI; `--bench` records the realtime factor. Errors: a missing model file is `model_missing` naming the path; a bad attachment is `bad_request`. [paraphrase]
- **R2:** `meetings_diarize`: a mock meeting whose system track is the fixture finalises, the post-pass runs when `auto` is on, remote segments carry the expected speakers under the 0.25 coverage and 0.60 share rule (the straddling segment stays unlabeled), microphone segments are `You`, a microphone-only meeting diarizes the whole track, speakers carry `talk_ms`, and `job.progress` reports `diarizing` chunks. Errors: model absent leaves `diarization.status = unavailable` with the command and the transcript intact. [paraphrase]
- **R3:** Re-run: `meetings.diarize` on a completed meeting replaces the assignment with progress notices and refuses while running or before finalisation with the named `CONFLICT`. Errors: as stated. [paraphrase]
- **R4:** Crash isolation: a crash injected into the engine during the pass (the same injection the supervisor's whisper crash test uses) fails the job with the redacted reason, leaves the daemon answering `system.ping` and the meeting `completed`, and a third crash marks `engine.state` degraded. Errors: as stated. [paraphrase]
- **R5:** Rename: `meetings.speakers.rename` updates every segment of the meeting, the `txt`, `md`, `srt`, `vtt` and `json` exports and FTS carry the name, `meetings.speakers.suggest` returns it ordered by recency for the next meeting; every new `meetings.*` fixture passes against the live daemon and the catalogue downloads and verifies the two-file model set from the fixture model server (a checksum mismatch quarantines it). Errors: an over-long name is `INVALID_PARAMS` naming the limit. [paraphrase]
- **R6:** `docs/meetings.md` gains the diarization section (the rule, the labels, re-run, renames), `docs/engines.md` the fourth engine, `docs/models.md` the model set; the config keys are documented and printed by `config print-default`; the CLI verbs are documented; an ADR records the sherpa-onnx engine, the binding choice and the assignment rule. Errors: as stated. [paraphrase]

## Boundaries

- No notes, analysis, search over speaker names beyond FTS, or delete policy changes (S-26). [paraphrase]
- No GUI: the speaker rename popover, talk-time bar and swatches are S-27. [paraphrase]
- No live diarization; labels arrive after the pass, as the live baseline's footer states. [paraphrase]
- No GPU path for diarization (T6). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Names on a transcript are what make a meeting readable later; the Windows port proved the pyannote and 3D-Speaker pair, and running it in its own process keeps ONNX Runtime away from the ggml engines and the hotkey daemon. [paraphrase]

## Strategy Alignment

- **Local engines and performance:** a fourth isolated engine process with a verified catalogue model set, benchmarked for NFR-4. [strategy:Local engines and performance]
- **Complete speech workflows, proven by drives:** the diarization fixture, the re-run test and the crash test prove speaker labels end to end before the GUI shows them. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |

# Speech catalogue: providers, selection, verified downloads and preload

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-06 row: "`speech-catalogue-selection-download-verify-preload` | phase 2 | depends on S-05 | FR-E4, E5, FR-P3; `speech.providers.list`, `speech.selection.*`, `speech.models.*` | Fixture model server tests; checksum mismatch quarantine test; `speech.*` fixtures"
> masterplan FR-E4: "The model catalogue is a versioned manifest listing provider, model id, display name, size, source URL, SHA-256, license and redistribution note. Downloads verify the checksum, resume partial files, report progress events and can be cancelled and deleted."
> masterplan FR-E5: "Model readiness is reported per provider and model; the GUI, CLI and bar panel show the same readiness source."
> masterplan FR-P3: "Downloaded models are verified against pinned SHA-256 values before use; a mismatch quarantines the file."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

The daemon knows which speech models exist, which are on disk and verified, which one is selected for dictation and for meetings, and can download a model with a pinned checksum, resumable, with progress on the event stream. The GUI, the CLI and the bar panel read one readiness source. This spec delivers the versioned catalogue manifest, the on-disk model layout with per-model manifests (ADR 0005), the `speech.providers.list`, `speech.selection.get` and `speech.selection.set` methods the contract already adopted from Windows, the Linux additions `speech.models.download`, `speech.models.cancel`, `speech.models.delete` and `speech.models.status`, the `model.download` event topic, the `dettivo speech` CLI verbs, and the preload on model selection that the supervisor spec left waiting for a catalogue. [paraphrase]

Model ids follow the macOS and Windows aliases (`tiny.en`, `base`, `small`, `medium`, `large-v3`, `large-v3-turbo`, `silero-vad`, `parakeet-v2`, `parakeet-v3`) so `speech.selection.set` values are portable; unlike the other ports, Linux records a checksum and a license per model. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 60% [paraphrase], 40% [inferred] -->

- Catalogue: a versioned TOML manifest compiled into `dettivo-speech` (`catalogue/v1.toml`) with one entry per model: `provider`, `id`, `display_name`, `size_bytes`, `url`, `sha256`, `license`, `redistribution` note, `kind` (`stt`, `vad`, `speaker`), `languages`, `default` flag, `quantization`. Whisper entries point at the ggerganov whisper.cpp release set (`ggml-<id>.bin`, the turbo default at `q8_0`), the VAD entry at whisper.cpp's silero model; Parakeet entries are listed with `available = false` until S-13 lands so the readiness source is honest. [paraphrase]
- On disk: `<models>/<provider>/<id>/` holds the model file and a `manifest.json` (`provider`, `id`, `sha256`, `size_bytes`, `downloaded_at`, `catalogue_version`, `verified`). A partial download lives at `<file>.part` beside a `.part.json` recording the URL and the bytes expected, so a restart resumes with an HTTP `Range` request. A file whose checksum mismatches is moved to `<models>/quarantine/<provider>/<id>-<timestamp>/` and reported. [paraphrase]
- Readiness: `ready` when the file exists and the manifest says verified; `downloading` with `bytes_done` and `bytes_total`; `partial` when a `.part` exists and no download runs; `missing`; `quarantined`. Verification on first use after a daemon start re-hashes the file once and caches the result in the manifest. [inferred]
- Downloader: `reqwest` with rustls, one download at a time per model, a bounded number overall, progress events at most four per second per model, cancellation through a token, resume through `Range`. A fixture server for tests serves catalogue files from a directory and can inject a truncated body, a wrong checksum and a dropped connection. [inferred]
- Selection: `[speech] provider`, `model`, `meeting_model`, `parakeet_model_id` in `config.toml` (the daemon's config layer already carries `provider` and `model`); `speech.selection.set` writes through the config layer with validation against the catalogue and answers the resolved selection for dictation and meetings; a change requests a preload from `model_selection` through the supervisor spec's preloader. [paraphrase]
- Events: `model.download` payload `{ provider, model, state, bytes_done, bytes_total, error }` on the event stream when the events layer (S-07) exists; until then the payload type and the publisher hook live in the daemon and the CLI polls `speech.models.status --follow`. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `speech.providers.list` and `speech.selection.get|set` in the contract's Windows-adopted shapes (`crates/dettivo-proto/src/methods/speech.rs`), with readiness filled from the catalogue and the disk. [paraphrase]
- Linux additions, recorded in `docs/api/linux-deltas.md` with fixtures: `speech.models.status { provider?, model? }` returning every model's readiness; `speech.models.download { provider, model }` returning the download state (idempotent while running); `speech.models.cancel { provider, model }`; `speech.models.delete { provider, model }` refusing the selected model with `CONFLICT` unless `force`. [inferred]
- CLI: `dettivo speech providers`, `selection get|set --provider --model --meeting-model`, `download --model [--provider] [--wait]`, `delete --model [--force]`, `status [--follow]`, next to the existing `engines`. Exit codes per contract; `--wait` streams progress lines in human mode. [paraphrase]
- Config keys `[speech] meeting_model = ""` (empty means the dictation model), `parakeet_model_id = ""`, `[models] max_concurrent_downloads = 1`, `verify_on_start = true`, documented in `docs/config.md`. [inferred]
- `dettivo doctor` gains a models row per provider: selected model, readiness, quarantined count. [paraphrase]

## Edge Cases & Constraints

- A checksum mismatch never leaves the bad file in place: quarantine, report, readiness `quarantined`; a retry downloads afresh. [paraphrase]
- Deleting the selected model is refused with `CONFLICT` naming the selection unless forced; a forced delete leaves the selection and readiness `missing`. [inferred]
- A download that fails mid-way keeps the `.part` and resumes on the next request; a server without `Range` support restarts from zero and says so in the log. [inferred]
- No implicit downloads: selection, preload and session start never download; they report `missing` and the recommended action. [paraphrase]
- The catalogue file is checked at build time by a test: every entry has a checksum, a license and a URL on an allowlisted host. [inferred]

## Acceptance Criteria

- **R1:** The catalogue manifest lists every Whisper model, the VAD model and the Parakeet placeholders with provider, id, display name, size, URL, SHA-256, license and redistribution note; a test rejects an entry missing any field. Errors: a malformed catalogue fails the build test naming the entry. [paraphrase]
- **R2:** `speech.models.download` against the fixture model server writes the file under `<models>/<provider>/<id>/` with a manifest marking it verified, reports progress (at least three states for a multi-chunk fixture), resumes a truncated download with a `Range` request, and can be cancelled leaving a `.part`. Errors: a wrong checksum quarantines the file and reports `quarantined`. [paraphrase]
- **R3:** `speech.providers.list`, `speech.selection.get`, `speech.selection.set` and `speech.models.status` answer with the contract shapes and their fixtures pass against the live daemon; readiness comes from one source shared by the CLI and the doctor. Errors: `speech.selection.set` with an unknown model is `INVALID_PARAMS` naming the model. [paraphrase]
- **R4:** Selecting a downloaded model through `speech.selection.set` preloads it through the supervisor (`model_selection`, coalesced) and a later session start finds the engine warm; selecting a missing model reports `missing` and does not download. Errors: the preload failure is logged privacy-safe and the selection still applies. [paraphrase]
- **R5:** `dettivo speech providers|selection|download|delete|status` and the doctor's models row work end to end against the fixture server, with `--json` shapes matching the methods. Errors: deleting the selected model without `--force` is `CONFLICT`. [paraphrase]
- **R6:** A verified model whose file changes on disk is detected on the next start (`verify_on_start`) and marked `quarantined`; the `[speech]` and `[models]` keys are documented and printed by `config print-default`. Errors: an invalid key value fails validation naming the key. [inferred]

## Boundaries

- No GUI; the models screen of first run and the settings route read these methods later. [paraphrase]
- No Parakeet or LLM downloads beyond catalogue placeholders; S-13 and S-33 fill their entries. [paraphrase]
- No event stream transport; the `model.download` payload and publisher hook are ready for S-07. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- Neither existing port records checksums or licenses per model; a verified, resumable catalogue is what makes a provisioned machine (config plus model directory) skip first run entirely (FR-C5). [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** the model a user picks is downloaded, verified and warm before the first hotkey press. [strategy:Complete speech workflows, proven by drives]
- **Contract parity and agent surfaces:** the Windows-adopted `speech.*` methods answer with their shapes, and the Linux additions are recorded as deltas. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |

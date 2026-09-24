# Cleanup: backend, tier and configuration truth (18 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

What the daemon reports about backends, tiers and effective configuration is what is running. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0049 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### daemon

- **daemon/F3** (contract): Invalid configuration silently removes the extra authentication requirement
  - Where: `crates/dettivo-core/src/config/mod.rs:74`: `Err(err) => (Config::default(), BTreeMap::new(), Some(err))`; `docs/config.md:33`: “the daemon still starts, on defaults”.
  - Why: Reload installs this fallback configuration. The default IPC mode is `peer`, so a malformed edit while using `peer_token` removes token enforcement for same-user clients. Other explicit settings, including retention settings, also revert. This follows the documented decision; the decision itself is unsafe.
  - Change: Retain the last valid effective configuration on reload failure and report the rejected file separately. For an invalid existing file at startup, refuse normal startup or provide an explicitly restricted repair mode. Keep defaults for a missing file.
  - Risk: This changes documented recovery behavior. Test malformed edits while token authentication is enabled, checking both authentication and retained settings.

- **daemon/F4** (bug): The configuration lock does not cover the reads that determine edits
  - Where: `crates/dettivod/src/handlers/polish.rs:128`: `let mut rules = daemon.config().config.polish.rules.clone();`; `crates/dettivod/src/handlers/config.rs:86`: `let _guard = daemon.config_edit_lock();`; `crates/dettivod/src/watcher.rs:29`: `let loaded = daemon.reload();`.
  - Why: Rule and trusted-endpoint handlers derive replacement collections before acquiring the edit lock. Concurrent additions can overwrite each other, and an update can restore a deleted entry. The watcher also reloads outside this lock, allowing an older snapshot to be installed after a newer edit.
  - Change: Serialize reading, deriving, validating, writing and installing a configuration snapshot. Derive collection changes from the file read inside that transaction. Route watcher installation through the same synchronization.
  - Risk: Avoid recursive locking when an edit reloads. Use barriers to test simultaneous additions, update/delete races and watcher overlap.

- **daemon/F15** (bug): Configuration validation accepts settings that the pipeline must reject
  - Where: `crates/dettivo-core/src/config/validate.rs:54`: `config.rest.bind_error().map(|m| ("rest.bind", m))`; `crates/dettivo-transcribe/src/chunker.rs:60`: `if chunk <= overlap + margin`.
  - Why: Semantic validation checks only REST binding. Setting `transcribe.chunk_seconds = 1` passes configuration validation, although default overlap and margin are two and five seconds, making every affected transcription fail later. Numeric types also do not enforce documented finite ranges.
  - Change: Centralize cross-field and range validation in the configuration layer. Validate chunk arithmetic, finite thresholds and closed option sets before writing or installing a configuration.
  - Risk: Do not reject supported custom model names or legitimate boundary values. Test the same invalid configuration through file loading, `config.validate` and `config.set`, with useful key-specific errors.

### agent-surfaces

- **agent-surfaces/F11** (bug): MCP host configuration can discard existing servers
  - Where: `crates/dettivo-mcp/src/hosts.rs:199` — `if !doc.contains_table("mcp_servers")` replaces the entry; `crates/dettivo-mcp/src/hosts.rs:245` — `read_to_string(path).unwrap_or_default()`.
  - Why: A valid TOML inline table is not a regular `Table`, so an inline `mcp_servers` value is replaced and its other servers disappear. Separately, every file-read error is treated as an empty configuration, allowing a subsequent write to overwrite content that was never successfully read.
  - Change: Preserve or explicitly reject existing inline-table representations. Treat only `NotFound` as an empty file, propagate other read failures, and replace files atomically after a successful merge.
  - Risk: Test regular and inline TOML tables, unrelated servers, unreadable files and interrupted writes. Failure must leave the original file intact.

### engines

- **engines/F7** (contract): Model-set verification remembers only the first download’s checksum
  - Where: `crates/dettivo-speech/src/models.rs:146` stores `sha256: entry.sha256.clone()`. Line 170 considers verification valid when `m.verified && m.sha256 == entry.sha256`.
  - Why: The manifest does not identify the additional files or extracted-member checksums. Changing the embedding checksum while leaving the segmentation archive unchanged preserves `Ready` for the old set. Replacing a verified file also leaves the flag valid because readiness checks only file existence.
  - Change: Record a verification identity for every final file, including extracted members. Invalidate verification when catalogue identity or file metadata changes, then rehash before use.
  - Risk: Existing manifests need one re-verification. Test a secondary-file catalogue update and replacement of an already verified file.

- **engines/F8** (contract): Engines can load catalogue models before verification finishes
  - Where: `crates/dettivod/src/main.rs:150` starts background verification immediately before `preload_startup()` at line 152. `crates/dettivod/src/engines.rs:159` checks only `model.is_file()`. `crates/dettivod/src/diarization.rs:270` explicitly accepts `Readiness::Ready | Readiness::Unverified`.
  - Why: Verification is advisory rather than a load prerequisite. A corrupt model can be opened before the verifier quarantines it; moving the file afterward does not revoke an already loaded model.
  - Change: Require verified readiness for catalogue-backed loads. Coalesce verification and make preload wait or defer until it completes. Keep private sideload policy explicit.
  - Risk: First use of manually provisioned weights may wait for hashing. Test startup with an unverified file and a deliberately delayed verifier.

- **engines/F10** (contract): Vulkan preference and backend reporting are not trustworthy
  - Where: `crates/dettivo-engine-proto/src/backend.rs:33` handles `Vulkan | Auto` together and returns CPU when Vulkan is unavailable. `crates/dettivo-engine-parakeet/src/engine.rs:60` accepts a CPU result from the Vulkan attempt. `crates/dettivo-engine-whisper/src/engine.rs:61` labels any successful GPU-requested load `Backend::Vulkan`.
  - Why: `vulkan` is documented as “Vulkan or fail.” A release-binary probe instead loaded Parakeet on CPU under that preference. Whisper also reported Vulkan with an unusable ICD override. An installed ICD file and a successful load do not prove GPU execution.
  - Change: Separate strict preference from automatic fallback. Determine the backend from the native engine’s actual initialized device, and reject CPU results under strict Vulkan preference.
  - Risk: Configurations that silently fell back will now fail explicitly. Test CPU builds, unusable ICDs, missing devices and successful GPU execution.

- **engines/F13** (bug): Running CPU-only diarization changes a GPU machine’s tier
  - Where: `crates/dettivo-speech/src/tier.rs:55` downgrades when any loaded engine has `Backend::Cpu`. `crates/dettivod/src/engines.rs:29` includes `dettivo-engine-diarize` in the binaries passed to tier detection.
  - Why: Diarization intentionally runs on CPU. Its normal operation therefore changes the platform tier and associated performance interpretation, even while speech recognition continues on Vulkan. The tier changes again when diarization idles out.
  - Change: Exclude engines that have no GPU execution path from the fallback calculation. Prefer reporting execution backend per workload over inferring one global tier from all resident engines.
  - Risk: Benchmark classification changes. Test Vulkan speech before, during and after CPU diarization.

- **engines/F14** (test): The named Vulkan LLM test runs inference on CPU
  - Where: `justfile:62` runs the CLI integration suite with `--features vulkan`. `crates/dettivo-engine-llm/tests/cli.rs:42` supplies `--cpu`; its protocol helper sets `DETTIVO_FORCE_CPU=1` at line 99.
  - Why: Compiling Vulkan does not exercise it. Every model-backed inference route in that suite forces CPU. Separately, the supervisor timeout test explicitly accepts the fake engine finishing its slow answer before serving the next request.
  - Change: Parameterize the execution backend and require Vulkan in the Vulkan recipe. Add a cancellation test whose engine cannot finish naturally before the assertion deadline.
  - Risk: GPU-specific failures will become visible. Keep a separate required CPU suite and make unavailable GPU coverage explicit.

### dictation

- **dictation/F9** (bug): HTTP redirects bypass the configured endpoint trust boundary
  - Where: `crates/dettivo-language/src/provider/openai.rs:63` — `Client::builder().timeout(timeout).build()`; `crates/dettivo-language/src/provider/ollama.rs:89` — the same client construction.
  - Why: The initial endpoint is checked, but these clients retain reqwest’s default redirect policy. A trusted or loopback endpoint can issue a 307/308 redirect that forwards the transcript request body to an endpoint that was never checked.
  - Change: Disable redirects for inference requests. If redirects become a requirement, validate every destination against the same endpoint policy before forwarding a body.
  - Risk: Deployments relying on redirects must configure their final inference URL. Test a permitted local server redirecting to a second receiver and assert that the receiver gets no transcript.

- **dictation/F16** (bug): Per-rule model overrides change policy metadata without changing the requested model
  - Where: `crates/dettivo-language/src/policy/mod.rs:304` — `backend.with_model(model)`; `crates/dettivo-language/src/pipeline.rs:206` — `provider.as_ref()`; `crates/dettivo-language/src/provider/openai.rs:68` — `"model": self.model`.
  - Why: The provider is constructed before rule resolution. The override changes the effective policy, but inference still uses the already-constructed provider and its original model. The policy hash can therefore describe an override that was never executed.
  - Change: Bind the provider’s requested model after effective policy resolution, or remove unsupported model overrides until they can be honored.
  - Risk: Model availability differs by provider. Use a recording fake or mock endpoint to assert the request’s actual model, including an unavailable override.

### qa-rig

- **qa-rig/F2** (bug): Hard-linked model files are not isolated against writes
  - Where: `crates/dettivo-qa/src/profile_models.rs:147`: `if std::fs::hard_link(from, to).is_err()`; `crates/dettivo-speech/src/models.rs:154`: `std::fs::write(self.manifest_path(entry), text + "\n")`.
  - Why: The recursive copy hard-links every file, including manifests. Model verification rewrites the manifest in place, changing the shared inode in the real model tree. Existing unlink-safety tests do not establish write isolation.
  - Change: Use reflinks with copy fallback for writable fixtures. At minimum, copy manifests and other mutable files; sharing weights requires an enforceable immutability contract.
  - Risk: Copies consume disk and setup time. Add tests that rewrite and truncate destination manifests and weights, then verify unchanged source bytes and metadata.

- **qa-rig/F3** (bug): The model-directory guard misses symlinked parents of new destinations
  - Where: `crates/dettivo-qa/src/profile_models.rs:64`: `guard(&dir, &s.real)` precedes directory creation; `crates/dettivo-qa/src/profile_models.rs:157`: `std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())`.
  - Why: A fresh destination normally does not exist, so canonicalization fails and the guard falls back to its lexical spelling. If its existing parent is a symlink into the real model directory, containment is missed and creation proceeds inside that directory.
  - Change: Resolve the deepest existing ancestor, append the missing components, and validate containment before creation. Fail explicitly on resolution errors.
  - Risk: Legitimate symlinked stores should remain usable outside the source. Test an existing symlinked parent with a nonexistent profile child, alongside safe external stores.

### qt-hosts

- **qt-hosts/F5** (contract): The meeting engine picker cannot reliably change the daemon’s provider
  - Where: `qt/host/app/meetings_actions.cpp:306`, `QJsonObject params{{QStringLiteral("provider_id"), providerId}};`; line 308 sends `"model_id"`. `crates/dettivo-proto/src/methods/speech.rs:125` defines `pub provider: Option<String>`, and line 128 defines `pub model: Option<String>`.
  - Why: The request uses response-field names that the request schema rejects. There is another failure when the current provider is Parakeet: `applyProviders()` substitutes the first meeting-capable provider into `m_selectedProvider` without changing the daemon. Picking that displayed provider then takes the “same provider” branch and only writes its meeting model.
  - Change: Send `provider` and `model`. Keep the daemon’s actual selection separate from the picker’s offered fallback, and confirm the selection write before starting.
  - Risk: Changing providers also changes dictation, as ADR 0038 records. Test starting a meeting from an initial Parakeet configuration through the actual daemon contract.

- **qt-hosts/F6** (contract): Home’s Start action ignores the selected dictation mode
  - Where: `qt/host/app/status_model.cpp:221`, `QJsonObject{{QStringLiteral("mode"), QStringLiteral("raw")}}`; `qt/qml/Dettivo/app/InstrumentStrip.qml:59`, `root.config.set("dictation.mode", chosen);`.
  - Why: The mode control writes the user’s choice, but Start always explicitly requests Raw. Home can display Enhanced or Polish while starting a Raw take.
  - Change: Remove the hardcoded override and let the daemon use the configured mode.
  - Risk: Check Raw, Polish and Enhanced through the Home button, comparing the completion and stored transcript mode with the displayed selection.

- **qt-hosts/F7** (contract): First run’s local-model download is still an unfinished integration
  - Where: `qt/host/app/first_run_models.cpp:370`, `m_link->call(QStringLiteral("llm.models.download"), QJsonObject(), ...`; `crates/dettivo-proto/src/methods/llm.rs:111`, `pub model: String`.
  - Why: The download request omits its required model, and its callback discards the error. Additionally, provider refresh starts before capabilities are read; `setLlmMethods()` only stores the later answer, so the initial local-model row can remain “soon” even when downloading is supported.
  - Change: Replace the obsolete provisioning stub with the existing catalogue/status/download operations used by Settings. Send the selected model ID, recompute availability when capabilities arrive, and retain download failures on the row.
  - Risk: Test a fresh profile with no language model, delayed capability replies, a successful download and a refused download. The optional choice must remain skippable.

### qml

- **qml/F5** (contract): Starting a meeting overrides the configuration defaults the rail claims to follow
  - Where: `qt/qml/Dettivo/app/meetings/NewMeetingRail.qml:22` — `property bool analyze: true`; line 59 passes `root.analyze, true` to `start`. Line 226 says `config.toml sets these defaults`. `qt/host/app/meeting_live_model.cpp:89` sends explicit `analyze` and `diarize` values.
  - Why: The rail never initializes those choices from configuration. A user who disables automatic analysis or automatic diarization still gets explicit requests for them from this screen. The daemon treats these as meeting-specific overrides.
  - Change: Read the effective configuration into the rail and preserve explicit user changes separately, or omit untouched options so the daemon applies its defaults.
  - Risk: Test both automatic settings disabled, individual overrides enabled, and configuration changes while the rail is open. Verify the resulting request and post-meeting jobs.

### ops-and-record

- **ops-and-record/F4** (bug): The install workflow creates a broken model symlink
  - Where: `.github/workflows/rig.yml:466` passes `--models .ci-data/dettivo/models`; `scripts/packaging/install-test.sh:37` retains that relative value; `scripts/packaging/install-test-checks.sh:183` runs `ln -sfn "$models" "$tmp/data/dettivo/models"`.
  - Why: The symlink resolves relative to the nested daemon data directory, not the repository. Engine smoke checks can find the models from the working directory while the installed daemon cannot find them through its model directory.
  - Change: Resolve the models directory to an absolute path once when parsing arguments.
  - Risk: Missing directories should produce an explicit failure or skip. Exercise the workflow’s exact relative-path invocation and an absolute-path invocation.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** daemon/F3, Invalid configuration silently removes the extra authentication requirement: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** daemon/F4, The configuration lock does not cover the reads that determine edits: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** daemon/F15, Configuration validation accepts settings that the pipeline must reject: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** agent-surfaces/F11, MCP host configuration can discard existing servers: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** engines/F7, Model-set verification remembers only the first download’s checksum: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** engines/F8, Engines can load catalogue models before verification finishes: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** engines/F10, Vulkan preference and backend reporting are not trustworthy: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** engines/F13, Running CPU-only diarization changes a GPU machine’s tier: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** engines/F14, The named Vulkan LLM test runs inference on CPU: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R10:** dictation/F9, HTTP redirects bypass the configured endpoint trust boundary: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R11:** dictation/F16, Per-rule model overrides change policy metadata without changing the requested model: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R12:** qa-rig/F2, Hard-linked model files are not isolated against writes: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R13:** qa-rig/F3, The model-directory guard misses symlinked parents of new destinations: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R14:** qt-hosts/F5, The meeting engine picker cannot reliably change the daemon’s provider: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R15:** qt-hosts/F6, Home’s Start action ignores the selected dictation mode: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R16:** qt-hosts/F7, First run’s local-model download is still an unfinished integration: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R17:** qml/F5, Starting a meeting overrides the configuration defaults the rail claims to follow: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R18:** ops-and-record/F4, The install workflow creates a broken model symlink: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R19:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0049 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.

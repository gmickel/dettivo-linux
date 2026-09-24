# The speech engines, the supervisor and the engine protocol

## Verdict

The process boundaries are sound, and all four engines perform real inference. The surrounding lifecycle and transcription logic is not ready to call done: downloads can deadlock, inference can block status requests, and transcription heuristics can silently remove speech. The most valuable change is to make engine operations bounded and recoverable without holding locks needed by the daemon’s control paths.

Review was read-only. Existing release binaries passed CPU fixture probes for Whisper, Parakeet, LLM generation and diarization; additional probes reproduced the Vulkan preference and streaming defects below. These were existing artifacts, not a fresh build. I did not run `just build test lint`, which writes build artifacts; the working tree remained clean.

## Findings

### F1. Download completion and model status can deadlock each other
- **Kind:** bug
- **Where:** `crates/dettivo-speech/src/download.rs:114` holds `let mut g = p.lock()` through `on_progress(&g)` at line 122. `crates/dettivod/src/models.rs:287` then takes `inner.lock()`, while line 342 calls `d.progress()` from the locked model service.
- **Why:** Completion takes the progress lock followed by the model-service lock. A concurrent status request takes those locks in reverse order. Each can wait indefinitely for the other.
- **Change:** Update progress and clone its final snapshot under the lock, release the guard, then invoke the callback. Never call external callbacks while holding the progress lock.
- **Risk:** Preserve the terminal progress event. Test completion racing with status using barriers that force the opposing lock order.

### F2. Status requests wait for inference to finish
- **Kind:** bug
- **Where:** `crates/dettivo-speech/src/supervisor.rs:145` takes `slot.lock()` and retains it through `f(process, &loaded)` at line 213. Its status path takes the same lock at line 361. `crates/dettivo-speech/src/llm/mod.rs:189` calls this blocking status path before reaching `with_running`.
- **Why:** A status request cannot observe a busy engine until that engine releases its inference lock. Consequently, the LLM’s intended `Busy` handling does not solve the problem. `system.capabilities` also reaches this path through engine tier detection.
- **Change:** Keep an independently readable status snapshot and use nonblocking inspection for busy slots. Make the idle reaper skip occupied slots rather than wait behind inference.
- **Risk:** Snapshot transitions must stay consistent. Hold a fake inference request open and verify that capabilities, engine status and idle checks still return promptly.

### F3. Timeouts neither bound the complete operation nor invalidate uncertain engine state
- **Kind:** bug
- **Where:** `crates/dettivo-speech/src/process.rs:215` calls `write_frame(...)` before creating the deadline at line 216; line 267 returns a transport error without terminating the process. `crates/dettivo-speech/src/supervisor.rs:208` returns other load errors without clearing the slot.
- **Why:** Waiting for the slot, loading and blocking pipe writes sit outside the request timeout. A timed-out engine remains available for reuse. Worse, if loading model B times out while the cached model is A, B can finish loading later; a subsequent request for A can skip loading and run against B.
- **Change:** Carry one deadline through queueing, loading, writing and response handling. After a timeout or broken protocol, terminate the child and clear cached load state unless cancellation and recovery have been positively acknowledged.
- **Risk:** Recovery will sacrifice warm models. Test stalled stdin, ignored cancellation, queued requests and a late model-load response.

### F4. Whole-chunk RMS can classify audible speech as silence
- **Kind:** bug
- **Where:** `crates/dettivo-transcribe/src/job.rs:219` skips recognition when `filters::is_near_silent(&pcm, ...)` succeeds. `crates/dettivo-transcribe/src/filters.rs:96` implements that as `rms(samples) < floor`.
- **Why:** The default chunk is five minutes. One second of speech at RMS 0.1 followed by 299 seconds of silence averages to approximately 0.00577, below the 0.0065 threshold. The job skips the speech and can report `notice = silent`. Applying a speech activation threshold to a five-minute average is the wrong assumption.
- **Change:** Reject a chunk only when every short analysis window falls below the speech threshold. Retain the cheap shortcut for genuinely silent audio.
- **Risk:** More sparse recordings will reach inference. Test a brief utterance surrounded by several minutes of silence alongside the all-silence fixture.

### F5. Seam deduplication deletes genuine repetitions across gaps
- **Kind:** bug
- **Where:** `crates/dettivo-transcribe/src/merger.rs:119` matches `tail[tail.len() - k..] == head[..k]` without checking time. Lines 181–183 invoke reconciliation even when consecutive chunks do not overlap.
- **Why:** Matching words alone do not establish duplicate audio. Two separate “yes” utterances can collapse into one even when their windows are seconds apart. The live path retains earlier windows across silence, so this affects ordinary meetings.
- **Change:** Bypass reconciliation for disjoint windows. Restrict seam candidates to the actual shared audio interval, with an explicit timestamp tolerance.
- **Risk:** Some previously hidden engine repetitions may reappear. Test repeated phrases across silence, zero-overlap chunks and genuine duplicated overlap.

### F6. Changing load settings can silently retain the old configuration
- **Kind:** contract
- **Where:** `crates/dettivo-speech/src/supervisor.rs:177` compares only `l.model != model || s.lora.as_deref() != params.lora.as_deref()`. `crates/dettivod/src/engines.rs:132` rebuilds the selected speech engine only when its model or provider changes.
- **Why:** Backend preference, context length, VAD model and diarization thread count are absent from the cache identity. A backend-only speech configuration change also leaves the old engine wrapper intact. A successful configuration write can therefore have no effect.
- **Change:** Cache the complete effective load configuration. Rebuild speech wrappers when their backend changes, and invalidate processes when their executable selection changes.
- **Risk:** Configuration reloads may trigger additional loads. Test each load-affecting setting independently while keeping the model path constant.

### F7. Model-set verification remembers only the first download’s checksum
- **Kind:** contract
- **Where:** `crates/dettivo-speech/src/models.rs:146` stores `sha256: entry.sha256.clone()`. Line 170 considers verification valid when `m.verified && m.sha256 == entry.sha256`.
- **Why:** The manifest does not identify the additional files or extracted-member checksums. Changing the embedding checksum while leaving the segmentation archive unchanged preserves `Ready` for the old set. Replacing a verified file also leaves the flag valid because readiness checks only file existence.
- **Change:** Record a verification identity for every final file, including extracted members. Invalidate verification when catalogue identity or file metadata changes, then rehash before use.
- **Risk:** Existing manifests need one re-verification. Test a secondary-file catalogue update and replacement of an already verified file.

### F8. Engines can load catalogue models before verification finishes
- **Kind:** contract
- **Where:** `crates/dettivod/src/main.rs:150` starts background verification immediately before `preload_startup()` at line 152. `crates/dettivod/src/engines.rs:159` checks only `model.is_file()`. `crates/dettivod/src/diarization.rs:270` explicitly accepts `Readiness::Ready | Readiness::Unverified`.
- **Why:** Verification is advisory rather than a load prerequisite. A corrupt model can be opened before the verifier quarantines it; moving the file afterward does not revoke an already loaded model.
- **Change:** Require verified readiness for catalogue-backed loads. Coalesce verification and make preload wait or defer until it completes. Keep private sideload policy explicit.
- **Risk:** First use of manually provisioned weights may wait for hashing. Test startup with an unverified file and a deliberately delayed verifier.

### F9. Model replacement temporarily keeps both models resident
- **Kind:** simplify
- **Where:** `crates/dettivo-engine-proto/src/host_llm.rs:125` calls `E::load` before replacing `self.engine` at line 128. `crates/dettivo-engine-llm/src/engine.rs:164` compares the new model’s size with current device free memory.
- **Why:** The old model still consumes memory during the new load. Two models that each fit individually can fail to switch on Vulkan or force an unnecessary CPU fallback. Speech and diarization hosts use the same replacement pattern.
- **Change:** Unload the previous model before loading its replacement. A failed replacement should leave an explicit unloaded state. Reset Parakeet’s process-global backend only after its old model has been dropped.
- **Risk:** Failed switches lose the previous warm model. Test switching between two models that fit separately but not simultaneously.

### F10. Vulkan preference and backend reporting are not trustworthy
- **Kind:** contract
- **Where:** `crates/dettivo-engine-proto/src/backend.rs:33` handles `Vulkan | Auto` together and returns CPU when Vulkan is unavailable. `crates/dettivo-engine-parakeet/src/engine.rs:60` accepts a CPU result from the Vulkan attempt. `crates/dettivo-engine-whisper/src/engine.rs:61` labels any successful GPU-requested load `Backend::Vulkan`.
- **Why:** `vulkan` is documented as “Vulkan or fail.” A release-binary probe instead loaded Parakeet on CPU under that preference. Whisper also reported Vulkan with an unusable ICD override. An installed ICD file and a successful load do not prove GPU execution.
- **Change:** Separate strict preference from automatic fallback. Determine the backend from the native engine’s actual initialized device, and reject CPU results under strict Vulkan preference.
- **Risk:** Configurations that silently fell back will now fail explicitly. Test CPU builds, unusable ICDs, missing devices and successful GPU execution.

### F11. Diarization cannot transport the full supported import duration
- **Kind:** contract
- **Where:** `crates/dettivo-engine-proto/src/frame.rs:16` sets `MAX_ATTACHMENT_BYTES` to `256 << 20`. `crates/dettivo-speech/src/diarize.rs:85` sends the entire track as one attachment. `crates/dettivo-core/src/config/schema.rs:312` defaults `max_import_seconds` to `14_400`.
- **Why:** The attachment limit holds only about 2 hours 20 minutes of 16 kHz mono PCM. A supported four-hour import needs 460,800,000 bytes. The writer does not enforce the reader’s limit, so rejection happens after transfer begins.
- **Change:** Define and enforce consistent duration and transport limits before reading or sending the full track. Support the advertised maximum through an appropriately bounded transport.
- **Risk:** Increasing the byte ceiling also increases memory pressure. Test the largest supported duration and rejection immediately above it without allocating oversized test buffers.

### F12. Streamed LLM text disagrees with the final answer when a stop string spans tokens
- **Kind:** bug
- **Where:** `crates/dettivo-engine-llm/src/engine.rs:315` checks `stop_at(&text, ...)`, then line 320 emits `partial(&piece)` for preceding pieces.
- **Why:** A prefix of a stop string can already have been emitted when its remaining characters arrive. The release probe with stop string `"3\n"` streamed `"3"` but returned an empty final answer.
- **Change:** Hold back the longest suffix that could begin a stop string. Emit it only when it can no longer match; discard it when the stop completes.
- **Risk:** Streaming gains a small bounded delay. Test stop strings spanning multiple tokens, Unicode boundaries and ordinary end-of-generation flushing.

### F13. Running CPU-only diarization changes a GPU machine’s tier
- **Kind:** bug
- **Where:** `crates/dettivo-speech/src/tier.rs:55` downgrades when any loaded engine has `Backend::Cpu`. `crates/dettivod/src/engines.rs:29` includes `dettivo-engine-diarize` in the binaries passed to tier detection.
- **Why:** Diarization intentionally runs on CPU. Its normal operation therefore changes the platform tier and associated performance interpretation, even while speech recognition continues on Vulkan. The tier changes again when diarization idles out.
- **Change:** Exclude engines that have no GPU execution path from the fallback calculation. Prefer reporting execution backend per workload over inferring one global tier from all resident engines.
- **Risk:** Benchmark classification changes. Test Vulkan speech before, during and after CPU diarization.

### F14. The named Vulkan LLM test runs inference on CPU
- **Kind:** test
- **Where:** `justfile:62` runs the CLI integration suite with `--features vulkan`. `crates/dettivo-engine-llm/tests/cli.rs:42` supplies `--cpu`; its protocol helper sets `DETTIVO_FORCE_CPU=1` at line 99.
- **Why:** Compiling Vulkan does not exercise it. Every model-backed inference route in that suite forces CPU. Separately, the supervisor timeout test explicitly accepts the fake engine finishing its slow answer before serving the next request.
- **Change:** Parameterize the execution backend and require Vulkan in the Vulkan recipe. Add a cancellation test whose engine cannot finish naturally before the assertion deadline.
- **Risk:** GPU-specific failures will become visible. Keep a separate required CPU suite and make unavailable GPU coverage explicit.

### F15. The Parakeet build modifies a caller’s source checkout
- **Kind:** bug
- **Where:** `crates/parakeet-cpp-sys/build.rs:41` uses `PARAKEET_CPP_SOURCE_DIR` directly, then line 44 calls `apply_cpu_patch(&source)`. Lines 163–168 apply the patch inside that tree and write a marker.
- **Why:** An override described as pointing at local source unexpectedly mutates it. An already patched checkout without Dettivo’s marker can also fail, and concurrent builds can race on the same patch operation.
- **Change:** Treat supplied source trees as immutable. Copy them into a build-owned staging directory before patching, or require an explicitly prepared source tree and only validate it.
- **Risk:** Staging adds build time and disk usage. Test a read-only override directory and two builds using the same source tree.

### F16. The transcription crate carries an unused language dependency
- **Kind:** delete
- **Where:** `crates/dettivo-transcribe/Cargo.toml:14` declares `dettivo-language.workspace = true`. Its only source reference is `dettivo_language::CRATE_NAME` in `crates/dettivo-transcribe/src/lib.rs:31`.
- **Why:** Transcription uses no language-crate behavior. The dependency exists solely to make a declared dependency list resolve, adding coupling without functionality.
- **Change:** Delete the dependency and its `UPSTREAM` entry; update the corresponding dependency-edge expectation.
- **Risk:** No runtime behavior should change. Run the transcription tests and dependency-edge lint.

## Keep

- Separate native engine processes prevent ggml symbol collisions and contain native failures. Keep that boundary in `crates/dettivo-speech/src/process.rs`.
- Length-prefixed JSON with binary PCM attachments is appropriate. Keep the framing rather than replacing it with newline parsing or base64 in `crates/dettivo-engine-proto/src/frame.rs`.
- Pinned, checksummed native archives and narrow bindings are justified in `crates/parakeet-cpp-sys/build.rs` and `crates/sherpa-onnx-sys/build.rs`.
- Range-based audio reads and quiet-point chunk boundaries are straightforward and useful in `crates/dettivo-transcribe/src/source.rs` and `crates/dettivo-transcribe/src/chunker.rs`.
- Keep Parakeet’s explicit dictation-only capability until stronger alignment evidence changes the decision in `crates/dettivo-speech/src/engines.rs`.
- The real-engine WER and diarization fixtures provide useful evidence in `crates/dettivo-engine-whisper/tests/cli.rs`, `crates/dettivo-engine-parakeet/tests/cli.rs` and `crates/dettivo-engine-diarize/tests/cli.rs`.

## Questions for the owner

- Should degradation cover repeated inference crashes? Currently a successful load resets the counter, so an engine that loads successfully and crashes on every recognition never reaches three consecutive crashes. The tests deliberately preserve this behavior.
- Is exact macOS parity required for cross-source suppression? `crates/dettivo-transcribe/src/interleave.rs` deletes short microphone responses near longer remote speech without establishing that their words match. Legitimate “yes” and “no” answers can satisfy that rule.
- Should private sideloads have the same integrity and containment guarantees as catalogue models? `crates/dettivo-speech/src/llm/sideload.rs` checks lexical relative paths but follows symlinks and does not verify a model checksum.
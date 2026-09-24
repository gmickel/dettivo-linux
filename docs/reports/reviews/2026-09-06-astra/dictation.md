# The dictation chain: capture, session, hotkeys, insertion and the language layer

## Verdict

The chain has useful module boundaries, but it does not yet reliably guarantee that releasing a key delivers the intended text to the intended window—or that cancellation prevents delivery. The most valuable change is to make each take own its completion, cancellation and verified target through the final insertion decision. No files changed; this was a source and test review, the scoped Rust files meet the 500-line limit, and `just build test lint` was not run because the environment is read-only.

## Findings

### F1. Cancellation during polishing can still insert and archive the transcript

- **Kind:** bug
- **Where:** `crates/dettivo-session/src/worker.rs:126` — `if self.cancel.load(Ordering::Relaxed)`; `crates/dettivo-session/src/worker.rs:168` — `dettivo_language::pipeline::run(`; `crates/dettivo-session/src/worker.rs:202` — `Some(self.inserter.insert(&text, self.target.as_ref()))`.
- **Why:** The worker checks cancellation after transcription, then runs the language pipeline and proceeds directly to insertion. Cancellation during a slow rewrite therefore does not prevent insertion or archival. Meanwhile, `crates/dettivo-session/src/machine.rs:279` returns a snapshot whose state is `State::Cancelled`, even if the worker completed those effects.
- **Change:** Carry cancellation through language processing and insertion preparation. Coordinate cancellation with the final insertion decision, and distinguish cancellation before delivery from an operation whose delivery has already started.
- **Risk:** A check alone leaves a race immediately before typing. Test cancellation with a provider blocked behind a barrier, then release it and assert no insertion, archive or successful completion.

### F2. The captured target identifies an application process, not the originating window

- **Kind:** contract
- **Where:** `crates/dettivod/src/actions.rs:53` — `Some(SessionTarget { app_id: Some(target.app_id), pid: target.pid, })`; `crates/dettivo-insert/src/chain.rs:37` — `if guards.app_id.is_none() && guards.pid.is_none() { return Ok(()); }`.
- **Why:** Two windows belonging to the same process satisfy both guards. A failed focus probe at recording start also becomes an absent target, which means no origin guard; if probing works later, automatic insertion can proceed into a different window.
- **Change:** Preserve a stable native window identifier internally. Represent “origin could not be verified” separately from an intentionally unguarded request, and route an unverified dictation origin to clipboard recovery.
- **Risk:** Window identity differs between compositors and X11. Test two windows from one process, a closed origin, and a failed initial probe followed by a successful final probe.

### F3. Backend failure blindly retries text that may already have been partially delivered

- **Kind:** bug
- **Where:** `crates/dettivo-insert/src/service.rs:353` — `Err(e)` logs `"backend failed, trying the next"`; `crates/dettivo-insert/src/backend/commands.rs:15` — `Duration::from_secs(20)`; `crates/dettivo-insert/src/backend/commands.rs:155` — `inter_key_delay_ms.max(REMAP_DELAY_MS)`.
- **Why:** Backend errors do not distinguish failure before delivery from partial or uncertain delivery. For example, non-ASCII text gives xdotool a minimum 40 ms delay; a sufficiently long transcript can reach the 20-second timeout after typing a prefix. The next backend receives the entire transcript again.
- **Change:** Return structured delivery status: nothing delivered, partial delivery, or uncertain delivery. Automatically try another backend only when nothing was delivered; otherwise retain the transcript and report the incomplete result.
- **Risk:** Some backends cannot determine exactly what the application received. Treat uncertainty conservatively and test failure after a delivered prefix, including command timeout.

### F4. Delete the second transcript archive outside history retention

- **Kind:** delete
- **Where:** `crates/dettivo-session/src/worker.rs:230` — `write_transcript(&self.dir, &transcript)`; `crates/dettivo-session/src/worker.rs:374` — `serde_json::to_string_pretty(t)`; `crates/dettivo-session/src/worker.rs:362` — `if !self.policy.keep_audio`.
- **Why:** Every successful take writes the complete transcript object, including raw text and frozen policy, into its session directory before separately archiving it. Cleanup only discards take files when audio retention is disabled; `crates/dettivo-audio/src/takes.rs:178` does not remove this transcript file. History retention owns a different artifact directory. Cancelled takes can also retain audio through the same `keep_audio` condition without creating a history item.
- **Change:** Remove the redundant transcript write. Give history sole ownership of retained artifacts, clean successful staging after archival, and discard cancelled takes regardless of successful-take retention preferences.
- **Risk:** Preserve recoverability when archival fails through an explicit, bounded recovery mechanism. Check raw-disabled history, history deletion, retention expiry and cancellation against both history and session storage.

### F5. Delete synthesized undo until it can identify the inserted range

- **Kind:** delete
- **Where:** `crates/dettivo-insert/src/service.rs:169` — `then.app_id == now.app_id && then.pid == now.pid`; `crates/dettivo-insert/src/backend/virtual_keyboard.rs:310` — `typist.combo(&[(key("Shift_L"), shift)], &[key("Left")], chars)` followed by `typist.tap(key("Delete"))`.
- **Why:** Matching the process within a short time window does not establish that the cursor remains immediately after the inserted text. Typing more text, moving the cursor or switching windows can make this delete unrelated content. Unicode scalar counts also need not equal cursor movements.
- **Change:** Stop advertising undo for backends that only synthesize selection and deletion. Restore it only where the backend can verify the inserted range or use a suitable application transaction.
- **Risk:** This removes a convenience but avoids destructive guesses. Cover cursor movement, intervening typing, same-process windows, combining characters and terminal behavior before restoring support.

### F6. Focus and self-target guards expire before the backend actually types

- **Kind:** bug
- **Where:** `crates/dettivo-insert/src/service.rs:251` — `let probed = self.probe(&session, settings)`; `crates/dettivo-insert/src/service.rs:281` — `self.candidates(&backends, &session)`; `crates/dettivo-insert/src/service.rs:315` — `backend.insert(request.text, &ctx)`.
- **Why:** Guard checks happen before backend availability probing and setup. Portal interaction and multi-character typing can take appreciable time, during which focus may change—even to Dettivo itself. The eventual insertion still uses the earlier decision.
- **Change:** Separate backend preparation from delivery. Revalidate the origin and self-target guard immediately before delivery, serialize insertion operations, and stop subsequent typing batches when focus changes.
- **Risk:** Synthetic input cannot offer universal atomic focus guarantees. Test focus changes during portal setup and between typing batches, and report partial delivery honestly.

### F7. The secure-field guard is unreachable in the real probe path

- **Kind:** contract
- **Where:** `crates/dettivo-insert/src/service.rs:91` — `probe::target_from(&r, settings, false)`; `crates/dettivo-insert/src/service.rs:275` — `if target.is_some_and(|t| t.secure_field)`.
- **Why:** Production probing always supplies `false` for secure-field status. The later clipboard-only branch therefore does not protect password fields, despite the insertion record describing that protection.
- **Change:** Obtain focused-field sensitivity from an actual accessibility probe and represent unknown status explicitly. Align the documented guarantee with the detection that is implemented.
- **Risk:** Accessibility information is incomplete in some applications. Verify ordinary text, a native password field and an inaccessible field through the real probe, rather than constructing a target with the bit already set.

### F8. Cancellation hotkeys can be delayed until after insertion—or discarded entirely

- **Kind:** bug
- **Where:** `crates/dettivod/src/hotkeys.rs:121` — `for event in rx`; `crates/dettivod/src/actions.rs:153` — `Release(Action::PushToTalk) => stop(daemon)`; `crates/dettivo-hotkeys/src/evdev.rs:45` — `if self.active.is_some() { return None; }`.
- **Why:** The portal/evdev action consumer calls synchronous stop, which waits through transcription, rewriting and insertion. A subsequent cancel event remains queued until that finishes. Separately, evdev rejects any second chord while the push-to-talk chord is held, including cancellation during recording.
- **Change:** Keep hotkey dispatch responsive by separating the stop signal from completion waiting. Allow the cancel action while push-to-talk remains active without losing the eventual release event.
- **Risk:** Preserve event ordering and prevent duplicate starts. Test cancellation while holding push-to-talk and after release while a fake engine or provider is blocked.

### F9. HTTP redirects bypass the configured endpoint trust boundary

- **Kind:** bug
- **Where:** `crates/dettivo-language/src/provider/openai.rs:63` — `Client::builder().timeout(timeout).build()`; `crates/dettivo-language/src/provider/ollama.rs:89` — the same client construction.
- **Why:** The initial endpoint is checked, but these clients retain reqwest’s default redirect policy. A trusted or loopback endpoint can issue a 307/308 redirect that forwards the transcript request body to an endpoint that was never checked.
- **Change:** Disable redirects for inference requests. If redirects become a requirement, validate every destination against the same endpoint policy before forwarding a body.
- **Risk:** Deployments relying on redirects must configure their final inference URL. Test a permitted local server redirecting to a second receiver and assert that the receiver gets no transcript.

### F10. The primary compositor hotkey path ignores the configured dictation mode

- **Kind:** bug
- **Where:** `crates/dettivo-cli/src/commands.rs:269` — `"mode": mode.clone().unwrap_or_else(|| "raw".into())`; `crates/dettivo-cli/src/commands.rs:283` — `"mode": "raw"`.
- **Why:** Compositor snippets invoke the CLI’s start and toggle commands without a mode. Both paths explicitly send Raw, overriding the daemon’s configured default. Portal and evdev starts instead pass no override, so identical hotkeys select different processing behavior depending on backend.
- **Change:** Make an omitted mode consistently mean the daemon’s configured mode. Preserve an explicit Raw override and handle the imported wire contract compatibly.
- **Risk:** Existing clients may depend on required wire fields. Test compositor CLI, portal and evdev entry points with an Enhanced default and with an explicit Raw request.

### F11. Session completion belongs to a global slot that another take can overwrite

- **Kind:** bug
- **Where:** `crates/dettivo-session/src/machine.rs:163` — `*self.outcome.lock()... = None`; `crates/dettivo-session/src/machine.rs:239` — `(a.stop.clone(), a.worker.take())`; `crates/dettivo-session/src/machine.rs:245` — reading `self.outcome`.
- **Why:** Stop joins one worker but retrieves its result from a shared, resettable slot. A new take can clear that result before the old stopper reads it. Concurrent stoppers also compete for the single join handle; a stop during source opening sees no worker and can return `NoSession` despite an active reservation.
- **Change:** Create a completion object for each take before opening its source. Let all stop/cancel callers wait on that take’s result, and publish completion consistently with active-state retirement.
- **Risk:** Start failure and shutdown need to complete the same object. Use barriers to test stop during source opening, two simultaneous stoppers and a new start immediately after completion.

### F12. Polish edits inside tokens that Raw deliberately protected

- **Kind:** bug
- **Where:** `crates/dettivo-language/src/polish/mod.rs:35` — `collapse_whitespace(trimmed)`; `crates/dettivo-language/src/polish/mod.rs:68` — global filler replacement; `crates/dettivo-language/src/polish/mod.rs:100` — replacing `r"\bi\b"` with `"I"`; `crates/dettivo-language/src/polish/mod.rs:121` — `text.to_lowercase()`.
- **Why:** Raw restores protected spans before handing text to Polish. Polish then applies unrestricted transformations to the whole string. A backticked command containing `um` loses it, a path component `i` can become `I`, and Very Casual lowercases case-sensitive tokens.
- **Change:** Share protected-span handling across all deterministic stages. Apply prose transformations only outside protected spans and preserve structural whitespace where it matters.
- **Risk:** Existing goldens may encode these mutations. Add full-pipeline cases for backticked commands, case-sensitive paths and identifiers under every style.

### F13. Disabled transforms and post-processors still run through unconditional passes

- **Kind:** simplify
- **Where:** `crates/dettivo-language/src/polish/mod.rs:43` — punctuation runs for `SmartPunctuation || FixGrammar`; `crates/dettivo-language/src/polish/post.rs:34` — selected processors run before unconditional normalization at lines 41–52.
- **Why:** Disabling Smart Punctuation does not prevent punctuation when Fix Grammar remains enabled. Selecting any post-processor also brings along unrelated fixed passes, some duplicating selected processors. The Code preset filters out Smart Punctuation but still reaches capitalization and terminal punctuation.
- **Change:** Delete duplicated passes and make each configurable operation run once, only when enabled. Separate genuinely mandatory normalization from optional editorial changes, and update the parity decision to describe the resulting behavior.
- **Risk:** This intentionally changes some macOS parity outputs. Check disabled-transform behavior, processor subsets, literal commands and paragraph preservation; resolve the parity question below before changing defaults.

### F14. The golden tests bypass the stage ordering used by dictation

- **Kind:** test
- **Where:** `crates/dettivo-language/tests/goldens.rs:91` — `rewrite(&case.input, ...)`; `crates/dettivo-language/tests/goldens.rs:120` — `apply_post_processors(&processors, &case.input)`; `crates/dettivo-language/src/raw/mod.rs:28` — `("colon", ":")`, and line 34 — `("dash", " - ")`.
- **Why:** The goldens supply original phrases directly to later stages. Real dictation runs Raw first. For example, the corpus expects spoken “dash dash model qwen three colon 4b” to become technical syntax, but Raw consumes “dash” and “colon” before the later phrase recognizers see them.
- **Change:** Keep the unit goldens and add acceptance cases through `pipeline::run` with explicit mode, application context and Raw settings. Fix the stage ownership of spoken technical syntax based on those results.
- **Risk:** Unit parity and useful end-to-end behavior may disagree. Assert the actual inserted string for representative prose, command, filename and model-name cases.

### F15. The rewrite timeout does not bound provider discovery or credential lookup

- **Kind:** bug
- **Where:** `crates/dettivo-language/src/pipeline.rs:149` — provider resolution precedes rewriting; `crates/dettivo-language/src/enhanced/mod.rs:131` — `let deadline = Instant::now() + timeout`; `crates/dettivo-language/src/provider/mod.rs:201` — `Command::new("secret-tool")...output()`.
- **Why:** Provider probing occurs before the rewrite deadline starts. Credential lookup can block outside the HTTP request timeout, and the helper subprocess has no deadline. Thus the configured budget does not bound release-to-result time.
- **Change:** Establish one deadline before provider resolution and pass its remaining budget through probes, credential lookup and inference. Bound and cancel subprocesses as well as network requests.
- **Risk:** A short budget may be exhausted during discovery. Test delayed probes and a stalled credential helper, asserting bounded completion and deterministic fallback.

### F16. Per-rule model overrides change policy metadata without changing the requested model

- **Kind:** bug
- **Where:** `crates/dettivo-language/src/policy/mod.rs:304` — `backend.with_model(model)`; `crates/dettivo-language/src/pipeline.rs:206` — `provider.as_ref()`; `crates/dettivo-language/src/provider/openai.rs:68` — `"model": self.model`.
- **Why:** The provider is constructed before rule resolution. The override changes the effective policy, but inference still uses the already-constructed provider and its original model. The policy hash can therefore describe an override that was never executed.
- **Change:** Bind the provider’s requested model after effective policy resolution, or remove unsupported model overrides until they can be honored.
- **Risk:** Model availability differs by provider. Use a recording fake or mock endpoint to assert the request’s actual model, including an unavailable override.

### F17. The numeric preservation guard accepts changed numbers

- **Kind:** bug
- **Where:** `crates/dettivo-language/src/enhanced/guard.rs:106` — `output.chars().filter(char::is_ascii_digit).collect()`; `crates/dettivo-language/src/enhanced/guard.rs:127` — `if !output_digits.contains(&digits)`.
- **Why:** Concatenating every output digit discards number boundaries and punctuation. Input `42` is considered preserved in `420`; `12` can be satisfied by separate output numbers `1` and `2`; `1.5` can be satisfied by `15`. Repeated input numbers also reuse one output occurrence.
- **Change:** Compare numeric tokens and their multiplicity, preserving signs and decimal meaning. Allow only explicitly supported formatting equivalences.
- **Risk:** Locale-specific grouping requires a defined policy. Test changed magnitude, decimals, signs, repeated numbers and separate numbers whose concatenated digits match.

### F18. Silence detection depends on the UI meter interval

- **Kind:** bug
- **Where:** `crates/dettivo-session/src/worker.rs:109` — `peak < self.policy.silence_peak_threshold`; `crates/dettivo-session/src/worker.rs:299` — only `Event::Level` updates peak; `crates/dettivo-audio/src/level.rs:32` — emission requires `self.count >= self.window`.
- **Why:** The worker derives silence from completed meter windows rather than captured PCM. A voiced take shorter than the configured meter interval can contain samples but no level event, leaving peak at zero and skipping transcription. Speech confined to the final partial window has the same problem.
- **Change:** Calculate the take’s silence statistic directly from PCM as it is collected, independently of display-level events.
- **Risk:** Keep amplitude normalization consistent with the existing threshold. Test short speech, silence followed by a voiced partial window, and different meter intervals over identical PCM.

### F19. Recording is finalized before the capture source is stopped

- **Kind:** bug
- **Where:** `crates/dettivo-session/src/worker.rs:68` — `self.record(&src, &shared)` precedes `src.stop()`; `crates/dettivo-session/src/worker.rs:324` — `while let Ok(event) = src.events().try_recv()`; `crates/dettivo-audio/src/service.rs:171` — stop only sends `Cmd::Stop(self.id)`.
- **Why:** The worker drains an actively producing channel until it happens to be empty, finalizes the take, and only then requests capture stop. Buffers still in flight can arrive after that drain and be omitted. Queue emptiness is not an acknowledged capture boundary.
- **Change:** Request capture stop first, then drain through a defined terminal acknowledgement before finalizing the take. Bound waiting when the source fails.
- **Risk:** The source must reliably emit its terminal event after its final accepted PCM. Test a source that delivers its last chunk during stop, plus a source that fails to acknowledge.

### F20. Device-follow subscriptions can miss changes during capture startup

- **Kind:** bug
- **Where:** `crates/dettivo-audio/src/capture.rs:74` — `Target::Default => graph.default_node()`; `crates/dettivo-audio/src/capture.rs:198` — `drop(guard)`; `crates/dettivo-audio/src/capture.rs:208` — `graph.watch_default()`; line 236 — `graph.watch_removed()`.
- **Why:** Capture selects a node and connects before subscribing to default changes or node removal. A change in that interval is not replayed. A pinned stream can consequently remain alive on a vanished device feeding silence, or a default change can escape the take’s device-change handling.
- **Change:** Establish subscriptions and capture the initial graph state consistently before connecting. Reconcile the selected node with that state instead of relying solely on future notifications.
- **Risk:** Graph-loop locking and callback ordering matter. Test removal and default changes between selection, connection and subscription, without timing-dependent sleeps.

## Keep

- `crates/dettivo-session/src/machine.rs`: reserving the active slot before opening capture correctly prevents competing starts; retain that invariant.
- `crates/dettivo-session/src/lib.rs`: injected source, engine, inserter, archive and publisher boundaries are useful seams for meaningful lifecycle tests.
- `crates/dettivo-audio/src/capture.rs`: requesting 16 kHz mono from PipeWire keeps live capture simple; avoid introducing another live resampling layer.
- `crates/dettivo-insert/src/chain.rs`: pure backend selection, explicit pinning and clipboard fallback are worth keeping once target and delivery facts are trustworthy.
- `crates/dettivo-hotkeys/src/chord.rs` and `crates/dettivo-hotkeys/src/snippet.rs`: shared chord parsing and compositor-owned bindings are the right primary architecture.
- `crates/dettivo-language/src/raw/protect.rs`: explicit protected-span scanning is a useful primitive; extend its lifetime through the pipeline.
- `crates/dettivo-language/src/enhanced/mod.rs` and `crates/dettivo-language/tests/goldens.rs`: deterministic fallback and the existing corpora are valuable; strengthen their integration coverage.

## Questions for the owner

- Should macOS parity preserve unconditional editorial passes, or should explicit transform settings and literal technical text take precedence? `docs/adr/0023-polish-layers-and-the-llm-provider-layer.md` records parity, but the current behavior makes several controls misleading.
- Should automatic `@path` insertion apply to ordinary terminals and editors, or only to explicitly selected coding-assistant contexts? Those destinations need different literal text.
- The requested standalone audio guide was absent; I used `docs/adr/0006-pipewire-capture.md` as the audio decision record. Is there another current audio contract this review should be measured against?
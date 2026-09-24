# Cleanup: other (33 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

The remaining findings across every area, each fixed or rejected with evidence. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0055 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### daemon

- **daemon/F7** (bug): Jobs report completion even when the result was not stored
  - Where: `crates/dettivod/src/history.rs:281`: `if let Err(e) = self.with_store(|store| store.update(item))`, followed only by a warning; `crates/dettivod/src/jobs_run.rs:177`: `jobs.publish(bus, &job_id, progress, last.1, stage);`.
  - Why: `finish_item` and `finish_meeting` return no persistence result. A database failure leaves the runner free to publish `Done`, remove temporary audio, clear busy state and forget the job. Meeting completion can also start downstream work against an uncommitted result.
  - Change: Return persistence failures to the runner. Publish success and start downstream processing only after the authoritative result is durable. Retain sufficient recovery material when committing fails.
  - Risk: Do not leave failed jobs permanently busy. Inject database write failures and verify an honest terminal status, recoverable input and no downstream success callback.

- **daemon/F16** (contract): An explicit null request ID is treated as an absent ID
  - Where: `crates/dettivo-proto/src/envelope.rs:89`: `pub id: Option<RequestId>`; `crates/dettivod/src/router.rs:81`: `let is_notification = request.id.is_none();`.
  - Why: Serde maps both an omitted field and JSON `null` to `None` here. Although `RequestId` supports `Null`, the outer option prevents it from preserving an explicit null request ID. The daemon executes that request as a notification and sends no response.
  - Change: Preserve field presence during deserialization so an explicit null ID receives a null-ID response and an absent ID remains a notification.
  - Risk: Preserve existing string and numeric IDs and notification behavior. Test all four cases through the live socket; response-only null-ID tests cannot detect this defect.

- **daemon/F17** (bug): Overflow accounting can leave dropped events unreported indefinitely
  - Where: `crates/dettivod/src/events.rs:142`: `let n = dropped.swap(0, Ordering::Relaxed);`, followed by `pending.store(false, Ordering::Relaxed)`.
  - Why: A publisher can drop another event between those operations. It increments the counter but sees an overflow notification already pending, so it schedules nothing. The pending notification sends the earlier count. If publishing then stops, the remaining dropped count is never reported.
  - Change: Coordinate the dropped count and notification ownership as one state transition, or keep one drain task responsible until the counter is empty.
  - Risk: Preserve nonblocking publishers and bounded queues. Add a Tokio-runtime test that pauses between the two transitions, causes one final drop, then stops publishing and verifies the complete count.

### agent-surfaces

- **agent-surfaces/F3** (bug): Process-hosted REST uploads exceed the daemon’s default line limit
  - Where: `crates/dettivo-rest/src/stream.rs:106` — `begin["chunk_max_bytes"]`; `crates/dettivod/src/platform.rs:17` — `chunk_max_bytes: 1_048_576`; `crates/dettivo-core/src/config/schema.rs:262` — `max_line_bytes: 1_048_576`.
  - Why: REST sends the full advertised raw chunk size after base64 encoding. A 1 MiB chunk becomes 1,398,104 bytes before the JSON envelope, exceeding the socket’s 1 MiB request limit. Daemon-hosted REST bypasses socket framing, so the same upload behaves differently between hosts.
  - Change: Calculate upload chunks against both the raw transfer limit and the encoded IPC envelope limit. Use the same calculation across clients.
  - Risk: Test uploads crossing chunk boundaries through both hosting modes, including a reduced IPC line limit. These tests can exercise transfers without an inference model.

- **agent-surfaces/F6** (contract): MCP does not enforce its outgoing message cap
  - Where: `crates/dettivo-mcp/src/protocol.rs:122` — `transport::write_message(&mut output, reader.framing(), &response)`; `crates/dettivo-mcp/src/bounds.rs:21` — “Bytes accepted per transport message in either direction.”
  - Why: The cap configures the reader, but responses are serialized and written without a byte check. Per-field truncation does not bound the complete envelope, especially when results appear twice. A 1,999,948-byte initialize request reproducibly produced a 2,000,077-byte JSON response, exceeding the default cap.
  - Change: Enforce the cap after serializing every complete response. Return a bounded error retaining the request ID when the result cannot fit.
  - Risk: Test both framings, small configured caps, multibyte text, large tool results and resource listings. Existing incoming-size tests do not cover this.

- **agent-surfaces/F9** (contract): A refused MCP framing switch still executes its payload
  - Where: `crates/dettivo-mcp/src/transport.rs:171` — only the current line is drained; `crates/dettivo-mcp/src/transport.rs:328` — the switch test checks one error and then stops reading.
  - Why: In line mode, a `Content-Length` header produces an invalid-request response, but its following JSON payload is accepted as another line. The release binary returned an error and then successfully answered the switched `ping`. A tool payload would likewise reach dispatch.
  - Change: Consume the entire rejected frame before resuming the original framing. If safe recovery is impossible, terminate the session and record that contract change.
  - Risk: Test switches in both directions with fragmented input. Assert that the rejected payload never executes and that recovery preserves only subsequent valid messages.

- **agent-surfaces/F13** (bug): Waiting for an import depends on its position in the newest 100 items
  - Where: `crates/dettivo-cli/src/history.rs:351` — `"limit": 100, "cursor": null`; `crates/dettivo-cli/src/transfer.rs:320` — the same query; `crates/dettivod/src/handlers/transcripts.rs:131` — item facts already expose `status`.
  - Why: Both wait loops search one listing page and report “is no longer listed” when the target is absent. A long-running item can leave that page as newer items arrive, although it still exists and its job continues.
  - Change: Poll `transcripts.get` by reference and read its item status. Delete the duplicated listing searches.
  - Risk: Test a running item behind more than 100 newer rows, cancellation, deletion and normal completion.

- **agent-surfaces/F16** (contract): `--json` produces no JSON for locally detected failures
  - Where: `crates/dettivo-cli/src/output.rs:183` — JSON is printed only under `if let Some(rpc) = &failure.rpc`; `docs/guides/agents.md:177` — “With `--json` a failure prints the JSON-RPC error object on standard output”.
  - Why: Argument validation, socket failures, timeouts and local file errors generally have no daemon error object. For example, `dettivo --json call system.ping '[]'` reproducibly exits 4 with empty stdout, preventing a JSON consumer from reading the promised error shape.
  - Change: Give adapter-originated failures a documented structured error representation while preserving daemon errors unchanged. Apply it consistently to command parsing as well.
  - Risk: Test stdout, stderr and exit code together for each failure class. Preserve quiet-mode precedence.

### engines

- **engines/F2** (bug): Status requests wait for inference to finish
  - Where: `crates/dettivo-speech/src/supervisor.rs:145` takes `slot.lock()` and retains it through `f(process, &loaded)` at line 213. Its status path takes the same lock at line 361. `crates/dettivo-speech/src/llm/mod.rs:189` calls this blocking status path before reaching `with_running`.
  - Why: A status request cannot observe a busy engine until that engine releases its inference lock. Consequently, the LLM’s intended `Busy` handling does not solve the problem. `system.capabilities` also reaches this path through engine tier detection.
  - Change: Keep an independently readable status snapshot and use nonblocking inspection for busy slots. Make the idle reaper skip occupied slots rather than wait behind inference.
  - Risk: Snapshot transitions must stay consistent. Hold a fake inference request open and verify that capabilities, engine status and idle checks still return promptly.

- **engines/F11** (contract): Diarization cannot transport the full supported import duration
  - Where: `crates/dettivo-engine-proto/src/frame.rs:16` sets `MAX_ATTACHMENT_BYTES` to `256 << 20`. `crates/dettivo-speech/src/diarize.rs:85` sends the entire track as one attachment. `crates/dettivo-core/src/config/schema.rs:312` defaults `max_import_seconds` to `14_400`.
  - Why: The attachment limit holds only about 2 hours 20 minutes of 16 kHz mono PCM. A supported four-hour import needs 460,800,000 bytes. The writer does not enforce the reader’s limit, so rejection happens after transfer begins.
  - Change: Define and enforce consistent duration and transport limits before reading or sending the full track. Support the advertised maximum through an appropriately bounded transport.
  - Risk: Increasing the byte ceiling also increases memory pressure. Test the largest supported duration and rejection immediately above it without allocating oversized test buffers.

### dictation

- **dictation/F19** (bug): Recording is finalized before the capture source is stopped
  - Where: `crates/dettivo-session/src/worker.rs:68` — `self.record(&src, &shared)` precedes `src.stop()`; `crates/dettivo-session/src/worker.rs:324` — `while let Ok(event) = src.events().try_recv()`; `crates/dettivo-audio/src/service.rs:171` — stop only sends `Cmd::Stop(self.id)`.
  - Why: The worker drains an actively producing channel until it happens to be empty, finalizes the take, and only then requests capture stop. Buffers still in flight can arrive after that drain and be omitted. Queue emptiness is not an acknowledged capture boundary.
  - Change: Request capture stop first, then drain through a defined terminal acknowledgement before finalizing the take. Bound waiting when the source fails.
  - Risk: The source must reliably emit its terminal event after its final accepted PCM. Test a source that delivers its last chunk during stop, plus a source that fails to acknowledge.

- **dictation/F20** (bug): Device-follow subscriptions can miss changes during capture startup
  - Where: `crates/dettivo-audio/src/capture.rs:74` — `Target::Default => graph.default_node()`; `crates/dettivo-audio/src/capture.rs:198` — `drop(guard)`; `crates/dettivo-audio/src/capture.rs:208` — `graph.watch_default()`; line 236 — `graph.watch_removed()`.
  - Why: Capture selects a node and connects before subscribing to default changes or node removal. A change in that interval is not replayed. A pinned stream can consequently remain alive on a vanished device feeding silence, or a default change can escape the take’s device-change handling.
  - Change: Establish subscriptions and capture the initial graph state consistently before connecting. Reconcile the selected node with that state instead of relying solely on future notifications.
  - Risk: Graph-loop locking and callback ordering matter. Test removal and default changes between selection, connection and subscription, without timing-dependent sleeps.

### meetings

- **meetings/F5** (bug): Failed or interrupted finalisation has no supported retry path
  - Where: `crates/dettivod/src/history.rs:82` — `store.fail_stale_meeting_jobs(...)`; `crates/dettivod/src/meetings.rs:425` — recovery selects only `Recording` and `Stopping`; `crates/dettivod/src/handlers/meetings_recovery.rs:89` — `row.status != MeetingStatus::Partial`.
  - Why: A restart during final transcription marks the meeting failed before capture recovery runs. Failed and cancelled finalisations retain audio, but recovery accepts only partial rows, and stop does not restart transcription. The retained inputs become inaccessible through the meeting’s recovery workflow.
  - Change: Make interrupted capture finalisations recoverable. Provide the same explicit retry operation for failed or cancelled finalisation when validated audio remains.
  - Risk: Imports and captured meetings have different input layouts. Test restart during transcription, engine failure and cancellation for both, preserving the original meeting ID and notes.

- **meetings/F6** (bug): A microphone cannot return after an unsuccessful reopen
  - Where: `crates/dettivo-meeting/src/worker.rs:228` — `None => return Ok(busy)`; line 323 — one `open_microphone(...)` attempt; line 360 — failure only records a gap.
  - Why: Once the microphone ends, its source is cleared. If the immediate reopen finds no device, nothing subsequently watches for its return or retries opening it. The loop keeps reporting recording while permanently capturing only the system track, or nothing in a microphone-only meeting.
  - Change: Keep device availability observation independent of the capture handle, or retry reopening at a bounded interval while the microphone is absent. Start a new take with a gap when it returns.
  - Risk: Repeated attempts must not create empty takes or busy-loop. Unplug the only microphone, wait until reopening fails, reconnect it, and verify later speech is captured.

- **meetings/F7** (bug): Missing capture files are treated as a successful silent meeting
  - Where: `crates/dettivo-meeting/src/finalize.rs:65` — `read_sidecar(&sidecar).unwrap_or_default()`; line 68 — `take.samples > 0 && path.is_file()`; line 259 — `Ok(Outcome { ... })`.
  - Why: An unreadable manifest becomes an empty manifest, and a missing referenced WAV is silently omitted. Finalisation can therefore complete with one side missing or an empty transcript. This also compounds take-close failures, which `crates/dettivo-meeting/src/worker_end.rs:299` merely logs as `"meeting: take not closed"`.
  - Change: Return errors for missing or unreadable expected inputs and propagate take-finalisation failures. Distinguish an intentionally absent system track and a valid empty capture from damaged capture data.
  - Risk: Microphone-only meetings must remain valid. Test truncated manifests, missing referenced takes and failed take closure; none should become a completed silent meeting or trigger destructive cleanup.

- **meetings/F8** (bug): Recovery discards the timing information its checkpoint preserved
  - Where: `crates/dettivo-meeting/src/recovery.rs:53` — `files.sort()`; lines 58–59 — `start_offset_ms: 0`, `start_offset_ns: 0`; line 86 — the checkpoint is read only after track repair.
  - Why: When a sidecar is unreadable, recovery enumerates WAVs lexically and assigns every take offset zero. It never uses the checkpoint’s take offsets. Several sequential takes then overlap at the beginning; lexical ordering also puts numbered takes before the original filename. The speaker-track reader can truncate earlier audio when consuming those zero-offset takes.
  - Change: Recover take identity, order and offsets from the checkpoint and journal, reconciling them with files on disk. Report unknown timing explicitly instead of inventing zero offsets. Calculate recovered duration from the latest take end, including gaps.
  - Risk: Older or absent checkpoints need a conservative fallback. Corrupt the sidecar of a meeting with three takes and known gaps; verify transcript order, total duration and the complete diarization input.

- **meetings/F14** (bug): Combined search can hide every meeting hit
  - Where: `crates/dettivod/src/handlers/transcripts.rs:307` — dictation hits are appended first; line 331 — meetings follow; line 347 — `items.truncate(limit as usize)`.
  - Why: With at least `limit` matching dictations, every meeting is discarded regardless of relevance or recency. There is no subsequent page through this handler. A meeting found by meeting search can therefore be invisible in combined history search.
  - Change: Define and apply one ordering across both kinds before enforcing the limit. Do not assume independently calculated FTS ranks are directly comparable.
  - Risk: Search ordering will change. Test more than one page’s worth of dictation matches plus an exact meeting-title match and ensure the meeting remains reachable.

- **meetings/F17** (contract): An empty notes override restores the saved notes
  - Where: `crates/dettivo-storage/src/meeting_export.rs:129` — `.filter(|n| !n.trim().is_empty())` before falling back to saved notes; `qt/host/app/history_actions.cpp:179` — `if (!notesOverride.isEmpty())`.
  - Why: Clearing the live editor and exporting can include the old saved notes. Both the client and renderer collapse “explicitly empty override” into “no override,” violating the documented precedence.
  - Change: Preserve the distinction between absent and explicitly empty overrides. Choose the override first, then decide whether the resulting Notes section is empty.
  - Risk: Callers currently using an empty string to mean “use saved notes” must pass absence instead. Add Markdown and JSON cases for absent, empty, whitespace-only and nonempty overrides.

- **meetings/F18** (bug): Valid speaker names can break subtitle exports
  - Where: `crates/dettivod/src/handlers/meetings_speakers.rs:48` — validation checks character count; `crates/dettivo-storage/src/meeting_export.rs:165` — `"{}\n{} --> {}\n{}: {}\n\n"` interpolates the label unchanged.
  - Why: A name such as `Ada\n\nBob` passes validation and inserts a subtitle block separator into the cue payload. The same unrestricted interpolation exists in VTT. Ordinary golden fixtures cannot establish that arbitrary valid meeting content produces valid subtitles.
  - Change: Reject control characters in speaker names and normalize subtitle payload line breaks; escape format-specific markup where necessary. Keep stored transcript text unchanged.
  - Risk: Existing names containing control characters need safe rendering. Validate generated subtitles with a parser using multiline text, blank lines and markup characters, alongside the existing goldens.

### qa-rig

- **qa-rig/F6** (bug): An Omarchy-bar capture failure can leave live dictation running
  - Where: `crates/dettivo-qa/src/scenarios/omarchy_bar.rs:175`: `Self::live_json(ctx, &["dictation", "start"])?;`; `crates/dettivo-qa/src/scenarios/omarchy_bar.rs:182`: `Self::capture(ctx, "listening")?;`; stop occurs at line 187.
  - Why: Status and screenshot operations can return early after recording starts. This scenario uses the live daemon, so profile process cleanup cannot end that recording.
  - Change: Acquire a scoped session guard immediately after a successful start. Cancel the session on every unsuccessful exit and report cleanup failures.
  - Risk: Cleanup must affect only the session this drive started. Inject a screenshot failure after start and verify that recording ends without touching a preexisting session.

### qa-packs

- **qa-packs/F15** (contract): Promotion compares unrelated datasets and can pass without the requested comparison
  - Where: `crates/dettivo-qa/src/polish_eval/mod.rs:96` — `Ok(Incumbent { model: report.model, backend: report.backend, metrics: report.metrics.as_map() })`; `crates/dettivo-qa/src/polish_eval/gate.rs:175` — `if inc.backend != candidate_backend`; `crates/dettivo-qa/src/polish_eval/gate.rs:231` — `passed: failures.is_empty()`.
  - Why: Loading the incumbent discards its dataset, split and row identities. Two different evaluation sets can therefore produce a relative promotion verdict. A backend mismatch skips the relative gates without preventing `passed`, even when the caller explicitly supplied an incumbent.
  - Change: Require compatible dataset content, selected rows, scoring rules and execution conditions for relative comparisons. Report absolute acceptance separately from promotion; an unavailable requested comparison cannot establish promotion.
  - Risk: Existing reports lack some compatibility metadata. Check different sets with identical filenames, different splits and backend mismatches.

- **qa-packs/F18** (bug): A failed approval command can already have replaced baselines
  - Where: `crates/dettivo-qa/src/visual/approve.rs:51` — `"{}/{}: {e}; nothing approved"`; `crates/dettivo-qa/src/visual/approve.rs:69` — `std::fs::copy(&png, &path)`; `crates/dettivo-qa/src/visual/approve.rs:70` — `record(repo, &entry, &path, artboard_score, &who, &today)?`.
  - Why: Each entry is copied and recorded before the next entry is rendered. If a later render fails, the command says “nothing approved” despite earlier replacements. A log-write failure can also leave a replaced baseline without its provenance row.
  - Change: Render and validate the complete selection before replacing baselines. Publish the selected files and provenance together, or explicitly report any partial completion and changed entries.
  - Risk: Staging costs temporary storage. Fail the second render and the provenance write, then verify the baseline set and log remain consistent.

### qt-hosts

- **qt-hosts/F4** (bug): A malformed WAV header can hang the GUI during file selection
  - Where: `qt/host/app/meetings_actions.cpp:55`, `const quint32 size = qFromLittleEndian<quint32>(...)`; `qt/host/app/meetings_actions.cpp:60`, `pos += 8 + int(size) + (size % 2);`.
  - Why: The scanner narrows an untrusted unsigned chunk length to `int` without checking bounds or forward progress. A chunk size of `0xfffffff8` makes the increment zero on the target toolchain, repeatedly scanning the same chunk. Other sizes can move the cursor backwards. The import dialog invokes this synchronously while its path field changes.
  - Change: Delete this second unchecked WAV walker. Use one bounded metadata reader with checked offsets, chunk-length validation and guaranteed forward progress.
  - Risk: Valid WAVs can contain unknown and padded chunks. Test those alongside truncated headers, oversized lengths and the zero-progress example.

- **qt-hosts/F8** (contract): App renders bypass the negative style check entirely
  - Where: `qt/apps/dettivo-app/app_main.cpp:389`, `const QImage image = window->grabWindow();`, followed by `result = image.save(target) ? 0 : 1;`; `qt/host/render.cpp:39`, `const QStringList findings = styleFindings(window, fontFamily);`.
  - Why: The app has its own grab/save/quit implementation instead of calling the shared renderer. It neither plants `DETTIVO_QA_PLANT` nor scans the tree. Consequently, its many routes can render successfully with a stock control or wrong font, contrary to ADR 0021’s “every `--render`” requirement.
  - Change: Delete the app’s duplicate render block and use `renderAndQuit()`. Register positive and planted-negative app render tests.
  - Risk: Preserve sample population before capture. Both plant variants must produce the expected finding and exit 3; ordinary app renders must pass.

- **qt-hosts/F11** (bug): Opening the app during a meeting does not recover the live meeting
  - Where: `qt/host/app/meeting_live_model.cpp:43`, `if (!m_id.isEmpty()) attach(m_id);`; line 423 adopts an unknown meeting only upon a `"recording"` event.
  - Why: A freshly opened app has no meeting ID. If recording began before subscription, no code discovers that already-active meeting. Its segment events are ignored because their ID does not match the empty ID. Lost meeting-state events also remain unreconciled after overflow because the live model does not subscribe to the overflow signal.
  - Change: Discover active meetings on connection, attach to the current one, and reconcile meeting state after overflow. Coordinate that refresh with subscription establishment.
  - Risk: Test CLI-started recording before app launch, temporary connection loss, and a dropped completion event. Preserve the ADR’s explicit limitation on replaying earlier live segments.

- **qt-hosts/F16** (bug): Theme pacing can credit a frame that still contains the old palette
  - Where: `qt/host/pacing/pacing_collector.cpp:136`, `m_pendingThemeChange = int(m_themeChanges.size()) - 1;`; line 43 assigns `frame_after_ms` at the next swap.
  - Why: A frame can already have synchronized before the GUI thread applies the theme. Its later swap clears the pending change even though that frame contains the previous palette. This understates application time. The app also maintains a separate theme timing implementation, so its journal and the shared pacing report need not agree.
  - Change: Associate the theme generation with scene-graph synchronization and complete it only when a corresponding frame swaps. Use that collector for the app journal too.
  - Risk: Keep render-thread timestamps and synchronization safe. Test a change after synchronization but before swap, consecutive changes and a hidden window.

### qml

- **qml/F1** (bug): Selecting a segment disconnects it from the daemon’s value
  - Where: `qt/qml/Dettivo/components/SegmentedControl.qml:71` — `root.currentIndex = segment.index;`. `qt/qml/Dettivo/app/settings/SettingChoice.qml:44` — `currentIndex: root.currentIndex`.
  - Why: The click assigns over the caller’s binding. Subsequent configuration changes stop updating that selector; a rejected write can also leave it displaying an uncommitted value. The isolated probe selected index 1, changed the external value to 2, and still displayed 1. This follows [Qt’s binding-removal semantics](https://doc.qt.io/qt-6/qtqml-syntax-propertybinding.html).
  - Change: Delete the internal assignment. Emit the selection request and let the owning model supply the displayed value.
  - Risk: Standalone callers must own their selection explicitly. Test accepted writes, rejected writes and external changes after the first click on Home, Settings and the bar panel.

- **qml/F2** (bug): Accessibility actions bypass the behavior that makes controls work
  - Where: `qt/qml/DettivoStyle/Switch.qml:26` — `Accessible.onToggleAction: control.toggle()`. `qt/qml/Dettivo/app/settings/SettingSwitch.qml:23` — `onClicked: root.toggled(control.checked)`. `qt/qml/DettivoStyle/TabButton.qml:23` — `Accessible.onPressAction: control.clicked()`.
  - Why: Toggling the switch changes its check state without firing the handler that writes the setting. Emitting a tab’s `clicked` signal does not select it. Both behaviors reproduced in an isolated Qt probe. The existing style test calls `toggle()` and checks appearance, so it misses the absent application action.
  - Change: Remove redundant accessibility overrides where Qt already supplies the behavior; otherwise invoke the actual activation method. Qt 6.8, the repository’s floor, provides [AbstractButton.click()](https://doc.qt.io/qt-6/qml-qtquick-controls-abstractbutton.html#click-method).
  - Risk: Avoid double activation. Test accessibility actions through to one configuration write or one tab change, rather than checking only `checked`.

- **qml/F6** (bug): Transcript text can be interpreted as markup instead of displayed faithfully
  - Where: `qt/qml/Dettivo/app/history/DetailBlock.qml:86` — `text: root.body`. `qt/qml/Dettivo/app/meetings/TranscriptRow.qml:143` — `text: root.text`. Neither sets `textFormat`.
  - Why: Qt Text defaults to automatic format detection. A transcript containing recognized markup, such as `<b>example</b>`, can lose its literal tags and change presentation. This matters particularly for a speech workstation used to write code. See [Qt’s text-format behavior](https://doc.qt.io/qt-6/qml-qtquick-text.html#textFormat-prop).
  - Change: Set `Text.PlainText` for verbatim transcript, title, speaker and analysis strings. Reserve rich text for explicitly assembled, escaped content.
  - Risk: Audit intentional rich-text labels separately. Check literal tags, ampersands, multiline code and search highlighting.

- **qml/F7** (bug): A settings field can silently suppress a later valid edit
  - Where: `qt/qml/Dettivo/app/settings/SettingField.qml:24` — `property string committedText: ""`; line 49 requires `field.text !== root.committedText` before committing on blur.
  - Why: The suppression value survives indefinitely. After committing B, receiving B, and later receiving A from elsewhere, editing back to B and leaving the field emits no write. The isolated probe produced only one commit across that sequence.
  - Change: Limit duplicate suppression to the Enter/blur pair for one editing session. Clear it on a new session and when the authoritative value changes.
  - Risk: Preserve the useful protection against duplicate writes. Test Enter followed by blur, failed-write retries, and A→B→external A→B.

- **qml/F8** (bug): Popup opacity was fixed locally instead of in the style
  - Where: `qt/qml/DettivoStyle/Popup.qml:38` and `qt/qml/DettivoStyle/Menu.qml:57` — `color: Theme.roleRaisedSurface`. `qt/qml/Dettivo/Theme.qml:93` defines that role as an alpha wash. `qt/qml/Dettivo/app/meetings/DialogFrame.qml:13` supplies the local fix with `Qt.tint(Theme.roleSurface, Theme.roleRaisedSurface)`.
  - Why: The default popup and menu backgrounds remain translucent, allowing underlying content to compete with their text. ADR 0038 records this exact problem, but its opaque frame is applied only to meetings dialogs. History dialogs and combo-box popups retain the shared defect.
  - Change: Compose an opaque floating surface inside DettivoStyle. Delete the meetings-only frame and its repeated overrides once the shared style supplies it.
  - Risk: Inline raised surfaces should retain their washes. Check menus, dropdowns and every dialog over dense content in dark and light palettes.

- **qml/F16** (bug): Home’s “Start meeting” is a permanently dead action
  - Where: `qt/qml/Dettivo/app/InstrumentStrip.qml:124` — the button has `enabled: false`, `text: qsTr("Start meeting")`, and no action handler.
  - Why: Meetings are implemented, but Home still contains the disabled placeholder from an earlier slice. Nothing about daemon readiness or meeting capability can enable it.
  - Change: Route the action into the existing meeting setup/start flow. Reuse its disclosure and configuration handling.
  - Risk: Check connected, disconnected and already-recording states, plus first-use disclosure. Avoid implementing a second start workflow in Home.

### ops-and-record

- **ops-and-record/F6** (bug): “Stop meeting” always sends invalid parameters
  - Where: `omarchy/DettivoState.qml:112` sends `["call", "meetings.stop", "{}"]`; `crates/dettivo-proto/src/methods/meetings.rs:73` requires `pub meeting_id: Id`.
  - Why: The daemon cannot deserialize the empty object. The plugin retains no active meeting ID and launches commands detached, so the rejection never reaches the panel. First-use meeting start can likewise fail silently at the disclosure gate.
  - Change: Retain the active meeting ID and pass it to stop. Observe action results and route first-use disclosure through the app.
  - Risk: Avoid stopping a stale meeting after reconnect. Test start and stop through a real daemon, including missing disclosure and an already-finished meeting.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** daemon/F7, Jobs report completion even when the result was not stored: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** daemon/F16, An explicit null request ID is treated as an absent ID: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** daemon/F17, Overflow accounting can leave dropped events unreported indefinitely: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** agent-surfaces/F3, Process-hosted REST uploads exceed the daemon’s default line limit: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** agent-surfaces/F6, MCP does not enforce its outgoing message cap: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** agent-surfaces/F9, A refused MCP framing switch still executes its payload: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** agent-surfaces/F13, Waiting for an import depends on its position in the newest 100 items: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** agent-surfaces/F16, `--json` produces no JSON for locally detected failures: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** engines/F2, Status requests wait for inference to finish: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R10:** engines/F11, Diarization cannot transport the full supported import duration: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R11:** dictation/F19, Recording is finalized before the capture source is stopped: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R12:** dictation/F20, Device-follow subscriptions can miss changes during capture startup: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R13:** meetings/F5, Failed or interrupted finalisation has no supported retry path: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R14:** meetings/F6, A microphone cannot return after an unsuccessful reopen: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R15:** meetings/F7, Missing capture files are treated as a successful silent meeting: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R16:** meetings/F8, Recovery discards the timing information its checkpoint preserved: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R17:** meetings/F14, Combined search can hide every meeting hit: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R18:** meetings/F17, An empty notes override restores the saved notes: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R19:** meetings/F18, Valid speaker names can break subtitle exports: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R20:** qa-rig/F6, An Omarchy-bar capture failure can leave live dictation running: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R21:** qa-packs/F15, Promotion compares unrelated datasets and can pass without the requested comparison: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R22:** qa-packs/F18, A failed approval command can already have replaced baselines: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R23:** qt-hosts/F4, A malformed WAV header can hang the GUI during file selection: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R24:** qt-hosts/F8, App renders bypass the negative style check entirely: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R25:** qt-hosts/F11, Opening the app during a meeting does not recover the live meeting: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R26:** qt-hosts/F16, Theme pacing can credit a frame that still contains the old palette: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R27:** qml/F1, Selecting a segment disconnects it from the daemon’s value: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R28:** qml/F2, Accessibility actions bypass the behavior that makes controls work: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R29:** qml/F6, Transcript text can be interpreted as markup instead of displayed faithfully: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R30:** qml/F7, A settings field can silently suppress a later valid edit: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R31:** qml/F8, Popup opacity was fixed locally instead of in the style: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R32:** qml/F16, Home’s “Start meeting” is a permanently dead action: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R33:** ops-and-record/F6, “Stop meeting” always sends invalid parameters: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R34:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0055 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.

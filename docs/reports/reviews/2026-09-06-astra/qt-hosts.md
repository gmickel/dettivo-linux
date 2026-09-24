# The Qt hosts: C++ models, the QA environment, rendering and pacing

## Verdict

The separation between daemon state, C++ presentation models and reusable QML surfaces is sound, but the hosts are not ready to call complete: persistence can lose user data, several actions violate the daemon contract, and QA misses important failures. The single most valuable change is to stop the app overwriting daemon-owned runtime state when it closes. This was a read-only source and contract review; all 122 C++/header/QML files in scope meet the file limits, but `just build test lint` was not run because it writes build artifacts.

## Findings

### F1. Closing the app can erase a meeting acknowledgement recorded while it was open

- **Kind:** bug
- **Where:** `qt/host/app/app_state.cpp:92`, `s.otherTables = QString::fromStdString(rest.str()).trimmed();`; `qt/host/app/app_state.cpp:110`, `out += QStringLiteral("\n") + otherTables + QStringLiteral("\n");`.
- **Why:** The app preserves daemon-owned tables from its startup snapshot, then writes that snapshot back on exit. Meanwhile, `crates/dettivod/src/meetings.rs:410` independently loads and updates the same file to record disclosure acknowledgement. Opening the app, acknowledging disclosure and closing the app can therefore restore the old acknowledgement. Atomic replacement prevents partial files; it does not prevent this lost update.
- **Change:** Give the file one writer, or move app-owned state into a separate file. Delete the app’s preservation and rewriting of daemon-owned tables.
- **Risk:** Existing window geometry and first-run completion need migration. Test acknowledgement and daemon restart while the app remains open, then close and reopen it.

### F2. Meeting notes stop being protected before the daemon confirms their save

- **Kind:** bug
- **Where:** `qt/host/app/meeting_detail_model.cpp:415` and `qt/host/app/meeting_live_model.cpp:257`, both `m_notesDirty = false;`; `qt/host/app/meeting_detail_model.cpp:140`, `if (!m_notesDirty)`.
- **Why:** Both models clear the dirty flag before sending the save. A failure leaves it cleared, so neither retries that edit. The detail model also accepts incoming notes whenever that flag is clear, allowing a reload during an outstanding save to overwrite the local draft. Closing the application before the debounce fires has no notes-flush path.
- **Change:** Track the draft revision separately from the acknowledged revision. Preserve failed and outstanding drafts, serialize saves per meeting, and finish or retain pending edits before switching meetings or closing. Share this small save mechanism between the two models.
- **Risk:** Late acknowledgements must not mark newer text saved or affect another meeting. Test delayed replies, disconnects, rapid edits, meeting switches and immediate window closure.

### F3. A failed export can destroy an existing destination file

- **Kind:** bug
- **Where:** `qt/host/app/history_actions.cpp:271`, `file.open(QIODevice::WriteOnly | QIODevice::Truncate)`; `qt/host/app/history_actions.cpp:278`, `m_link->call(QStringLiteral("transfer.pull"), params,`.
- **Why:** Export truncates the chosen destination before retrieving the first chunk. A daemon disconnect or transfer failure leaves an empty or partial file where the user’s previous export existed.
- **Change:** Keep one `QSaveFile` for the transfer and commit only after successful completion. Delete the repeated truncate/open/append sequence.
- **Risk:** Preserve overwrite confirmation and cancellation behavior. Export over a sentinel file, fail the first and a later chunk, and verify the sentinel survives unchanged.

### F4. A malformed WAV header can hang the GUI during file selection

- **Kind:** bug
- **Where:** `qt/host/app/meetings_actions.cpp:55`, `const quint32 size = qFromLittleEndian<quint32>(...)`; `qt/host/app/meetings_actions.cpp:60`, `pos += 8 + int(size) + (size % 2);`.
- **Why:** The scanner narrows an untrusted unsigned chunk length to `int` without checking bounds or forward progress. A chunk size of `0xfffffff8` makes the increment zero on the target toolchain, repeatedly scanning the same chunk. Other sizes can move the cursor backwards. The import dialog invokes this synchronously while its path field changes.
- **Change:** Delete this second unchecked WAV walker. Use one bounded metadata reader with checked offsets, chunk-length validation and guaranteed forward progress.
- **Risk:** Valid WAVs can contain unknown and padded chunks. Test those alongside truncated headers, oversized lengths and the zero-progress example.

### F5. The meeting engine picker cannot reliably change the daemon’s provider

- **Kind:** contract
- **Where:** `qt/host/app/meetings_actions.cpp:306`, `QJsonObject params{{QStringLiteral("provider_id"), providerId}};`; line 308 sends `"model_id"`. `crates/dettivo-proto/src/methods/speech.rs:125` defines `pub provider: Option<String>`, and line 128 defines `pub model: Option<String>`.
- **Why:** The request uses response-field names that the request schema rejects. There is another failure when the current provider is Parakeet: `applyProviders()` substitutes the first meeting-capable provider into `m_selectedProvider` without changing the daemon. Picking that displayed provider then takes the “same provider” branch and only writes its meeting model.
- **Change:** Send `provider` and `model`. Keep the daemon’s actual selection separate from the picker’s offered fallback, and confirm the selection write before starting.
- **Risk:** Changing providers also changes dictation, as ADR 0038 records. Test starting a meeting from an initial Parakeet configuration through the actual daemon contract.

### F6. Home’s Start action ignores the selected dictation mode

- **Kind:** contract
- **Where:** `qt/host/app/status_model.cpp:221`, `QJsonObject{{QStringLiteral("mode"), QStringLiteral("raw")}}`; `qt/qml/Dettivo/app/InstrumentStrip.qml:59`, `root.config.set("dictation.mode", chosen);`.
- **Why:** The mode control writes the user’s choice, but Start always explicitly requests Raw. Home can display Enhanced or Polish while starting a Raw take.
- **Change:** Remove the hardcoded override and let the daemon use the configured mode.
- **Risk:** Check Raw, Polish and Enhanced through the Home button, comparing the completion and stored transcript mode with the displayed selection.

### F7. First run’s local-model download is still an unfinished integration

- **Kind:** contract
- **Where:** `qt/host/app/first_run_models.cpp:370`, `m_link->call(QStringLiteral("llm.models.download"), QJsonObject(), ...`; `crates/dettivo-proto/src/methods/llm.rs:111`, `pub model: String`.
- **Why:** The download request omits its required model, and its callback discards the error. Additionally, provider refresh starts before capabilities are read; `setLlmMethods()` only stores the later answer, so the initial local-model row can remain “soon” even when downloading is supported.
- **Change:** Replace the obsolete provisioning stub with the existing catalogue/status/download operations used by Settings. Send the selected model ID, recompute availability when capabilities arrive, and retain download failures on the row.
- **Risk:** Test a fresh profile with no language model, delayed capability replies, a successful download and a refused download. The optional choice must remain skippable.

### F8. App renders bypass the negative style check entirely

- **Kind:** contract
- **Where:** `qt/apps/dettivo-app/app_main.cpp:389`, `const QImage image = window->grabWindow();`, followed by `result = image.save(target) ? 0 : 1;`; `qt/host/render.cpp:39`, `const QStringList findings = styleFindings(window, fontFamily);`.
- **Why:** The app has its own grab/save/quit implementation instead of calling the shared renderer. It neither plants `DETTIVO_QA_PLANT` nor scans the tree. Consequently, its many routes can render successfully with a stock control or wrong font, contrary to ADR 0021’s “every `--render`” requirement.
- **Change:** Delete the app’s duplicate render block and use `renderAndQuit()`. Register positive and planted-negative app render tests.
- **Risk:** Preserve sample population before capture. Both plant variants must produce the expected finding and exit 3; ordinary app renders must pass.

### F9. Concurrent launches can create two apps and steal each other’s control socket

- **Kind:** bug
- **Where:** `qt/host/app/app_control.cpp:69`, `QLocalServer::removeServer(m_path);`; `qt/apps/dettivo-app/app_main.cpp:165`, `AppControl::forward(...)`, with `control.listen(...)` deferred until line 403.
- **Why:** There is a substantial gap between checking for an existing app and claiming the socket. Two launches can both pass the check, initialize their windows and then unlink each other’s listening socket. A slow existing app can also be mistaken for a crashed one. The destructor subsequently removes the pathname without establishing that it still belongs to this instance.
- **Change:** Acquire exclusive instance ownership before GUI initialization. Only the owner may remove a stale socket; other launches should forward or wait for startup.
- **Risk:** Exercise simultaneous cold launches, slow startup, a stale socket and an unresponsive live instance. Exactly one window and one usable control endpoint should remain.

### F10. An old OSD completion can overwrite a newer take

- **Kind:** bug
- **Where:** `qt/host/osd/osd_model.cpp:264`, the delayed callback captures `payload, job`; line 265 checks only `m_state == QStringLiteral("transcribing")`.
- **Why:** Take A completes during its minimum dwell and schedules a timer. Take B starts and reaches Transcribing before that timer fires. The condition succeeds, so A’s completion replaces B’s state and text. Capturing A’s job ID does not help because it is used only for logging.
- **Change:** Cancel the pending completion on a new session or guard it with a session generation. A matching state name is insufficient.
- **Risk:** Test two short takes inside the dwell interval, plus cancellation and daemon disconnection while a completion is pending.

### F11. Opening the app during a meeting does not recover the live meeting

- **Kind:** bug
- **Where:** `qt/host/app/meeting_live_model.cpp:43`, `if (!m_id.isEmpty()) attach(m_id);`; line 423 adopts an unknown meeting only upon a `"recording"` event.
- **Why:** A freshly opened app has no meeting ID. If recording began before subscription, no code discovers that already-active meeting. Its segment events are ignored because their ID does not match the empty ID. Lost meeting-state events also remain unreconciled after overflow because the live model does not subscribe to the overflow signal.
- **Change:** Discover active meetings on connection, attach to the current one, and reconcile meeting state after overflow. Coordinate that refresh with subscription establishment.
- **Risk:** Test CLI-started recording before app launch, temporary connection loss, and a dropped completion event. Preserve the ADR’s explicit limitation on replaying earlier live segments.

### F12. Requests can remain pending forever while the socket still appears connected

- **Kind:** bug
- **Where:** `qt/host/daemon/daemon_client.cpp:102`, `m_pending.insert(id, std::move(reply));`; line 103 calls `m_socket.write(...)` without checking its result.
- **Why:** Pending callbacks complete only on a matching reply or disconnection. There is no deadline. A daemon that accepts the socket but stops answering leaves actions permanently busy and loading indicators unresolved. Periodic reads can continue accumulating callbacks.
- **Change:** Add request deadlines and handle write failures through the same completion path. Expiry must fail each callback exactly once; it must not automatically replay mutations.
- **Risk:** A timed-out mutation may already have succeeded. Report that uncertainty and refresh its state. Test an accepted-but-silent connection, late replies and disconnect during expiry.

### F13. Model actions immediately erase their own failure message

- **Kind:** bug
- **Where:** `qt/host/app/models_table.cpp:305–308`, `emit refused(m_lastRefusal);` followed by `refresh();`; `qt/host/app/models_table.cpp:109–110`, `m_lastRefusal.clear();` followed by `emit refused(QString());`.
- **Why:** Every refused download, deletion or selection emits its error and then clears it synchronously through refresh. QML can receive the failure and empty string before rendering a frame, leaving the user with no explanation.
- **Change:** Keep the action error until dismissal, another explicit attempt or successful resolution. Background refresh must not clear it.
- **Risk:** Avoid retaining an obsolete error after success. Assert the error remains observable after the refresh replies finish.

### F14. Meeting import buffers and hashes the entire recording on the GUI thread

- **Kind:** simplify
- **Where:** `qt/host/app/meetings_actions.cpp:365`, `upload.bytes = file.readAll();`; line 411 hashes `upload.bytes` in one operation.
- **Why:** The transfer protocol already supports chunks, but the host first retains the whole recording and later hashes it synchronously. Memory therefore scales with recording size, and both operations block window input and painting. Large files can exhaust memory before the daemon evaluates their size.
- **Change:** Remove `Upload::bytes`. Stream bounded chunks from an open file, hash incrementally, and perform blocking media I/O outside the GUI thread.
- **Risk:** Preserve chunk ordering and the final digest. Test a large recording, cancellation, read failure and a file changed during upload.

### F15. The visual metric is too permissive to enforce approved-render fidelity

- **Kind:** simplify
- **Where:** `qt/tools/visual-diff/main.cpp:37`, `constexpr int kRows = 18;`; line 87 scales with `Qt::IgnoreAspectRatio`. `docs/adr/0021-visual-regression-gate-and-frame-pacing.md:26` records that a font-size increase “passed the 74 entries held to approved renders.”
- **Why:** The comparator discards dimensions, most spatial detail and absolute colour information, then accepts shifted correlations. Those allowances help compare different artboards and fonts, but also normalize away regressions between a render and its approved counterpart. The record already demonstrates the resulting false passes.
- **Change:** Use a stricter comparison for approved renders of the same theme and scale, with exact dimensions and explicit rasterization tolerance. Retain the permissive artboard score only as advisory evidence.
- **Risk:** Pin the rendering environment before tightening tolerance. Check small independent regressions in colour, typography, spacing and missing controls, rather than relying only on the combined canary.

### F16. Theme pacing can credit a frame that still contains the old palette

- **Kind:** bug
- **Where:** `qt/host/pacing/pacing_collector.cpp:136`, `m_pendingThemeChange = int(m_themeChanges.size()) - 1;`; line 43 assigns `frame_after_ms` at the next swap.
- **Why:** A frame can already have synchronized before the GUI thread applies the theme. Its later swap clears the pending change even though that frame contains the previous palette. This understates application time. The app also maintains a separate theme timing implementation, so its journal and the shared pacing report need not agree.
- **Change:** Associate the theme generation with scene-graph synchronization and complete it only when a corresponding frame swaps. Use that collector for the app journal too.
- **Risk:** Keep render-thread timestamps and synchronization safe. Test a change after synchronization but before swap, consecutive changes and a hidden window.

### F17. The declared Qt 6.8 minimum cannot compile the history filters

- **Kind:** contract
- **Where:** `qt/CMakeLists.txt:29`, `find_package(Qt6 6.8 REQUIRED ...)`; `qt/host/app/history_model.cpp:408–409`, `beginFilterChange();` and `endFilterChange();`.
- **Why:** These APIs were introduced in Qt 6.9 and 6.10 respectively. Configuration accepts a supported 6.8 installation that will then fail compilation. [Qt’s API documentation](https://doc.qt.io/qt-6/qsortfilterproxymodel.html#protected-functions) confirms the version requirements.
- **Change:** Use the filter invalidation API available at the recorded minimum, or explicitly raise the minimum and update packaging and the ADR together.
- **Risk:** Build against the minimum supported Qt as well as the current version; verify kind filtering and the newest-day proxy after updates.

### F18. The duplicated QA parser has a known-variable hole

- **Kind:** simplify
- **Where:** `qt/host/qa_environment.cpp:18` lists `"DETTIVO_E2E_DISCLOSURE"`; line 225 enforces QA mode only when `firstHook` was populated.
- **Why:** Disclosure is recognized but never passed through `hook()` or validated. Qt therefore accepts it without QA mode and accepts invalid values that the Rust parser rejects. The larger structure duplicates daemon-specific fields and validation even though Qt does not consume most of them; the test mainly checks that 26 names appear in documentation.
- **Change:** Separate recognition and QA-mode enforcement from parsing host-consumed values. Remove unused stored fields and use shared conformance cases for the Rust and Qt parsers, including every recognized hook.
- **Risk:** Preserve inherited daemon hooks and typo rejection. Test each hook without QA mode, in a release build, and with an invalid value.

### F19. Completed first run still runs provisioning reads and a hotkey polling timer

- **Kind:** delete
- **Where:** `qt/host/app/first_run_model.cpp:85–90` unconditionally reads keys, snippets, devices, selection and models; line 106 starts polling when the step is `"keys"`. `qt/host/app/first_run_model.h:161` defaults that step to `"keys"`.
- **Why:** `markComplete()` does not deactivate this work. A provisioned app still initializes the first-run data path and starts the polling timer, alongside Settings’ duplicate hotkey and model reads. Home startup also unconditionally launches Agents checks and Doctor in `qt/apps/dettivo-app/app_main.cpp:319–321`.
- **Change:** Activate provisioning only while first run is required or explicitly reopened. Load diagnostics when requested. Reuse the existing model catalogue and hotkey facts instead of maintaining duplicate fetch-and-parse paths.
- **Risk:** Explicitly reopening onboarding and reconnecting on an active settings page must still refresh correctly. Check startup calls for a provisioned Home session.

### F20. Host tests validate invented JSON and synchronous replies instead of the socket contract

- **Kind:** test
- **Where:** `qt/host/app/fake_link.h:24–25`, `reply(answers.value(method), {});`; `qt/host/app/first_run_model_test.cpp:93` invents a `"path"` result for `config.path`; `qt/host/app/app_meetings_test.cpp:366` explicitly expects `"provider_id"` in the selection request.
- **Why:** The fake returns success by method name regardless of request shape and calls callbacks inline. It blesses F5’s rejected request and hides asynchronous save races. The invented config result also conceals a real broken action: `qt/host/app/first_run_model.cpp:415` reads `"path"` when the daemon returns `"config"`, so Open config cannot obtain the correct filename.
- **Change:** Use the checked-in contract fixtures for wire shapes and fix that field lookup. Add controllable queued replies and a small local-socket integration suite covering framing, delayed replies, subscription and reconnect behavior. Keep lightweight fakes for formatting tests.
- **Risk:** Avoid rebuilding the daemon inside a fake. Tests should reject malformed requests and exercise ordering explicitly, while the real contract suite remains authoritative.

## Keep

- `qt/host/daemon/daemon_link.h`: the narrow injectable transport interface is the right boundary.
- `qt/host/app/config_binding.cpp`: configuration changes go through the daemon, preserving one source of truth.
- `qt/host/app/router.cpp`: bounded navigation history and explicit route validation are appropriately small.
- `qt/host/pacing/pacing_collector.cpp`: direct render-thread timestamps and guarded sample storage are worth retaining.
- `qt/host/unix_signals.cpp`: the nonblocking socket-pair handoff keeps Qt work out of signal handlers.
- `qt/apps/dettivo-app/Main.qml`: explicit model injection supports reusable QML and focused component tests.
- `docs/adr/0038-meetings-gui-live-events-pause-gap-and-inline-dialogs.md`: leaving Pause disabled until a real daemon verb exists is honest scope control.

## Questions for the owner

- Should app-owned runtime state move to its own file, or should the daemon become the sole writer through an API?
- Which variations should approved-render comparisons intentionally tolerate: glyph rasterization only, or also font substitution and layout scaling?
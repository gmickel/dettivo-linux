# The QA rig: drivers, profiles, scenarios

## Verdict

The rig exercises real GUI, daemon, and audio paths, but it is not yet a trustworthy acceptance gate: profile isolation leaks, and several assertions accept broken behavior. The single most valuable change is to make `Profile` own isolated launch configuration, writable fixtures, and cleanup for every scenario. This was a read-only source audit: all six QA shell scripts pass `bash -n`, the worktree is unchanged, and GUI/audio scenarios and `just build test lint` were not run.

## Findings

### F1. Inherited environment variables can redirect a scenario into real user data

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/profile.rs:138`: `env.insert("HOME".into(), r("home"));`; `crates/dettivo-qa/src/scenarios/daemon.rs:63`: `.envs(&env)`; `crates/dettivo-core/src/paths.rs:78`: `match get(ENV_CONFIG).filter(|v| !v.is_empty())`.
- **Why:** Setting private XDG paths does not remove inherited `DETTIVO_CONFIG` or `DETTIVO_DATA_DIR`, which override those paths in the product. Several daemon and driver launchers inherit the parent environment. Settings edits or model operations can therefore reach real configuration or data. `XDG_CACHE_HOME` also remains inherited.
- **Change:** Use one profile command builder with `env_clear()`, an explicit desktop/session allowlist, private cache storage, and deliberate scenario overrides. Reuse the clearing pattern already present in `InsertDaemon`.
- **Risk:** Display, accessibility, and audio connections require selected host variables. Test every launcher with poisoned parent overrides and verify that sentinel files outside the profile remain unchanged.

### F2. Hard-linked model files are not isolated against writes

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/profile_models.rs:147`: `if std::fs::hard_link(from, to).is_err()`; `crates/dettivo-speech/src/models.rs:154`: `std::fs::write(self.manifest_path(entry), text + "\n")`.
- **Why:** The recursive copy hard-links every file, including manifests. Model verification rewrites the manifest in place, changing the shared inode in the real model tree. Existing unlink-safety tests do not establish write isolation.
- **Change:** Use reflinks with copy fallback for writable fixtures. At minimum, copy manifests and other mutable files; sharing weights requires an enforceable immutability contract.
- **Risk:** Copies consume disk and setup time. Add tests that rewrite and truncate destination manifests and weights, then verify unchanged source bytes and metadata.

### F3. The model-directory guard misses symlinked parents of new destinations

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/profile_models.rs:64`: `guard(&dir, &s.real)` precedes directory creation; `crates/dettivo-qa/src/profile_models.rs:157`: `std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())`.
- **Why:** A fresh destination normally does not exist, so canonicalization fails and the guard falls back to its lexical spelling. If its existing parent is a symlink into the real model directory, containment is missed and creation proceeds inside that directory.
- **Change:** Resolve the deepest existing ancestor, append the missing components, and validate containment before creation. Fail explicitly on resolution errors.
- **Risk:** Legitimate symlinked stores should remain usable outside the source. Test an existing symlinked parent with a nonexistent profile child, alongside safe external stores.

### F4. Scenario configuration overwrites the profile’s disabled-hotkeys baseline

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/profile.rs:85`: `"[hotkeys]\nbackend = \"none\"\n"`; `crates/dettivo-qa/src/scenarios/daemon.rs:59`: `std::fs::write(&config_file, config)`; `crates/dettivo-core/src/config/schema.rs:125`: `backend: "auto".into()`.
- **Why:** The generic launcher replaces the seeded configuration. Helpers such as `daemon_config` omit hotkeys, restoring the product’s automatic backend and allowing desktop shortcut registration or portal interaction during ordinary GUI scenarios.
- **Change:** Merge scenario settings into a profile baseline through one configuration helper. Require explicit selection of a real hotkey backend.
- **Risk:** Actual hotkey scenarios need their overrides. Check the final parsed configuration and backend status for ordinary and hotkey-specific launches.

### F5. The Hyprland hotkey scenario does not own the bindings it removes

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/scenarios/hotkeys_hyprland.rs:27`: “The chords: Ctrl+Alt+Shift with a function key sits on no default binding”; `crates/dettivo-qa/src/scenarios/hotkeys_hyprland.rs:226`: `let _ = hyprctl(&["eval", UNBIND]);`.
- **Why:** Absence from default bindings does not establish absence from Gordon’s bindings. The scenario installs fixed F9–F12 chords, then unconditionally unbinds them and ignores cleanup failures.
- **Change:** Inspect existing bindings and reserve unused chords, or run in a dedicated compositor session. Track acquired bindings and make failed restoration a scenario failure.
- **Risk:** Binding serialization may vary with Hyprland. Test a preexisting matching chord and a failed drive; the original binding must survive both.

### F6. An Omarchy-bar capture failure can leave live dictation running

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/scenarios/omarchy_bar.rs:175`: `Self::live_json(ctx, &["dictation", "start"])?;`; `crates/dettivo-qa/src/scenarios/omarchy_bar.rs:182`: `Self::capture(ctx, "listening")?;`; stop occurs at line 187.
- **Why:** Status and screenshot operations can return early after recording starts. This scenario uses the live daemon, so profile process cleanup cannot end that recording.
- **Change:** Acquire a scoped session guard immediately after a successful start. Cancel the session on every unsuccessful exit and report cleanup failures.
- **Risk:** Cleanup must affect only the session this drive started. Inject a screenshot failure after start and verify that recording ends without touching a preexisting session.

### F7. Audio-rig cleanup can unload another scenario’s sink

- **Kind:** bug
- **Where:** `scripts/qa/audio-rig.sh:25`: `pactl load-module ... >/dev/null`; `scripts/qa/audio-rig.sh:47`: `$2 == "module-null-sink" && index($0, s)`.
- **Why:** Creation discards the module ID. Cleanup reconstructs ownership through substring matching, so requesting `dettivo-qa-mic-123` also matches `dettivo-qa-mic-1234`. Creation also leaves the module loaded if its monitor never appears.
- **Change:** Return and retain the module ID from creation, unload that exact module, and roll back failed setup.
- **Risk:** Callers currently expect a monitor name. Update the small caller set together; test overlapping names and monitor-discovery failure with a stub `pactl`.

### F8. Accessibility failures can become 100% naming coverage

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/driver/atspi.rs:125`: `component.get_extents(CoordType::Screen).await.ok()`; `crates/dettivo-qa/src/a11y_tree.rs:100`: `!is_interactive(&e.role) || e.bounds.is_none()`; line 116 returns `1.0` when no interactive elements remain.
- **Why:** Unknown geometry is treated as grounds to exclude a control. Failed child reads and traversal limits also silently omit nodes. An incomplete tree can therefore receive perfect coverage; bounds absence does not prove that a control is hidden.
- **Change:** Carry snapshot completeness and visibility state explicitly. Fail or mark coverage indeterminate when required traversal or geometry fails, and assert expected controls for the surface.
- **Risk:** Hidden Qt delegates need deliberate handling. Test an unnamed visible control with failed extents, a missing subtree, and a genuinely empty surface.

### F9. First-run insertion failures are repaired before the scenario judges success

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/scenarios/first_run_fresh.rs:302`: `if outcome != "inserted" && reason.starts_with("CONFLICT")`; line 325 assigns `outcome = "inserted".into()`; `crates/dettivo-qa/src/scenarios/first_run_fresh.rs:398`: `report.insert("allowance_after_done".into(), allowance["reason"].clone());`.
- **Why:** Failed automatic insertion triggers refocusing and a separate `insert.perform` call with `allow_self_target: true`. Success replaces the original outcome and bypasses the normal backend assertion. The post-Done allowance probe merely records its response without asserting refusal.
- **Change:** Require the original insertion to succeed in the happy-path scenario. Move recovery into a separate scenario. Assert the expected post-Done refusal and absence of an insertion side effect.
- **Risk:** Shared-desktop focus changes will become visible failures. Use controlled focus and preserve the original failure evidence rather than silently repairing it.

### F10. Settings comparison accepts unequal floating-point values

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/scenarios/settings_support.rs:157`: `typed.parse::<f64>().ok() == n.as_f64() || typed.parse::<i64>().ok() == n.as_i64()`.
- **Why:** For typed `0.35` and returned `0.9`, the float comparison fails but both integer conversions yield `None`. `None == None` makes the entire predicate true. Invalid numeric text can pass for the same reason.
- **Change:** Compare only successfully parsed numeric values. Parse expected tables into complete objects as well, so surplus returned entries cannot pass a round-trip assertion.
- **Risk:** Preserve intentional daemon coercions. Add negative cases for unequal floats, invalid numbers, and unexpected table entries.

### F11. History validation substitutes row counts and unknown geometry for correct contents

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/scenarios/history_seed.rs:221`: `is_full(&tree).unwrap_or(true)`; line 223: `if shown == expected || (folded && joined)`.
- **Why:** The exact-count branch ignores the computed row-content and insertion checks. When fewer rows appear, unknown list geometry is treated as proof that the rest are below the fold. Wrong rows at the right count, or a truncated tree, can pass.
- **Change:** Check expected row identities and order in every branch. For virtualized lists, scroll and verify the required contents; report incomplete evidence when viewport capacity is unknown.
- **Risk:** Accessibility tokens may change between snapshots. Use domain row identity, and test reordered rows, equal-count wrong rows, and a truncated snapshot without bounds.

### F12. A failed history rerun counts as a settled successful rerun

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/scenarios/history_seeded.rs:337`: `if item["status"] != "transcribing" { return Ok(got); }`.
- **Why:** Anything other than `transcribing`, including failure or a missing status, satisfies the predicate. The parent link is checked, but successful transcription is not.
- **Change:** Wait for the explicit successful terminal status and assert the fixture transcript. Fail immediately on a failed terminal status, retaining its diagnostic.
- **Risk:** Valid intermediate states must remain waitable. Exercise successful inference, engine failure, and malformed status responses.

### F13. The meeting device-switch check counts audio recorded before the switch

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/scenarios/meeting_rig.rs:271`: `sys.len().saturating_sub(16_000)`; `crates/dettivo-qa/src/scenarios/meeting_rig.rs:336`: `if system_samples_after_switch < 8_000`.
- **Why:** Recording begins before a 400 ms delay, a one-second fixture, and another 300 ms delay. Subtracting only one second counts pre-switch samples as post-switch continuity. The system track can stop at the device switch and still exceed the threshold; its correlation check examines the early recording.
- **Change:** Record the actual switch boundary and verify the second system fixture after that boundary.
- **Risk:** Account for capture buffering explicitly. A regression fixture that truncates the system track at the switch must fail.

### F14. The live-transcription scenario never proves delivery while recording

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/scenarios/meeting_live.rs:192`: `sleep(Duration::from_millis(12_500))`; line 195 calls `meetings.stop`; line 199 begins `events.collect`.
- **Why:** Notifications are consumed only after stop and finalization. Checking provisional/final ordering in the collected sequence does not prove that provisional text reached a consumer while the meeting was running.
- **Change:** Collect concurrently with monotonic receive timestamps. Require provisional delivery for each source before issuing stop, then verify finalization.
- **Risk:** Avoid a machine-specific latency threshold unless the product promises one. A daemon stub that releases all notifications after stop must fail.

### F15. The cua text parser truncates labels, and scenarios accommodate the loss

- **Kind:** contract
- **Where:** `crates/dettivo-qa/src/driver/cua.rs:408`: `after_quote.find('"')`; `crates/dettivo-qa/src/scenarios/history_states.rs:224`: `if !named && longer`.
- **Why:** Markdown parsing ends a name at the first quote without decoding quoting or multiline text. The history scenario explicitly accepts only `No match for ` when the driver loses the query. The two drivers consequently enforce different product behavior.
- **Change:** Obtain complete structured text or correctly decode the pinned upstream representation. Delete prefix-only accommodations and run shared snapshot/value conformance cases against both drivers.
- **Risk:** Upstream output formats may change. Cover embedded quotes, escapes, multiline labels, and values on named controls.

### F16. Negative-text scanning misses values and exempts entire mixed-content strings

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/negative_text.rs:98`: `classify(&e.name)`; line 111: `f.name.contains(PROFILE_ROOT_PREFIX)`.
- **Why:** Only accessible names are scanned, although text may live in `Element.value`. A raw-path finding is entirely discarded when its string contains `/tmp/dq`, even if the same string also exposes another path or developer diagnostic.
- **Change:** Scan relevant product-owned names and values. Normalize only the exact current profile-path span, then classify the remaining text. Share that policy across scenario helpers.
- **Risk:** User transcripts can legitimately contain technical text. Define that boundary and test mixed profile/real paths plus diagnostics in named text fields.

### F17. Scenario timeouts do not bound the operations they wrap

- **Kind:** contract
- **Where:** `crates/dettivo-qa/src/driver/cua.rs:83`: `.output()`; `crates/dettivo-qa/src/socket.rs:20`: `stream.set_read_timeout(Some(timeout))?;`; `crates/dettivo-qa/src/scenarios/daemon.rs:208`: `Ok(0) | Err(_) => break`.
- **Why:** A blocking cua subprocess can outlive every surrounding deadline. Socket timeouts cover individual reads, not the complete request. Event collection uses a separately configured read timeout and converts transport failures into an ordinary partial event list.
- **Change:** Give subprocess and socket operations absolute deadlines, pass remaining time into blocking operations, and return explicit timeout/transport errors with partial evidence.
- **Risk:** Model loading needs longer budgets than UI actions. Test a hung subprocess, a slowly trickling response, and a silent event stream without launching real applications.

### F18. Process cleanup depends on reaching the runner’s normal teardown

- **Kind:** simplify
- **Where:** `crates/dettivo-qa/src/scenarios/daemon.rs:74`: readiness timeout returns before `DaemonHandle` construction; `crates/dettivo-qa/src/profile.rs:164`: `self.children.push((name.to_string(), child.id()));`; `crates/dettivo-qa/src/profile.rs:224`: profile drop removes its directory.
- **Why:** The profile retains PIDs rather than child ownership. Failed startup drops an unguarded `Child`; callers outside the scenario runner, including pack platform probing, can leave it running while deleting its profile. Other paths rely on duplicate driver and runner cleanup.
- **Change:** Construct a process guard immediately after spawn. Consolidate termination and reaping under explicit ownership, including startup failure, and remove stale PID-only cleanup.
- **Risk:** Avoid double termination with existing driver guards. Test a daemon that starts but never answers readiness; no child or zombie should remain after either caller exits.

### F19. Delete the first-run scenario’s second model-store lifecycle

- **Kind:** delete
- **Where:** `crates/dettivo-qa/src/scenarios/first_run_support.rs:167`: `repo_root.join("target/qa-first-run").join(name)`; `crates/dettivo-qa/src/scenarios/first_run_fresh.rs:126`: `let _ = std::fs::remove_dir_all(store);`.
- **Why:** First-run creates another store and symlink hierarchy despite the profile already owning model storage. Manual removal ignores `--keep-profile`, while failures between staging and the drive bypass removal.
- **Change:** Stage the tiny model and downloadable test model through the existing profile model tree. Keep the scenario-specific catalogue/server; delete the second store, extra link, configuration redirection, and manual cleanup.
- **Risk:** The download target must begin missing. Verify fresh download, interrupted setup, normal cleanup, and retained-profile inspection.

### F20. The Omarchy-bar visual check can pass without the widget returning to idle

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/scenarios/omarchy_bar.rs:82`: “The right third of the bar”; `crates/dettivo-qa/src/scenarios/omarchy_bar.rs:217`: `if restored.score < changed.score`.
- **Why:** Any change in a large crop can satisfy the listening check, including unrelated bar widgets. Equal listening and after scores pass restoration, so a Dettivo indicator stuck in its active appearance can pass.
- **Change:** Capture the Dettivo widget specifically and compare explicit idle/listening states. Require restoration against an idle criterion, rather than merely being no worse than the listening comparison.
- **Risk:** Theme rendering can vary. Test a frozen-active widget and an unchanged Dettivo widget beside an updating clock; both must fail.

## Keep

- `crates/dettivo-qa/src/driver/mod.rs`: Keep the small driver boundary; tighten its semantics without adding another driver framework.
- `crates/dettivo-qa/src/scenarios/meeting_token_support.rs`: Source-specific token checks and swapped-source regression coverage test meaningful behavior.
- `crates/dettivo-qa/src/scenarios/meeting_rig.rs`: Distinct microphone/system tones and cross-correlation are useful evidence of capture separation.
- `crates/dettivo-qa/src/scenarios/placeholder_window.rs`: Keep the cheap real-window smoke test; broad scenarios do not replace that diagnostic.
- `crates/dettivo-qa/src/scenarios/support.rs`: Preserve the explicit environment allowlist and daemon-log capture when consolidating launchers.

## Questions for the owner

- Must cua provide independent accessibility evidence, or can both action backends share one AT-SPI snapshot implementation? That determines whether maintaining two semantic adapters has value.
- Should compositor-mutating scenarios run exclusively in a dedicated desktop session, or support explicit execution on the owner’s active desktop?
- Should negative-text checks cover only product-authored interface text, or also imported transcripts and editable user content? The latter can legitimately contain paths, model identifiers, and diagnostics.
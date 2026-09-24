# Packaging, the Omarchy plugin, CI, the scripts and the record

## Verdict

This area assembles the product, connects it to Omarchy, and decides whether a build can ship. The structure is sensible, but the release path and plugin contain concrete failures; the most valuable change is to make clean-install acceptance require complete evidence tied to the package being released. Read-only verification passed shell syntax for 42 files, token/icon/accessibility/settings/release-text lints, and recipe probes; `just build test lint` was not run because it writes artifacts, and this host also lacks Vulkan and SPIR-V headers.

## Findings

### F1. A release rehearsal against a tag ignores `dry_run`

- **Kind:** bug
- **Where:** `.github/workflows/release.yml:50` checks `GITHUB_REF_TYPE = tag`; line 56 writes `"publish=true"` before line 58 considers `inputs.dry_run`.
- **Why:** Dispatching against a matching tag with `dry_run=true` enables publication. Executing the prepare block with those inputs reproduced `publish=true`.
- **Change:** Apply the dry-run decision before tag-based publication. Permit publishing only for explicitly publishing events.
- **Risk:** Ordinary tag releases could stop publishing. Test tag pushes, branch rehearsals, and tag rehearsals as separate cases without invoking GitHub release creation.

### F2. Both packages omit a library the app requires

- **Kind:** bug
- **Where:** `packaging/aur/dettivo-bin/PKGBUILD:17` and `packaging/aur/dettivo/PKGBUILD:20` list `'qt6-base' 'qt6-declarative' 'qt6-svg' 'qt6-wayland'`; `qt/host/app/CMakeLists.txt:68` links `Qt6::Multimedia`.
- **Why:** Qt Multimedia is absent from both dependency lists. The existing release app’s ELF dependencies include `libQt6Multimedia.so.6`. CI’s build environment installs it separately, masking the source-package dependency omission.
- **Change:** Declare Qt Multimedia and ensure a playback backend resolves through package dependencies. Regenerate both `.SRCINFO` files.
- **Risk:** Backend selection affects playback. Build the source package with declared dependencies only, then install the binary package on a clean root and exercise playback.

### F3. The shared CI setup omits mandatory `jq`

- **Kind:** bug
- **Where:** `.github/actions/setup-toolchain/action.yml:26` installs packages ending with `just zstd python`; `scripts/lint-omarchy-plugin.sh:28` exits with `"jq not found"`.
- **Why:** The clean gate runs a mandatory lint whose executable is not provisioned. Settings and documentation checks also require `jq`.
- **Change:** Add `jq` to the shared setup and the prerequisite check, so its absence is caught before compilation.
- **Risk:** Minimal. Rehearse the gate in the pinned container without inherited host tools.

### F4. The install workflow creates a broken model symlink

- **Kind:** bug
- **Where:** `.github/workflows/rig.yml:466` passes `--models .ci-data/dettivo/models`; `scripts/packaging/install-test.sh:37` retains that relative value; `scripts/packaging/install-test-checks.sh:183` runs `ln -sfn "$models" "$tmp/data/dettivo/models"`.
- **Why:** The symlink resolves relative to the nested daemon data directory, not the repository. Engine smoke checks can find the models from the working directory while the installed daemon cannot find them through its model directory.
- **Change:** Resolve the models directory to an absolute path once when parsing arguments.
- **Risk:** Missing directories should produce an explicit failure or skip. Exercise the workflow’s exact relative-path invocation and an absolute-path invocation.

### F5. The plugin discards real dictation completions

- **Kind:** contract
- **Where:** `omarchy/DettivoState.qml:185` handles `"completed"`; line 194 hides the pill on `"idle"`. `crates/dettivo-session/src/machine.rs:353` maps `Exit::Completed` to `State::Idle`. `qt/fixtures/omarchy-shell/tests/tst_plugin_events.qml:83` supplies `"state": "completed"`.
- **Why:** The daemon attaches insertion results to the idle transition. The plugin therefore loses completion feedback and its history refresh. A probe using the actual completion shape produced `pillState: "hidden"`; the shim test passes because it invents the expected event.
- **Change:** Consume idle events carrying completion data. Replace the invented event with a daemon-generated fixture and preserve failed-state feedback across the following idle transition.
- **Risk:** Cancellation and ordinary idle events must still hide the pill. Cover insertion, clipboard fallback, failure, cancellation, and history refresh.

### F6. “Stop meeting” always sends invalid parameters

- **Kind:** bug
- **Where:** `omarchy/DettivoState.qml:112` sends `["call", "meetings.stop", "{}"]`; `crates/dettivo-proto/src/methods/meetings.rs:73` requires `pub meeting_id: Id`.
- **Why:** The daemon cannot deserialize the empty object. The plugin retains no active meeting ID and launches commands detached, so the rejection never reaches the panel. First-use meeting start can likewise fail silently at the disclosure gate.
- **Change:** Retain the active meeting ID and pass it to stop. Observe action results and route first-use disclosure through the app.
- **Risk:** Avoid stopping a stale meeting after reconnect. Test start and stop through a real daemon, including missing disclosure and an already-finished meeting.

### F7. The release gate accepts empty or stale install evidence

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/pack/gate.rs:182` selects only rows whose status equals `"fail"`; line 192 accepts an empty failure list with `"passed": true`.
- **Why:** `{"passed":true,"steps":[]}` passes. Skipped checks, missing required checks, and reports from another version also pass. The exact-SHA check in remote installation does not protect this local report consumer.
- **Change:** Validate a typed report with required check IDs, allowed outcomes, version, revision, and package checksum. Derive success from those fields.
- **Risk:** Historical receipts will need regeneration. Add rejection cases for empty, skipped, duplicate, stale, and mismatched reports.

### F8. The benchmark gate accepts unreadable evidence

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/pack/release_checks.rs:190` substitutes `Value::Null` for unreadable reports; line 216 discards the ancestry result; lines 229–230 pass when no filename is missing.
- **Why:** Matching CPU and GPU filenames can satisfy the check even when their contents are invalid. Missing revision data is subsequently described as measurement on a squashed branch, which the code has not established.
- **Change:** Reject malformed or incomplete reports. Require host, tier, measurements, and explicit provenance; represent a squashed source revision with recorded evidence rather than an inferred explanation.
- **Risk:** Existing branch-derived reports may need a provenance update. Check corrupt JSON, empty reports, wrong tiers, and valid squashed-branch evidence.

### F9. Fine-tune conversion destroys the previous model before succeeding

- **Kind:** bug
- **Where:** `scripts/models/convert-polish-finetune.sh:218` removes `"$old" "$old.sha256"`; conversion starts at line 225, and optional verification follows manifest publication at line 254.
- **Why:** Reusing an experiment ID deletes its working model before conversion, quantization, or verification succeeds. A failed replacement leaves the existing manifest pointing at missing or incomplete content.
- **Change:** Build and verify in a staging directory, then publish the completed model and manifest. Preserve the previous experiment until replacement succeeds.
- **Risk:** Publication order matters when the daemon loads concurrently. Inject failures at conversion, quantization, and verification and confirm the previous model remains usable.

### F10. The session install check reports failed setup as success

- **Kind:** test
- **Where:** `scripts/packaging/install-test-session.sh:33` captures failure with `|| code=$?`; line 34 unconditionally records `session_setup_check pass 0`.
- **Why:** Broken compositor setup receives a green receipt. Line 30 also selects Omarchy because the CLI advertises the command, which says nothing about the installed desktop.
- **Change:** Judge the exit status and setup result. Detect the actual desktop or accept an explicit test target.
- **Risk:** Plain Hyprland installations must remain supported. Test failed setup, missing snippet inclusion, Omarchy, and plain Hyprland.

### F11. Reconnecting the plugin does not restore active sessions

- **Kind:** bug
- **Where:** `omarchy/DettivoState.qml:280` only handles `!status.is_active`; line 258 resets dictation to `"idle"` after stream exit.
- **Why:** An active status response never restores recording or transcribing. There is no corresponding meeting snapshot request. Reloading the shell during a session can leave the bar showing ready until another transition arrives; audio-level events cannot repair that state.
- **Change:** Restore dictation and meeting state from snapshots after subscription/reconnection, including the meeting ID. Define ordering so a late snapshot cannot overwrite a newer event.
- **Risk:** Snapshot/event races. Reload during recording and transcription, and interrupt the event stream during a meeting.

### F12. The plugin claims pill ownership without proving it can render

- **Kind:** bug
- **Where:** `omarchy/Panel.qml:14` requires `moduleAvailable` for hosting; line 29 starts the ownership process using only `binaryAvailable` and panel mode.
- **Why:** Ownership and rendering have different prerequisites. The plugin can suppress the standalone pill while its own module or loader is unavailable. Conversely, the window can render without confirmation that the ownership claim succeeded.
- **Change:** Tie ownership to successful renderer readiness and observe the claim result. Release ownership when rendering fails.
- **Risk:** Handover could briefly show two pills or none. Test missing modules, loader failure, competing ownership, and mode changes.

### F13. Releases never update the documented plugin mirror

- **Kind:** contract
- **Where:** `docs/omarchy.md:66` says the export script copies the folder and “the release commits it”; `.github/workflows/release.yml:73` delegates to the rig, whose jobs contain no mirror export or publication.
- **Why:** The export helper has no release caller, and the release runbook contains no mirror publication step. Package users and mirror users therefore have no implemented guarantee of receiving the same plugin revision.
- **Change:** Add an explicit mirror publication stage tied to the released folder and version, or remove the mirror distribution promise until that path exists.
- **Risk:** This affects a separate repository. Rehearse into a disposable checkout and compare the exported tree byte for byte before enabling publication.

### F14. Documented QA commands do not match the recipes

- **Kind:** contract
- **Where:** `docs/RELEASING.md:12` uses `just qa-release engines=<that directory>/debug`; `docs/guides/omarchy.md:37` uses `just qa-pack gui --surface omarchy`.
- **Why:** The first passes the literal `engines=` prefix as part of the directory. A dry-run emitted `--engines engines=target/vulkan/debug`. The second failed with `justfile does not contain recipe '--surface'`.
- **Change:** Document positional engine arguments and the existing `gui-omarchy` pack. Correct the same examples in recipe comments and other guides.
- **Risk:** Minimal. Dry-run documented recipe invocations and check the resulting CLI arguments.

### F15. Delete the manually maintained Makefile mirror

- **Kind:** delete
- **Where:** `Makefile:2` says “keep these targets in step”; line 206 hardcodes `build-package.sh bin dist`, while the package target accepts `DIST`.
- **Why:** Drift already exists. `make --dry-run package-bin DIST=build/review-dist` stages into the requested directory and packages from `dist`. Several QA targets also omit the build prerequisites present in their Just counterparts.
- **Change:** Remove the duplicate recipe implementation and require Just. If compatibility is necessary, retain only a forwarding wrapper.
- **Risk:** Existing Make users must migrate. Update the documented entry points and verify the canonical recipes cover them.

### F16. The crate-edge lint ignores native workspace dependencies

- **Kind:** contract
- **Where:** `tools/xtask/src/edges.rs:262` filters dependencies with `n.starts_with("dettivo")`; lines 79–80 nevertheless list `parakeet-cpp-sys` and `sherpa-onnx-sys`.
- **Why:** A client can acquire either native binding without this lint seeing the edge. The intended engine-process boundary is therefore unenforced precisely where linking native inference code matters.
- **Change:** Identify internal dependencies through workspace membership, then declare the permitted native-binding edges.
- **Risk:** Existing legitimate edges need explicit entries. Test metadata extraction with a client-to-native-binding dependency, not only manually constructed dependency lists.

### F17. The installed-file check can miss the entire plugin

- **Kind:** test
- **Where:** `packaging/manifest.txt:28` lists `usr/share/dettivo/omarchy/`; `scripts/packaging/check-manifest.sh:62` removes subtree entries from comparison, and line 99 enters file-list comparison without checking subtree presence.
- **Why:** Root-tree mode checks that the directory exists and contains a file. Installed-list mode discards all plugin paths and never requires any back, so a package missing the whole plugin can pass. Neither mode verifies its required entry points.
- **Change:** Enumerate the small plugin file set, removing the subtree exception, or enforce identical required-entry checks in both modes.
- **Risk:** Plugin file additions must update the manifest. Test an absent plugin, missing entry point, and unexpected packaged file.

### F18. Delete the prose blacklist from the build gate

- **Kind:** delete
- **Where:** `scripts/check-docs.sh:178` selects the first nonblank line after H1; line 186 rejects `note*|warning*|caveat*|"before you"*|"this document"*`.
- **Why:** ADRs are judged on their `Status:` line, not their explanation. Other pages can fail merely because a useful opening begins with “Notes”. This checks vocabulary while the invalid commands in F14 remain accepted.
- **Change:** Remove this heuristic and its planted wording test. Keep structural links, contract pins, indexed records, and the generated CLI-tree comparison; judge prose during review.
- **Risk:** Stylistic problems stop blocking builds. Preserve the substantive documentation checks.

### F19. Accepted ADRs leave contradictory instructions without clear supersession

- **Kind:** record
- **Where:** `docs/adr/0034-install-layout-cuda-drop-in-and-release-workflow.md:21` says packaging runs “on every push”; `docs/adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md:18` assigns it to nightly regression. `docs/adr/README.md:3` requires the old record to receive “a pointer forward”.
- **Why:** ADR 0034 remains simply Accepted without a supersession pointer. Similar historical promises survive in ADR 0004’s meeting-capable Parakeet introduction despite ADR 0018’s contrary outcome. Readers must reconstruct chronology to know which instructions govern.
- **Change:** Mark affected portions superseded and link directly to the governing records. Keep historical evidence, but remove present-tense ambiguity from their introductions and index entries.
- **Risk:** Historical decisions must remain recoverable. Check each changed status against the current implementation and preserve the original rationale.

## Keep

- The single install-tree assembler and generated completions in `scripts/package.sh`.
- Package-byte checksums and exact installed revision checks in `.github/workflows/rig.yml` and `.github/workflows/release.yml`.
- The explicit aggregate CI result in `.github/workflows/ci.yml`.
- Toolchain-sensitive compiled-cache keys in `.github/actions/setup-toolchain/action.yml`; no unmeasured speed claim is needed.
- Shared visual components behind the thin plugin wrappers in `omarchy/DettivoPill.qml` and `omarchy/DettivoPanelContent.qml`.
- Named command failures in `scripts/run-step.sh` and private desktop-session cleanup in `scripts/qa/xvfb-session.sh`.
- Contract-copy integrity checks in `scripts/check-docs.sh`. The current crate/version table in `NOTICE.md` also matches `Cargo.lock`.
- The empty handwritten-code exemption list in `.file-length-allow`.

## Questions for the owner

- Is the plugin mirror still a required distribution channel, or can the packaged folder be the only supported path?
- Should publication accept the documented “external blockers”, including missing real-microphone and CPU-only-VM evidence, or should those require an explicit release exception?
- Is supporting machines without Just worth retaining any Make compatibility?
- What event ends fast-build mode? The record still describes review and QA as deferred until cleanup, while this review exposes release-blocking defects.
# Cleanup: the release gate and its evidence (16 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

The release gate passes only on complete evidence tied to the tested binaries, and an unexplained failure stays a failure. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0051 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### daemon

- **daemon/F11** (test): Fixture normalization replaces evidence with the expected answer
  - Where: `crates/dettivod/tests/contract.rs:274`: `*items = fixture["result"]["items"].as_array().unwrap().clone();`; `crates/dettivo-qa/src/replay.rs:246`: `Value::Array(_) => Value::String("list".into())`; line 250: `_ => Value::String("leaf".into())`.
  - Why: Search normalization substitutes expected rows for actual rows. An empty search result passes the preceding loop and becomes the expected result. QA normalization discards array contents and scalar types, so missing rows and wrong value types can compare equal.
  - Change: Deserialize actual responses into protocol types, assert seeded behavior, and normalize only specific nondeterministic fields. Never copy expected result collections into actual results.
  - Risk: Machine-dependent results need narrow tolerances. Add negative checks proving that empty search results, malformed rows and wrong scalar types fail.

### engines

- **engines/F15** (bug): The Parakeet build modifies a caller’s source checkout
  - Where: `crates/parakeet-cpp-sys/build.rs:41` uses `PARAKEET_CPP_SOURCE_DIR` directly, then line 44 calls `apply_cpu_patch(&source)`. Lines 163–168 apply the patch inside that tree and write a marker.
  - Why: An override described as pointing at local source unexpectedly mutates it. An already patched checkout without Dettivo’s marker can also fail, and concurrent builds can race on the same patch operation.
  - Change: Treat supplied source trees as immutable. Copy them into a build-owned staging directory before patching, or require an explicitly prepared source tree and only validate it.
  - Risk: Staging adds build time and disk usage. Test a read-only override directory and two builds using the same source tree.

### qa-rig

- **qa-rig/F8** (test): Accessibility failures can become 100% naming coverage
  - Where: `crates/dettivo-qa/src/driver/atspi.rs:125`: `component.get_extents(CoordType::Screen).await.ok()`; `crates/dettivo-qa/src/a11y_tree.rs:100`: `!is_interactive(&e.role) || e.bounds.is_none()`; line 116 returns `1.0` when no interactive elements remain.
  - Why: Unknown geometry is treated as grounds to exclude a control. Failed child reads and traversal limits also silently omit nodes. An incomplete tree can therefore receive perfect coverage; bounds absence does not prove that a control is hidden.
  - Change: Carry snapshot completeness and visibility state explicitly. Fail or mark coverage indeterminate when required traversal or geometry fails, and assert expected controls for the surface.
  - Risk: Hidden Qt delegates need deliberate handling. Test an unnamed visible control with failed extents, a missing subtree, and a genuinely empty surface.

### qa-packs

- **qa-packs/F1** (contract): Publication does not enforce the documented release gate
  - Where: `.github/workflows/release.yml:82` — `needs: [prepare, rig]`; `.github/workflows/release.yml:104` — `gh release create "v${version}"`; `docs/RELEASING.md:3` — “the release does not ship while a step has failed or a blocker is unexplained”.
  - Why: The workflow runs the reusable rig, but neither workflow runs `pack release` or validates its recorded verdict. A tag can therefore publish without the documented checks over benchmark evidence, human receipts and unexplained blockers. The rig supplies substantial independent testing, but it does not enforce this release policy.
  - Change: Require a validated release receipt before publication, bound to the tested revision and package. Keep orchestration outside the receipt validator so it does not recursively wait for its own CI run.
  - Risk: Existing release rehearsals may become blocked. Check that missing, failed and mismatched receipts prevent publication while the matching successful receipt permits it.

- **qa-packs/F2** (bug): The install gate accepts empty, stale or incomplete receipts
  - Where: `crates/dettivo-qa/src/pack/gate.rs:175` — `pub fn judge(text: &str)`; `crates/dettivo-qa/src/pack/gate.rs:182` — `.filter(|s| s["status"] == "fail")`; `crates/dettivo-qa/src/pack/gate.rs:192` — `failed.is_empty() && report["passed"] == Value::Bool(true)`.
  - Why: `{"passed":true,"steps":[]}` passes this predicate. Missing required checks, unknown statuses and nonzero exit codes also pass unless a row explicitly says `"fail"`. Neither version nor package identity is checked. A missing report can additionally become an allowed external blocker because its suggested remediation contains `--session`.
  - Change: Deserialize a defined receipt type and require the expected checks, successful statuses and package identity. A missing local install report must remain missing proof; classify an unavailable VM separately.
  - Risk: Older receipts will be rejected. Add cases for empty steps, omitted checks, unknown statuses, nonzero exits and a receipt from another package.

- **qa-packs/F3** (contract): The release wrapper converts unexplained failures into external exceptions
  - Where: `crates/dettivo-qa/src/pack/release_steps.rs:181` — `let atspi_ok = r["driver"] == "cua"`; `crates/dettivo-qa/src/pack/release_steps.rs:190` — `"failed on cua-driver (passed on atspi): {reason}"`; `crates/dettivo-qa/src/pack/release_checks.rs:109` — `external: true`.
  - Why: An atspi pass does not establish that a cua failure belongs to the driver. Different timing can expose a product race. Nevertheless, every such failure is automatically excused. Similarly, every nonzero `gh` result becomes external, and an empty run list asserts that workflows were disabled without checking that explanation. The checked-in release report actually marks the meetings step passed after exit 1.
  - Change: Delete inference from driver names and error-message substrings. Preserve failures; represent any accepted exception explicitly, with its scope and supporting diagnosis. Distinguish unavailable CI evidence from confirmed disabled workflows.
  - Risk: Previously green releases will expose existing failures. Verify product failures on either driver remain failures and only explicitly accepted exceptions change release eligibility.

- **qa-packs/F4** (contract): Reports identify the checkout rather than the binaries tested
  - Where: `crates/dettivo-qa/src/scenarios/mod.rs:173` — `.max_by_key(|p| modified(p))`; `crates/dettivo-qa/src/bench/host.rs:137` — `pub fn git_sha(repo_root: &Path)`; `crates/dettivo-qa/src/pack/report.rs:53` — `pub struct BinaryInfo`.
  - Why: Binary selection uses modification time between debug and release builds. Provenance records paths and profiles, while the report SHA comes from Git HEAD. A stale executable can therefore be attributed to a newer checkout; rebuilding debug can also silently change what a release-oriented drive tests.
  - Change: Resolve an explicit binary set once per run and record its embedded revision or hashes. Release runs should require the intended build profile and reject mismatched binaries. Delete modification-time selection from that path.
  - Risk: Convenient mixed local builds will need explicit selection. Check a stale release binary, a newer debug binary and an engine from another revision.

- **qa-packs/F5** (bug): The benchmark release check validates filenames, not measurements
  - Where: `crates/dettivo-qa/src/pack/release_checks.rs:190` — `.unwrap_or(Value::Null)`; `crates/dettivo-qa/src/pack/release_checks.rs:229` — `if missing.is_empty()`; `docs/reports/benchmarks/2026-09-06-thor-gpu.json:4` — `"date": "2026-09-05"`.
  - Why: Two matching filenames suffice to pass, even if their JSON is invalid or their measurements failed. History membership is calculated but never enforced. Recording meetings carries an older benchmark forward under a new filename; the current GPU file contains the previous day's benchmark and a newer meetings block whose `pack_passed` is false. Finding the VM's own report also cannot clear the desktop CPU exception because lookup is restricted to the current hostname.
  - Change: Validate complete measurement records and select evidence by explicit hardware role and tested build. Delete the carry-forward copy; reference separate immutable benchmark and meeting runs. Remove the obsolete diarization placeholder now that its measurement exists.
  - Risk: Historical evidence may no longer qualify as current release evidence. Test corrupt files, failed or absent measurements, mixed revisions and reports from separate GPU and CPU hosts.

- **qa-packs/F7** (contract): Evidence-map coverage is not evidence of requirement completion
  - Where: `crates/dettivo-qa/src/evidence_map/resolve.rs:187` — `text.contains(&needle_paren)`; `crates/dettivo-qa/src/evidence_map/resolve.rs:81` — `reference.split('/').next()`; `crates/dettivo-qa/src/evidence_map/resolve.rs:120` — `"human" => self.existing(reference, &[""])`.
  - Why: A unit reference can resolve to an ordinary function or commented-out declaration. A visual reference ignores any state suffix. A human route proves only that a file exists. For example, `qa/evidence-map.toml:4670` references the existing baseline log for a human checkpoint, while `.flow/specs/fn-38-beauty-pass-every-surface-against-the.md:56` requires an actual owner walkthrough and closed findings. The map can pass while that requirement remains unfulfilled.
  - Change: Keep the map as an inventory of verification routes and name that result accordingly. Resolve exact test and visual identities from their actual registries, removing duplicate name lists. Release completion must consume successful results and explicit human receipts.
  - Risk: Some current references will stop resolving. Plant an ordinary function, a nonexistent visual state and an existing file without an approval receipt; none should establish completed proof.

- **qa-packs/F16** (test): Missing accessibility captures are reported as clean coverage
  - Where: `crates/dettivo-qa/src/pack/scan.rs:105` — `.filter(|p| route_of(p).is_some())`; `crates/dettivo-qa/src/pack/surfaces.rs:133` — `a11y_coverage: if interactive == 0 { 1.0 }`.
  - Why: The scanner only discovers files already written. A missing route capture produces no finding, and zero inspected controls becomes coverage 1.0. The filename convention also excludes the runner's ordinary `tree.json`. Removing a capture can therefore improve the apparent result.
  - Change: Require expected captures for GUI steps and distinguish not inspected from an inspected screen with no interactive controls. Scan the default tree where applicable.
  - Risk: Headless and legitimately skipped steps must remain exempt. Delete an expected capture, supply an empty tree and truncate a capture; report each distinctly.

- **qa-packs/F17** (bug): The startup benchmark includes profile preparation in daemon startup time
  - Where: `crates/dettivo-qa/src/bench/footprint.rs:257` — `let at = Instant::now()`; `crates/dettivo-qa/src/bench/daemon.rs:55` — `Profile::create(...)`; `crates/dettivo-qa/src/bench/daemon.rs:57` — `profile.link_model(...)`.
  - Why: The clock starts before directory creation, fixture preparation, model linking and configuration writing. Those are harness operations, and cross-filesystem model copying can dominate them. The resulting figure is labelled socket-ready latency and compared with the product's startup target.
  - Change: Prepare the profile first, then measure process spawn to successful ping. Measure systemd socket activation separately if that is the promised user operation.
  - Risk: Existing calibration numbers will no longer be directly comparable. Vary profile-preparation cost and verify the daemon-start figure remains unaffected.

### qml

- **qml/F15** (test): The shared style checker examines only one of a control’s delegates
  - Where: `qt/host/style_check.cpp:38` iterates `{"background", "contentItem"}`, but line 41 immediately executes `return url`.
  - Why: A valid style background ends the search. A caller can replace the content item and still pass, despite ADR 0021 promising to reject either delegate created outside the style. The stock-control canary does not exercise this mixed case.
  - Change: Check both delegates independently. Add negative cases that replace only the background and only the content item.
  - Risk: Distinguish legitimate application content from a control’s drawing delegates. Report the offending delegate explicitly so fixes do not become broad exemptions.

### ops-and-record

- **ops-and-record/F1** (bug): A release rehearsal against a tag ignores `dry_run`
  - Where: `.github/workflows/release.yml:50` checks `GITHUB_REF_TYPE = tag`; line 56 writes `"publish=true"` before line 58 considers `inputs.dry_run`.
  - Why: Dispatching against a matching tag with `dry_run=true` enables publication. Executing the prepare block with those inputs reproduced `publish=true`.
  - Change: Apply the dry-run decision before tag-based publication. Permit publishing only for explicitly publishing events.
  - Risk: Ordinary tag releases could stop publishing. Test tag pushes, branch rehearsals, and tag rehearsals as separate cases without invoking GitHub release creation.

- **ops-and-record/F7** (test): The release gate accepts empty or stale install evidence
  - Where: `crates/dettivo-qa/src/pack/gate.rs:182` selects only rows whose status equals `"fail"`; line 192 accepts an empty failure list with `"passed": true`.
  - Why: `{"passed":true,"steps":[]}` passes. Skipped checks, missing required checks, and reports from another version also pass. The exact-SHA check in remote installation does not protect this local report consumer.
  - Change: Validate a typed report with required check IDs, allowed outcomes, version, revision, and package checksum. Derive success from those fields.
  - Risk: Historical receipts will need regeneration. Add rejection cases for empty, skipped, duplicate, stale, and mismatched reports.

- **ops-and-record/F8** (test): The benchmark gate accepts unreadable evidence
  - Where: `crates/dettivo-qa/src/pack/release_checks.rs:190` substitutes `Value::Null` for unreadable reports; line 216 discards the ancestry result; lines 229–230 pass when no filename is missing.
  - Why: Matching CPU and GPU filenames can satisfy the check even when their contents are invalid. Missing revision data is subsequently described as measurement on a squashed branch, which the code has not established.
  - Change: Reject malformed or incomplete reports. Require host, tier, measurements, and explicit provenance; represent a squashed source revision with recorded evidence rather than an inferred explanation.
  - Risk: Existing branch-derived reports may need a provenance update. Check corrupt JSON, empty reports, wrong tiers, and valid squashed-branch evidence.

- **ops-and-record/F13** (contract): Releases never update the documented plugin mirror
  - Where: `docs/omarchy.md:66` says the export script copies the folder and “the release commits it”; `.github/workflows/release.yml:73` delegates to the rig, whose jobs contain no mirror export or publication.
  - Why: The export helper has no release caller, and the release runbook contains no mirror publication step. Package users and mirror users therefore have no implemented guarantee of receiving the same plugin revision.
  - Change: Add an explicit mirror publication stage tied to the released folder and version, or remove the mirror distribution promise until that path exists.
  - Risk: This affects a separate repository. Rehearse into a disposable checkout and compare the exported tree byte for byte before enabling publication.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** daemon/F11, Fixture normalization replaces evidence with the expected answer: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** engines/F15, The Parakeet build modifies a caller’s source checkout: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** qa-rig/F8, Accessibility failures can become 100% naming coverage: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** qa-packs/F1, Publication does not enforce the documented release gate: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** qa-packs/F2, The install gate accepts empty, stale or incomplete receipts: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** qa-packs/F3, The release wrapper converts unexplained failures into external exceptions: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** qa-packs/F4, Reports identify the checkout rather than the binaries tested: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** qa-packs/F5, The benchmark release check validates filenames, not measurements: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** qa-packs/F7, Evidence-map coverage is not evidence of requirement completion: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R10:** qa-packs/F16, Missing accessibility captures are reported as clean coverage: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R11:** qa-packs/F17, The startup benchmark includes profile preparation in daemon startup time: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R12:** qml/F15, The shared style checker examines only one of a control’s delegates: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R13:** ops-and-record/F1, A release rehearsal against a tag ignores `dry_run`: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R14:** ops-and-record/F7, The release gate accepts empty or stale install evidence: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R15:** ops-and-record/F8, The benchmark gate accepts unreadable evidence: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R16:** ops-and-record/F13, Releases never update the documented plugin mirror: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R17:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0051 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.

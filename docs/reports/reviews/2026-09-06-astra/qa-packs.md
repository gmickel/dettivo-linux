# The QA packs, the visual gate, the evidence map and the release gate

## Verdict

The scenario runner and pack composition provide a useful foundation, but the release verdict is not reliable enough to authorize shipping. The most valuable change is to make publication consume validated evidence tied to the tested binaries, preserving failures and distinguishing missing proof from an approved exception. Read-only verification found the evidence map passing for 251 requirements and confirmed the report inconsistencies below; no files changed, and `just build test lint` and desktop drives were not run under the read-only constraint.

## Findings

### F1. Publication does not enforce the documented release gate

- **Kind:** contract
- **Where:** `.github/workflows/release.yml:82` — `needs: [prepare, rig]`; `.github/workflows/release.yml:104` — `gh release create "v${version}"`; `docs/RELEASING.md:3` — “the release does not ship while a step has failed or a blocker is unexplained”.
- **Why:** The workflow runs the reusable rig, but neither workflow runs `pack release` or validates its recorded verdict. A tag can therefore publish without the documented checks over benchmark evidence, human receipts and unexplained blockers. The rig supplies substantial independent testing, but it does not enforce this release policy.
- **Change:** Require a validated release receipt before publication, bound to the tested revision and package. Keep orchestration outside the receipt validator so it does not recursively wait for its own CI run.
- **Risk:** Existing release rehearsals may become blocked. Check that missing, failed and mismatched receipts prevent publication while the matching successful receipt permits it.

### F2. The install gate accepts empty, stale or incomplete receipts

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/pack/gate.rs:175` — `pub fn judge(text: &str)`; `crates/dettivo-qa/src/pack/gate.rs:182` — `.filter(|s| s["status"] == "fail")`; `crates/dettivo-qa/src/pack/gate.rs:192` — `failed.is_empty() && report["passed"] == Value::Bool(true)`.
- **Why:** `{"passed":true,"steps":[]}` passes this predicate. Missing required checks, unknown statuses and nonzero exit codes also pass unless a row explicitly says `"fail"`. Neither version nor package identity is checked. A missing report can additionally become an allowed external blocker because its suggested remediation contains `--session`.
- **Change:** Deserialize a defined receipt type and require the expected checks, successful statuses and package identity. A missing local install report must remain missing proof; classify an unavailable VM separately.
- **Risk:** Older receipts will be rejected. Add cases for empty steps, omitted checks, unknown statuses, nonzero exits and a receipt from another package.

### F3. The release wrapper converts unexplained failures into external exceptions

- **Kind:** contract
- **Where:** `crates/dettivo-qa/src/pack/release_steps.rs:181` — `let atspi_ok = r["driver"] == "cua"`; `crates/dettivo-qa/src/pack/release_steps.rs:190` — `"failed on cua-driver (passed on atspi): {reason}"`; `crates/dettivo-qa/src/pack/release_checks.rs:109` — `external: true`.
- **Why:** An atspi pass does not establish that a cua failure belongs to the driver. Different timing can expose a product race. Nevertheless, every such failure is automatically excused. Similarly, every nonzero `gh` result becomes external, and an empty run list asserts that workflows were disabled without checking that explanation. The checked-in release report actually marks the meetings step passed after exit 1.
- **Change:** Delete inference from driver names and error-message substrings. Preserve failures; represent any accepted exception explicitly, with its scope and supporting diagnosis. Distinguish unavailable CI evidence from confirmed disabled workflows.
- **Risk:** Previously green releases will expose existing failures. Verify product failures on either driver remain failures and only explicitly accepted exceptions change release eligibility.

### F4. Reports identify the checkout rather than the binaries tested

- **Kind:** contract
- **Where:** `crates/dettivo-qa/src/scenarios/mod.rs:173` — `.max_by_key(|p| modified(p))`; `crates/dettivo-qa/src/bench/host.rs:137` — `pub fn git_sha(repo_root: &Path)`; `crates/dettivo-qa/src/pack/report.rs:53` — `pub struct BinaryInfo`.
- **Why:** Binary selection uses modification time between debug and release builds. Provenance records paths and profiles, while the report SHA comes from Git HEAD. A stale executable can therefore be attributed to a newer checkout; rebuilding debug can also silently change what a release-oriented drive tests.
- **Change:** Resolve an explicit binary set once per run and record its embedded revision or hashes. Release runs should require the intended build profile and reject mismatched binaries. Delete modification-time selection from that path.
- **Risk:** Convenient mixed local builds will need explicit selection. Check a stale release binary, a newer debug binary and an engine from another revision.

### F5. The benchmark release check validates filenames, not measurements

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/pack/release_checks.rs:190` — `.unwrap_or(Value::Null)`; `crates/dettivo-qa/src/pack/release_checks.rs:229` — `if missing.is_empty()`; `docs/reports/benchmarks/2026-09-06-thor-gpu.json:4` — `"date": "2026-09-05"`.
- **Why:** Two matching filenames suffice to pass, even if their JSON is invalid or their measurements failed. History membership is calculated but never enforced. Recording meetings carries an older benchmark forward under a new filename; the current GPU file contains the previous day's benchmark and a newer meetings block whose `pack_passed` is false. Finding the VM's own report also cannot clear the desktop CPU exception because lookup is restricted to the current hostname.
- **Change:** Validate complete measurement records and select evidence by explicit hardware role and tested build. Delete the carry-forward copy; reference separate immutable benchmark and meeting runs. Remove the obsolete diarization placeholder now that its measurement exists.
- **Risk:** Historical evidence may no longer qualify as current release evidence. Test corrupt files, failed or absent measurements, mixed revisions and reports from separate GPU and CPU hosts.

### F6. The visual comparator deliberately ignores regressions the gate promises to catch

- **Kind:** simplify
- **Where:** `qt/tools/visual-diff/main.cpp:37` — `constexpr int kRows = 18`; `qt/tools/visual-diff/main.cpp:87` — `Qt::IgnoreAspectRatio`; `docs/adr/0021-visual-regression-gate-and-frame-pacing.md:26` — “the same change passed the 74 entries held to approved renders”.
- **Why:** The comparator reduces images to coarse grids, normalizes contrast and searches positional shifts. Those tolerances help compare an implementation with a differently rendered artboard, but also forgive changes against an approved render. The ADR already records a 12-to-16 font-size regression passing all 74 approved-render entries. The large combined canary does not establish sensitivity to smaller colour, typography or spacing regressions.
- **Change:** Remove artboard-comparison tolerances from comparisons against approved renders. Use a dimension-preserving, colour-sensitive comparison there; retain artboard scores as advisory evidence. Calibrate with separate, small regression plants.
- **Risk:** Font and renderer variation can introduce noise. Pin the rendering environment and measure unchanged-run variance before choosing tolerances.

### F7. Evidence-map coverage is not evidence of requirement completion

- **Kind:** contract
- **Where:** `crates/dettivo-qa/src/evidence_map/resolve.rs:187` — `text.contains(&needle_paren)`; `crates/dettivo-qa/src/evidence_map/resolve.rs:81` — `reference.split('/').next()`; `crates/dettivo-qa/src/evidence_map/resolve.rs:120` — `"human" => self.existing(reference, &[""])`.
- **Why:** A unit reference can resolve to an ordinary function or commented-out declaration. A visual reference ignores any state suffix. A human route proves only that a file exists. For example, `qa/evidence-map.toml:4670` references the existing baseline log for a human checkpoint, while `.flow/specs/fn-38-beauty-pass-every-surface-against-the.md:56` requires an actual owner walkthrough and closed findings. The map can pass while that requirement remains unfulfilled.
- **Change:** Keep the map as an inventory of verification routes and name that result accordingly. Resolve exact test and visual identities from their actual registries, removing duplicate name lists. Release completion must consume successful results and explicit human receipts.
- **Risk:** Some current references will stop resolving. Plant an ordinary function, a nonexistent visual state and an existing file without an approval receipt; none should establish completed proof.

### F8. The release contract step duplicates the CLI and omits REST verification

- **Kind:** delete
- **Where:** `crates/dettivo-qa/src/pack/gate.rs:121` — `replay::run(&opts.repo_root, &daemon.socket)`; `crates/dettivo-qa/src/daemon_verbs.rs:211` — `report.rest = rest`.
- **Why:** The ordinary contract command adds MCP and REST harness results. The release step starts another similarly configured daemon but calls only socket replay. MCP has a separate release step; REST does not. The checked-in strict-contract report consequently contains empty MCP and REST arrays despite its daemon enabling REST.
- **Change:** Delete the second contract orchestration path. Share one library operation between the CLI and release pack, with explicit required transports and validated result sections. Avoid rerunning MCP twice.
- **Risk:** Shared-daemon fixture ordering matters because fixtures mutate state. Verify the complete transport suite against a fresh profile and confirm a broken REST response fails the release step.

### F9. Contract normalization erases types and entire array element contracts

- **Kind:** contract
- **Where:** `crates/dettivo-qa/src/replay.rs:246` — `Value::Array(_) => Value::String("list".into())`; `crates/dettivo-qa/src/replay.rs:250` — `_ => Value::String("leaf".into())`.
- **Why:** A number, boolean, string and null become identical. Arrays with malformed elements compare equal to correctly structured arrays. This affects engine, device, selection and history results. Normalizing machine-specific values should not allow the transport to violate field types or row structure.
- **Change:** Validate typed response shapes before normalizing values. Preserve array element structure and declared nullability; tolerate only the particular fields whose values vary.
- **Risk:** Legitimate optional fields need accurate schemas. Check wrong scalar types, malformed array entries and permitted empty arrays or null values.

### F10. The visual matrix has permanent gaps and a second Home implementation

- **Kind:** delete
- **Where:** `qa/visual/manifest.toml:206` — `# Home joins here with the settings spec`; `qa/visual/manifest.toml:279` — `# The five sections that follow the pattern, General standing for them:`; `scripts/qa/app-visual-diff.sh:61` — `env QT_QPA_PLATFORM=offscreen QT_SCALE_FACTOR=1`.
- **Why:** Home still uses a separate shell implementation with two themes and one scale. First run omits Tokyo Night. General stands in for four other settings sections, so their content-specific layout changes escape the matrix. The “every surface” claim exceeds the implemented coverage.
- **Change:** Move Home into the manifest and delete its separate rendering, crop-loop and comparison orchestration. Enumerate the actual settings sections and restore the promised theme coverage.
- **Risk:** Additional entries require reviewed baselines. Verify the inventory against real routes and plant a regression in one previously represented settings section.

### F11. First-insert benchmarks discard failures and accept one successful warm sample

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/bench/first_insert.rs:152` — `Err(e) => failures.push(e)`; `crates/dettivo-qa/src/bench/first_insert.rs:164` — `.filter(|r| !r.cold && r.outcome == "inserted")`; `crates/dettivo-qa/src/bench/first_insert.rs:186` — `if row.warm.is_none()`.
- **Why:** Nine failed warm dictations and one successful dictation produce a measured row with p50 and p95 from the survivor. The failures remain in the JSON but do not fail the step. This biases latency toward successful runs and contradicts the intended ten-sample measurement.
- **Change:** Require the configured number of successful warm runs. Preserve every failure and report incomplete sampling as failure, with partial statistics clearly separated from a qualifying measurement.
- **Risk:** Flaky runs will stop producing apparently valid headline figures. Test one-of-ten, nine-of-ten and ten-of-ten completion.

### F12. The GPU proof establishes presence or aggregate activity, not workload execution

- **Kind:** simplify
- **Where:** `crates/dettivo-qa/src/pack/gpu_proof.rs:263` — `.find(|(_, n)| *n > 0)`; `crates/dettivo-qa/src/pack/gpu_proof.rs:274` — `let busy = with_engine`; `crates/dettivo-qa/src/pack/gpu_proof.rs:279` — `if busy >= AMD_BUSY_FLOOR`.
- **Why:** NVIDIA passes after one process-table appearance, which establishes a GPU allocation but not where inference ran. AMD passes when any observed device reaches 10% activity while an engine process exists; the compositor or another application can supply that activity. The report's workload-proof claim is stronger than either observation.
- **Change:** Delete the aggregate busy threshold as proof. Record these counters as diagnostics unless work can be attributed to the measured engine and device. Return unavailable proof when attribution is unsupported.
- **Risk:** Some machines will lose a green proof row. Check an unrelated GPU load alongside CPU inference and an engine that initializes a GPU context but performs no inference there.

### F13. An import error leaves the GPU sampler running

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/pack/meeting_throughput.rs:314` — `let sampler = gpu_proof::Sampler::start(...)`; `crates/dettivo-qa/src/pack/meeting_throughput.rs:324` — `)?`; `crates/dettivo-qa/src/pack/gpu_proof.rs:196` — `pub fn stop(mut self) -> Samples`.
- **Why:** Errors from the import call or missing response identifiers return before `stop()`. `Sampler` has no drop cleanup. Dropping its join handle detaches the thread, which retains its stop flag and continues polling and accumulating samples during later pack steps.
- **Change:** Give the sampler unconditional cleanup that stops and joins its thread, including early returns. Bound external counter calls so cleanup cannot wait forever.
- **Risk:** Cleanup must avoid double joins and preserve useful partial evidence. Inject an import failure immediately after sampler creation and verify polling stops.

### F14. Catalogue-model evaluation loses its checksum during cleanup

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/polish_eval/mod.rs:160` — `model_file: runner.model_file.clone()`; `crates/dettivo-qa/src/polish_eval/mod.rs:305` — `model_sha256: model_file`; `crates/dettivo-qa/src/profile.rs:222` — `models.remove()`.
- **Why:** `run_pipeline` returns the model path and drops its runner and profile. The caller then hashes that deleted path. Both checked-in Qwen3-4B evaluation reports contain temporary model paths and omit `model_sha256`; the external sideload reports retain checksums because their files survive cleanup.
- **Change:** Hash the loaded model while the profile exists and return the checksum with the pipeline result. Treat an unavailable checksum for a real candidate as an evidence failure.
- **Risk:** Large model hashing adds read time. Verify catalogue, direct-path and sideload candidates retain correct hashes after cleanup.

### F15. Promotion compares unrelated datasets and can pass without the requested comparison

- **Kind:** contract
- **Where:** `crates/dettivo-qa/src/polish_eval/mod.rs:96` — `Ok(Incumbent { model: report.model, backend: report.backend, metrics: report.metrics.as_map() })`; `crates/dettivo-qa/src/polish_eval/gate.rs:175` — `if inc.backend != candidate_backend`; `crates/dettivo-qa/src/polish_eval/gate.rs:231` — `passed: failures.is_empty()`.
- **Why:** Loading the incumbent discards its dataset, split and row identities. Two different evaluation sets can therefore produce a relative promotion verdict. A backend mismatch skips the relative gates without preventing `passed`, even when the caller explicitly supplied an incumbent.
- **Change:** Require compatible dataset content, selected rows, scoring rules and execution conditions for relative comparisons. Report absolute acceptance separately from promotion; an unavailable requested comparison cannot establish promotion.
- **Risk:** Existing reports lack some compatibility metadata. Check different sets with identical filenames, different splits and backend mismatches.

### F16. Missing accessibility captures are reported as clean coverage

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/pack/scan.rs:105` — `.filter(|p| route_of(p).is_some())`; `crates/dettivo-qa/src/pack/surfaces.rs:133` — `a11y_coverage: if interactive == 0 { 1.0 }`.
- **Why:** The scanner only discovers files already written. A missing route capture produces no finding, and zero inspected controls becomes coverage 1.0. The filename convention also excludes the runner's ordinary `tree.json`. Removing a capture can therefore improve the apparent result.
- **Change:** Require expected captures for GUI steps and distinguish not inspected from an inspected screen with no interactive controls. Scan the default tree where applicable.
- **Risk:** Headless and legitimately skipped steps must remain exempt. Delete an expected capture, supply an empty tree and truncate a capture; report each distinctly.

### F17. The startup benchmark includes profile preparation in daemon startup time

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/bench/footprint.rs:257` — `let at = Instant::now()`; `crates/dettivo-qa/src/bench/daemon.rs:55` — `Profile::create(...)`; `crates/dettivo-qa/src/bench/daemon.rs:57` — `profile.link_model(...)`.
- **Why:** The clock starts before directory creation, fixture preparation, model linking and configuration writing. Those are harness operations, and cross-filesystem model copying can dominate them. The resulting figure is labelled socket-ready latency and compared with the product's startup target.
- **Change:** Prepare the profile first, then measure process spawn to successful ping. Measure systemd socket activation separately if that is the promised user operation.
- **Risk:** Existing calibration numbers will no longer be directly comparable. Vary profile-preparation cost and verify the daemon-start figure remains unaffected.

### F18. A failed approval command can already have replaced baselines

- **Kind:** bug
- **Where:** `crates/dettivo-qa/src/visual/approve.rs:51` — `"{}/{}: {e}; nothing approved"`; `crates/dettivo-qa/src/visual/approve.rs:69` — `std::fs::copy(&png, &path)`; `crates/dettivo-qa/src/visual/approve.rs:70` — `record(repo, &entry, &path, artboard_score, &who, &today)?`.
- **Why:** Each entry is copied and recorded before the next entry is rendered. If a later render fails, the command says “nothing approved” despite earlier replacements. A log-write failure can also leave a replaced baseline without its provenance row.
- **Change:** Render and validate the complete selection before replacing baselines. Publish the selected files and provenance together, or explicitly report any partial completion and changed entries.
- **Risk:** Staging costs temporary storage. Fail the second render and the provenance write, then verify the baseline set and log remain consistent.

## Keep

- Ordered steps, explicit expectations and `not_run` outcomes in `crates/dettivo-qa/src/pack/mod.rs`. This is sufficient composition machinery; it does not need another workflow framework.
- Canary rejection of render errors and missing baselines in `crates/dettivo-qa/src/visual/report.rs`. A crashed renderer does not count as detecting the planted regression.
- Measuring meeting imports through the daemon in `crates/dettivo-qa/src/pack/meeting_throughput.rs`. It exercises the product path and preserves useful transcript and progress evidence.
- Raw samples and centralized target definitions in `crates/dettivo-qa/src/bench/report.rs` and `crates/dettivo-qa/src/nfr.rs`. Fix qualification and calibration without discarding the samples.
- Aggregate evaluation reports without private row text in `crates/dettivo-qa/src/polish_eval/report.rs`. Keep unsuccessful historical evaluations.
- The reviewed Rust files meet the stated size limit, including `crates/dettivo-qa/src/main.rs`. Further splitting is not a priority.

## Questions for the owner

- Is cua a supported acceptance path? If so, its unexplained failures must block. If not, should it leave the release gate entirely?
- Which missing proofs may a release ship with: real microphone, live Hyprland, CPU VM or unavailable CI? These need explicit exception decisions.
- Which representative hardware and engine/model combinations define the CPU and GPU performance promises? Calibration on one high-end desktop does not establish those broader tiers.
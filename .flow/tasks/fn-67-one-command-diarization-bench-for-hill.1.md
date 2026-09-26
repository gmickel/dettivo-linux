---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8]
---
# fn-67-one-command-diarization-bench-for-hill.1 Implement One-command diarization bench for hill-climbing speaker accuracy

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
`just diar-bench` now scores the tree's own speaker rule (`dettivo_meeting::diarize::assign`, run through `crates/dettivo-meeting/examples/diar_assign.rs`) against cached engine outputs. It covers AMI dev (18 meetings), held-out AMI test (`--heldout`) and ten retained meetings (4 English, 6 German), and it finishes in 0.4-2.4 s once the cache is warm. `just diar-bench-setup` prepares the mode-700 eval directory at `~/.local/share/dettivo-eval/bench`, and fn-64's cache seeded it.

- **R1:** A cached run takes 0.4-2.4 s for both engines on every split. The first run after a clean build also compiles the assigner.
- **R2:** A temporary `min_coverage` edit in `diarize.rs` reran only assignment and scoring (engine 39 cached), and the numbers moved. The edit was reverted. Every run ends with a stage ledger showing what was cached, computed, stale or missing.
- **R3:** `scripts/qa/diarbench/metrics.py` holds one `METRICS` table, and docs/diarization-bench.md documents it. On AMI test the current engine reproduces fn-64 exactly: engine DER 15.67% with 3.39% confusion, and on Whisper lines 1.97% confusion, 25.60% missed and 6.75% false alarm, with lines 75.6% right, 5.5% wrong and 18.9% unlabelled. On fn-64's eight meetings it reproduces 22.7% of remote lines unlabelled and 0.88% mix confusion.
- **R4:** `--save` and `--baseline` give paired bootstrap 95% deltas over files.
- **R5:** AMI test is scored only with `--heldout`, and the docs say to tune on dev.
- **R6:** Setup installs the venv, fetches AMI audio, references and word timings against pinned hashes, fetches the Nemotron model against the fn-64 receipt, and writes the selection from finished meetings, skipping any meeting that is recording or finalising.
- **R7:** Caches, scoreboards and baselines stay in the eval directory. `scripts/qa/test_diarbench.py` asserts that the report writer emits aggregates only, and the test is red-checked.
- **R8:** docs/qa.md gains a "Speaker accuracy" section, and docs/diarization-bench.md covers the loop, metrics, splits, variants and the labels format fn-72 writes.

The committed report is docs/reports/benchmarks/diarization-bench-2026-09-26.{md,json}.

**Fixed during the run:** the assigner used to run from cargo's output path under a key hashed at start. A rebuild during a long `--full` run could therefore store another rule's labels under the current key. The bench now runs a copy named by its hash, and the poisoned assign and score caches were rebuilt. A fresh uncached recompute matches the cache (0 differences).

**Follow-ups:**
- I dropped a line-level local/remote proxy because I could not verify it. It put about 39% of `You`-line time on remote-only frames, which is worth investigating in fn-71.
- The AMI mirror throttles bursts with HTTP 403. Setup backs off, skips the file with a message, and fetches it on the next run.
- Nemotron trained on AMI dev, so its dev figures flatter it. The docs say this.

stage: impl-review - skipped(config: REVIEW_MODE=none)
Tier: implementer claude-opus-5-5 (project routing block)
## Evidence
- Commits: f4abbf7f894d493ea2b40a15d847248dfa10ea7c, 20125d4b0fbc5dd9b55e4e88f4188cc4c99b5bb4, acc2f12438d716dd9be628b85d1a96302b83ef7d
- Tests: baseline: green (just build test lint, pre-edit), just build test lint (green: 161 suites ok, 0 failed; TMPDIR on /home because the per-user /tmp tmpfs quota was exhausted by other sessions; three earlier runs each failed one unrelated daemon timing test under load average 20-36 and those tests passed alone), python3 scripts/qa/test_diarbench.py, python3 scripts/qa/test_diarization_score.py, just diar-bench --heldout (cached, 0.4-2.4 s), just diar-bench --full (AMI dev 18 meetings + 2 new DE meetings computed), R2 check: temporary min_coverage edit in diarize.rs reran assign only, engine 39 cached, numbers moved; reverted, labels smoke: stored product labels as a labels file gave 0.23% headline for the current engine; file removed
- PRs:
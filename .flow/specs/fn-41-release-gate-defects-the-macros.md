# Release gate defects: the macros capability flag, the seeded row count and the QA profile's models directory

## Conversation Evidence

> release gate run of record, `docs/reports/release-gate/0.1.0.json` (fn-37, 2026-09-06): verdict FAILED; two steps fail on defects rather than external blockers: `contract_strict` ("the daemon declares `dictation_macros = true` but answers the six `automation.macros.*` methods as pending") and `pack_gui` ("`history_seeded`/`history_states` expect 13 seeded rows on atspi and cua, the list shows 15 since the seed gained meetings").
> fn-37 worker, 2026-09-06 06:50 UTC: "The shared test model `~/.local/share/dettivo/models/whisper/tiny.en` was removed at 06:50 by something outside this worktree between two gate runs; I restored it with `scripts/models/fetch-test-model.sh`."
> incident 2026-09-04: the contract replay profile symlinked the real `$XDG_DATA_HOME/dettivo/models` directory into the QA profile, so the `speech/models.delete` fixtures deleted Gordon's real `whisper/large-v3-turbo` model (fixed on fn-19 for the replay profile only: the test model is hard-linked in). `crates/dettivo-qa/src/profile.rs:84` still symlinks the real models directory into every drive profile.
> masterplan 9.2: "Every scenario at L2 and above runs in an isolated profile ... First-run profiles are used deliberately because they expose contract gaps a configured machine hides"

## Goal & Context

<!-- Goal & Context: 60% [paraphrase], 40% [inferred] -->

The release gate passes on what the code actually does, and no QA run can ever touch a real model again. Three small defects stand between the 0.1.0 candidate and a gate whose only blockers are external: the daemon advertises a capability it does not implement, the history drives count a seed that has since grown, and the drive profile still reaches into the user's real models directory through a symlink, which is how a test deleted `tiny.en` during the fn-37 gate run and `large-v3-turbo` on 2026-09-04.

## Architecture & Data Models

<!-- Architecture & Data Models: 50% [paraphrase], 50% [inferred] -->

- Capability flag: `crates/dettivod/src/platform.rs` sets `dictation_macros: true` while the six `automation.macros.*` methods answer `NOT_IMPLEMENTED` (pending). No macros spec exists in v1, so the honest value is `false`; the `system/capabilities.json` fixture, the contract replay's pending list, the delta register and any doc that names the flag follow. `dettivo-qa contract --strict` then passes because a pending method behind a `false` flag is an admitted gap, not a broken promise. [inferred]
- Seeded rows: `crates/dettivo-qa/src/scenarios/history_seeded.rs` (`SEEDED_ROWS = 13`) and `history_states.rs` count the seed of fn-22's day; the fn-33 and fn-34 seed adds meetings rows to the history list (15 today). The scenarios derive the expected count from the seed itself (the daemon's `transcripts.list` over the seeded profile, or one constant the seed module exports) so the next seed change cannot break them, and the doc rows in `docs/qa.md` follow. [inferred]
- The QA profile's models directory: `crates/dettivo-qa/src/profile.rs` links `models_dir` into the profile with a symlink to the real directory. Every drive and pack profile gets the treatment fn-19 gave the replay profile: a private `models/` directory under the profile with only the test models hard-linked in (`tiny.en`, and the diarization set and the LLM test model where a scenario needs them), never a link to the user's directory; a scenario that needs another model names it and the profile preflight refuses a real-directory link by path. The fn-35 preflight (`profile::tests`, refusing a root under `HOME` or `XDG_DATA_HOME`) extends to the models link. [inferred]

## API Contracts

<!-- API Contracts: 80% [paraphrase], 20% [inferred] -->

- `system.capabilities.dictation_macros` becomes `false` on Linux until a macros spec lands; recorded in `docs/api/linux-deltas.md` as the deviation it is. [paraphrase]
- No other contract change. [inferred]

## Acceptance Criteria

- **R1:** `system.capabilities` reports `dictation_macros = false`, the fixture and the delta register agree, and `cargo run -q -p dettivo-qa -- contract --strict` passes on the branch with the six macros methods listed as pending behind a false flag. Errors: a method that answers pending behind a `true` flag fails `--strict` naming the flag and the method. [inferred]
- **R2:** `history_seeded` and `history_states` pass on the in-repo driver under Xvfb against the current seed, the expected row count comes from the seed rather than a literal, and `docs/qa.md`'s rows say so. Errors: a seed change that the scenario cannot see fails naming the count it read and the count it expected. [inferred]
- **R3:** No QA profile links a real user directory: `profile.rs` builds a private `models/` with only the named test models hard-linked in, the preflight refuses a real-directory link by path, a unit test proves both, and a full `dettivo-qa pack gui` and `pack dictation` run under Xvfb leave `~/.local/share/dettivo/models` byte-identical (checked by a listing before and after). Errors: a scenario that names a model the profile does not carry fails by name before the daemon starts. [inferred]
- **R4:** `dettivo-qa pack release` re-run on this desktop records a report whose only blockers are external, checked in as the new run of record under `docs/reports/release-gate/`; the ADR that records the gate (0041) gains the outcome. Errors: as stated. [inferred]

## Boundaries

- No macros implementation; the flag tells the truth and a later spec may flip it back.
- No change to the seed itself.
- No cua-driver fixes for the meetings screens (they stay external blockers owned by fn-34 and the driver), no CPU-only VM run, no real-microphone receipt.

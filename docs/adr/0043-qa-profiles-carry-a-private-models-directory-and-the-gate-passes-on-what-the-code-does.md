# 0043. QA profiles carry a private models directory, the capability flags tell the truth, and the history drives read the seed

Status: Accepted 2026-09-06, amended 2026-09-06 by 0049 (a profile hard-links the weights alone and resolves a new tree through its ancestors)

Amended by [0050](0050-private-transfers-authenticated-adapters-and-isolated-qa-processes.md), which clears inherited QA environments and provides private cache storage.

## What this gives you

No QA run can touch a real model again: every drive, pack, bench and replay profile gets a `models/` tree of its own with only the models it names hard-linked in, and a link that would reach the user's directory is refused by path before a daemon starts. The release gate passes on what the daemon does rather than on what it advertises: `automation.dictation_macros` and `automation.dictation_macro_audit` are `false` until a macros spec lands, the strict contract replay accepts a pending method only behind a flag declared `false`, and the history drives take their row count from the seed they run against, so the next seed change cannot fail them.

## Situation

The first executed gate for 0.1.0 (`docs/reports/release-gate/0.1.0.json`, ADR 0041) failed two steps on defects rather than on external blockers: `contract_strict` because the daemon declared `dictation_macros = true` while the six `automation.macros.*` methods answered `NOT_IMPLEMENTED`, and `pack_gui` because `history_seeded` and `history_states` expected the thirteen rows of fn-22's seed where fn-33 and fn-34 had added meetings and the list showed fifteen. Beside those, `crates/dettivo-qa/src/profile.rs` symlinked the real `$XDG_DATA_HOME/dettivo/models` into every drive profile; the replay profile had already lost `whisper/large-v3-turbo` to the `speech.models.delete` fixtures on 2026-09-04 and been fixed on its own (fn-19, a hard-linked test model), and during the fn-37 gate run a drive removed `whisper/tiny.en` from the real directory the same way. The alternatives on the table were implementing macros to make the flag true (no spec, out of v1), a literal `15` in the scenarios (the next seed breaks it again), and a copy of the models into each profile's `/tmp` root (a tmpfs with a quota; the meetings pack times `large-v3-turbo` at 1.6 GB).

## Decision

- **The private models tree.** `profile_models::ModelTree` builds one directory per profile under `target/qa-models/` (the models' filesystem, so every link is a hard link; a copy only where the filesystems differ) and points the profile's `data/dettivo/models` at it. `TEST_MODELS` (`whisper/tiny.en`, `fixtures`) are linked when the real directory has them; anything else is linked by name through `Profile::link_model`, refused with the fetch command when the machine lacks it. A `Scenario` names what it needs beyond the test set through `models()` and the runner links them before the preconditions, skipping the scenario by name when one is missing; the bench, the meetings throughput step, the polish evaluation and the contract daemons name theirs at the call. The tree goes with the profile. `profile_models::guard` refuses a tree that is the real directory or under it, by path and through symlinks, before anything is created; the fn-35 root guard stays as it was.
- **The flags tell the truth.** `system.capabilities.automation.dictation_macros` and `dictation_macro_audit` are `false` on Linux; the fixture snapshot and the delta register (six rows `deferred`) say the same. The contract replay still sends a fixture whose required `true` flag the daemon declares `false`, and a `NOT_IMPLEMENTED` answer there is a pending row carrying `admitted_by` (the flag); `--strict` accepts those and fails a pending row behind a `true` flag or no flag, naming the method and the flag that promised it. The proto fixture test accepts a required `true` flag the snapshot holds as `false` for the same reason.
- **The seed is the count.** `history_seeded::seeded_rows` asks the seeded daemon for `transcripts.list` with the kinds the History list requests (`dictation`, `meeting`) and both history drives wait for that many rows; a mismatch names the count shown and the count the seed answered.

## Consequences

- A drive profile costs one hard-linked directory under `target/qa-models/` instead of a symlink; the tree is removed with the profile, and a kept profile (`--keep-profile`) keeps it. The real directory is read, never written: the fn-41 gate run was bracketed by a listing of `~/.local/share/dettivo/models` (size, mtime, inode, link count, checksum) before and after, byte-identical.
- A scenario that needs a model the machine lacks skips before its daemon starts, naming the model and the fetch command, rather than failing inside the drive.
- The six `automation.macros.*` fixtures are pending rows in every contract report until a macros spec flips the flag back with the implementation; a spec that implements a method without declaring its flag, or declares a flag without implementing the method, fails `--strict` by name.
- The history drives follow the seed; a seed past one page of the list (twenty rows) would need the scroll and is a change to make when it happens.
- ADR 0011's line on profiles with the model directory linked in is superseded by the private tree; ADR 0041 records the outcome of the gate re-run.

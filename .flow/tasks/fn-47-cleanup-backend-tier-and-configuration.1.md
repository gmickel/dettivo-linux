---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18, R19]
---
# fn-47-cleanup-backend-tier-and-configuration.1 Cleanup: backend, tier and configuration truth, every finding fixed or rejected with evidence

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-47 cleanup receipt

| R-ID | Outcome | Evidence |
|---|---|---|
| R1 | fixed | `9fd8b11a17cc627dbd2d90436e2b25767a3bd8d3`; invalid reloads retain the last valid authentication and configuration values, with live regression coverage. |
| R2 | fixed | `9fd8b11a17cc627dbd2d90436e2b25767a3bd8d3`; configuration derivation, writes, installation and watcher reloads are serialized, with concurrent edit coverage. |
| R3 | fixed | `5155a28c00dd395e1f4c0e103093db549f842160`; configuration validation rejects pipeline-invalid cross-field and range values by key. |
| R4 | fixed | `7f05279f42b2c585fe722724312baee5f71d8b5d`; inline MCP server tables are preserved and unreadable files are never overwritten. |
| R5 | fixed | `e5671fef9533f2981cace399db38cf44f30e2c4e`; verification records identity for every model file and detects changed catalogue entries and files. |
| R6 | fixed | `b70653da99eb1b4ab8059510cf2a86996c04e0aa`, `410393ec6f11232071e5407918ab54b0462f8017`; catalogue loads wait for verification and status probes do not perform unsafe hashing. |
| R7 | fixed | `3f163f619dec2668a2120761e2d9f6be5fbd180d`; strict Vulkan is Vulkan-or-refusal and backend reporting follows the initialized device. |
| R8 | fixed | `747f0707cdbbb5350e077e8b2574d3577575b19c`; CPU-only diarization is excluded from GPU tier fallback. |
| R9 | fixed | `b615ab72203203abc8c5a462ee2ac2ae7ee04a76`; Vulkan tests select Vulkan and cancellation terminates an engine that ignores cancellation. |
| R10 | fixed | `cc3a688580a93cb44904a17624b9f9dc551a217f`; language providers do not follow redirects outside the trusted endpoint. |
| R11 | fixed | `cc3a688580a93cb44904a17624b9f9dc551a217f`; effective policy model is bound to the provider request and covered by tests. |
| R12 | fixed | `ee9164028a9980101d8bd09414adb93bf262fa4c`; QA profile trees share weights only and copy mutable manifests/sidecars. |
| R13 | fixed | `ee9164028a9980101d8bd09414adb93bf262fa4c`; new profile trees resolve through their deepest existing ancestors before creation. |
| R14 | fixed | `6a2de5e410ba753f04678e4de129286867fd8bc4`; meeting picker sends request field names and separates daemon selection from offered fallback. |
| R15 | fixed | `6a2de5e410ba753f04678e4de129286867fd8bc4`, `30fe8da5ba74100b7637cac05c27dfe3149dfd1e`; Home Start uses configured dictation mode and the Linux request contract supplies configured defaults. |
| R16 | fixed | `6a2de5e410ba753f04678e4de129286867fd8bc4`; first run uses catalogue status/download methods, recomputes capabilities and retains failures. |
| R17 | fixed | `6a2de5e410ba753f04678e4de129286867fd8bc4`; meeting rail follows configuration defaults and sends untouched options as daemon defaults. |
| R18 | fixed | `7c2853b2abde18160968186102da2daf512c9175`; install-test resolves `--models` to an absolute path and tests relative invocation. |
| R19 | fixed | `40d320efba441518a19a99c6b1e1ae3007b3a686`, `40cafd9abdd534418b7fd0ddb9b1b9af5b15108e4`; ADR 0049 is indexed, files are within limits, and the final gate is green. |

### ADR

ADR 0049 records the backend, tier, model verification, configuration, QA isolation, client, Qt, QML and packaging decisions and amends the earlier records it supersedes.

### Verification

- `bash /home/gordon/work/dettivo-linux-wt/_factory/scripts/gate.sh /home/gordon/work/dettivo-linux-wt/fn-47 fn47`
- Final receipt: `GATE_fn47_EXIT=0 40cafd9a`
- The gate passed the workspace build, Rust and Qt tests, rustfmt, clippy, dependency/file-length/delta/notice checks, packaging, QML, token, icon, accessibility, Omarchy, settings, docs and rustdoc checks.
- Final model-tree checks remained isolated; no real user models directory was mounted.

### Deliberately left out

No finding was rejected. Wave-3 cleanup themes, fn-38's human walkthrough, and external release-gate blockers remain outside fn-47.
## Evidence
- Commits: 7a07afde828a186b4fa8621df2127c51ef09a6eb, 5bf77d6912d4800d7f0f457f6db8892afb0442f1, 5769d6b7f03a23090fd6dc577333671296859933, 21abcfc31adb7671248aafb92f1a96dbf20fc6d7, 750f2f37b8f67f5a132eaf8a937b22cbc209498d, e333580bc6be3040c7d3c085b9c2adb796f09db1, 1cb23df41def8e9510dc46138aa454b2b52f8b0e, 69ddf163d7b351fcde0039b994943f8f0a9690be, e5335ff4a22b5736e7845dce66ddb3213a1ef4d5, e454e95552405d89b20ee2f9cf6d5daf19e73c1a, 63ca6eed82c6b1208cfda6b65ce6c6b68ef46e31, 5dd63222bae3ce1e05d5f125cd0b5c609a572558, 52b18a14a9ef39d99800eae0352f2bb97d9e1713, e4eb5107d503a63799a94aded4cfeb4fc7a0c59e, fd3895bee03ee674e77c2e1b91f492a1a231b603, 329b0fe283359c73e8870c5eb0c729355ca9400e, 99b20af69feca2b353b043f0346b5ae37f19e1ef, 8e0086c730b25cbc2baf0294648c8439a4be008d
- Tests: bash /home/gordon/work/dettivo-linux-wt/_factory/scripts/gate.sh /home/gordon/work/dettivo-linux-wt/fn-47 fn47
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)

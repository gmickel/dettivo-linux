---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18, R19, R20, R21, R22, R23, R24]
---
# fn-44-cleanup-ownership-races-and-lifecycle.1 Cleanup: ownership, races and lifecycle, every finding fixed or rejected with evidence

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-44 cleanup receipt

| Finding | Outcome | Evidence |
|---|---|---|
| R1 daemon/F2 | fixed | `de192693` acquires the history store before recovery; lifecycle regression test |
| R2 daemon/F5 | fixed | `84c50028` adds atomic analysis admission and identity-checked publication; analysis race tests |
| R3 daemon/F6 | fixed | `a53e4130` adds the daemon capture reservation; session exclusion tests |
| R4 daemon/F9 | fixed | `2fdc7c0e` resolves authentication mode and token per request; `auth_rotation` tests |
| R5 daemon/F10 | fixed | `c7393fee` applies one bounded shutdown deadline and isolates blocking handlers |
| R6 daemon/F13 | fixed | `7cbf69eb` includes backend in selected speech-engine identity |
| R7 daemon/F14 | fixed | `a939f975` reloads log level and documents restart-required settings; log reload tests |
| R8 daemon/F18 | fixed | `e657dbea` reaps connection tasks and disarms import guards |
| R9 agent-surfaces/F4 | fixed | `63e0d3ce` rejects malformed MCP arguments; protocol/dispatch tests |
| R10 agent-surfaces/F7 | fixed | `ecf250f7` reaps REST tasks and caps active connections |
| R11 agent-surfaces/F8 | fixed | `ecf250f7` carries one REST deadline through read, dispatch, and write; timeout tests |
| R12 engines/F1 | fixed | `a81b4e0a` invokes download progress callbacks after releasing the lock; download tests |
| R13 engines/F3 | fixed | `489b5b0d` carries engine load deadlines and terminates uncertain processes; supervisor tests |
| R14 engines/F6 | fixed | `489b5b0d` caches complete load settings and `7cbf69eb` rebuilds backend changes |
| R15 dictation/F1 | fixed | `58b3283d` coordinates cancellation with rewrite, insertion, and archive; cancel-polish tests |
| R16 dictation/F8 | fixed | `bb4ede1d` keeps cancel hotkeys responsive while push-to-talk is held |
| R17 dictation/F15 | fixed | `0d354f77` starts one rewrite deadline before provider/credential work |
| R18 meetings/F13 | fixed | `de192693` settles orphaned analysis and diarization at startup; lifecycle tests |
| R19 qa-rig/F17 | fixed | `b0999414` bounds CUA and socket operations with absolute deadlines |
| R20 qa-packs/F13 | fixed | `25695beb` makes GPU sampler drop cleanup stop and join the thread |
| R21 qt-hosts/F9 | fixed | `88cb783d` claims app ownership before GUI initialization; host tests |
| R22 qt-hosts/F12 | fixed | `c6c97375` deadlines daemon-client requests and handles write failure; client tests |
| R23 ops-and-record/F11 | fixed | `b86d7c59` and `9d6458ec` restore snapshots with ordered sequence markers |
| R24 final gate | fixed | `8209c2f4` records and indexes ADR 0046; final gate and contract checks below |

### ADR

ADR 0046 records the one-owner, one-deadline decisions for this theme and indexes them in
`docs/adr/README.md`. It covers findings R1-R23 and amends the earlier ownership, timeout,
configuration, and lifecycle records named in the ADR.

### Verification

- `flock /tmp/dtv-gate.lock make build test lint`: exit 0, `GATE_fn-44_EXIT=0 88bf3012`.
- `cargo run -q -p dettivo-qa -- contract`: exit 0, 85 passed, 0 failed, 44 skipped, 6 pending; REST 55 passed.
- `cargo run -q -p dettivo-qa -- contract --strict`: exit 0, same passing contract result.
- The models directory was listed before and after; both snapshots were 6,594,988,554 bytes with the same top-level entries. The gate and contract replay do not download or delete models; a post-run recursive hash was also captured.

### Deliberately left out

No finding was rejected. Visual drives were not needed by these non-visual fixes; the relevant
unit, integration, Qt, and QML checks ran as part of the gate.
## Evidence
- Commits: e83edb5420964d243494427388726d883f0f0566, ad1056196f190167b1866ff2f19a4888c42bdf6d, 155600208aaf6387a7104c74919c976bc51e04fe, e54b9a6783061d1948df09318024560fb163b1bb, 0e7ee44df0047596e2d6910931d321462a9da812, 2c6281ffbc58ad6e0d6e895021d227272786fd94, 35524fa44835376f5643aaed62043ea398814b49, 61774d7c5b141000f70988780bd68d1216dd2a6f, 8b80dc0686edda50778e30045df7b7a211ac8406, e523e1783cf6b1e959752f3f388fda728f0bf962, bacfa2215bab54e16e2379ced786181a767ed9e0, c8a9216cadfd3f5df82b0c3d56ff79f76401ee64, 57d50ad1157bd0e0ba2963682c52810dca2d31e3, df8bd12239d076e9c1903bd0ae9c8fba2cf09335, 34e79ed72f0a0b379540e321a47fea1ce431d319, 9007aec80bebf34cddf1b76a6628c8a19624ac5a, ce314362597171097791b4bb1fb72a95ae994e0c, 65dabe83402bdccccbda38d696b40d32e2a70439, 1a8eb38e08b34158f5b79ff3ad752092fe50f376, 5078c7c24bc22eb5263c7278022b07788d015735, 916b05c002060c144a170bcc9cadc06d329b28a8, 8207509c2a12c5a5f42371e31a6bbe9e9213ee2e, 75c0158ab3423ea3defb1481b9184c918b00e4b3, cab5f95bf5bb0b128888c9ed44581c678e764d25, a9cdda036817b30c3ae5b7f4120208c5d3d48f22
- Tests: flock /tmp/dtv-gate.lock make build test lint, cargo run -q -p dettivo-qa -- contract, cargo run -q -p dettivo-qa -- contract --strict
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)

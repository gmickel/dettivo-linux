# Releasing

A release of Dettivo for Linux is a build that passed the gate below on the development machine, with every external blocker named in a checked-in report, published from one tag with no hand step. The gate is one script: `dettivo-qa pack release` runs thirteen named steps in order, embeds each step's own report, and ends with `blockers`, `external_blockers` and `unexplained_blockers`; the release does not ship while a step has failed or a blocker is unexplained, and the report it files under [docs/reports/release-gate/](reports/release-gate/0.1.0.md) is the artifact the release notes quote ([ADR 0041](adr/0041-guides-docs-build-evidence-map-and-the-release-gate-script.md), the report shape in [docs/guides/qa.md](guides/qa.md#the-release-gate)).

## Running the gate

```
just build-whisper-vulkan build-parakeet-vulkan build-llm-vulkan   # the engines on Vulkan, into one target directory, <target> below
just build-release                                                  # the release profile of the binaries the drives run
just bench <target>/debug && just bench-cpu <target>/debug   # the benchmark rows, committed
just package-bin                                                    # the package
just install-test                                                   # the prefix install test, build/install-test.json
scripts/qa/xvfb-session.sh just qa-release <target>/debug   # the gate, filed under docs/reports/release-gate/<version>.json
git add docs/reports && git commit
```

The gate drives the release build of this commit and nothing else: it pins `DETTIVO_BUILD_PROFILE=release` for itself and every verb it spawns, records every binary's SHA-256 and the commit `dettivo --version` embeds, and refuses to start (exit 2) when `dettivod` or `dettivo` is a debug build or the CLI's commit is not this checkout's, so `just build-release` at the gated commit is a precondition rather than a hope ([ADR 0051](adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md)). On this desktop the drives run under `scripts/qa/xvfb-session.sh`, off the live screen; the steps that need the live Hyprland desktop (the compositor hotkeys, the Omarchy bar) skip with that reason and the report lists them as external blockers. `--skip <step>` leaves a step out, recorded as such; `--cpu` runs the meetings pack on the CPU tier, which is the command the CPU-only VM runs. GitHub's workflows are disabled by instruction while the local gates land everything, so `ci_green` records that as an external blocker and `just build test lint` on the same commit stands in; the workflows are the automated form of the same steps ([ADR 0040](adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md)).

## The steps

Each step is one row of the report with its outcome, duration, the command a person can repeat, its exit code, its reason and its embedded report.

### 1. `version_match`

`Cargo.toml`'s version has its `## [<version>]` section in `CHANGELOG.md` ([scripts/packaging/release-notes.sh](../scripts/packaging/release-notes.sh) reads it as the notes) and, when the tag `v<version>` exists, it points at this commit. Before the tag the step passes and says the tag is not created yet.

### 2. `ci_green`

`gh run list --commit <sha>` shows every workflow run for the commit concluded `success`; a run that is not green fails the step. No run at all, or a `gh` error, is a blocker: external when the API confirms either GitHub Actions is disabled for the repository or the primary `.github/workflows/ci.yml` workflow is disabled. The workflow query retains its path, ID and state in the report. An active workflow, an unknown state or a failed lookup leaves missing CI evidence unexplained; a known failed run remains a failure even if the workflow was later disabled ([ADR 0051](adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md)).

### 3. `contract_strict`

The CLI and release gate share one complete contract operation covering MCP, REST and socket replay. Every required section must be nonempty and valid. Every fixture under `crates/dettivo-proto/fixtures/` replays against a fresh daemon and a method the daemon has not implemented counts as a failure unless the capability flag the fixture requires is declared `false`, which makes the gap an admitted one the row names ([docs/qa.md](qa.md#contract-replay), [ADR 0008](adr/0008-contract-parity.md)); `contract-replay.json` is embedded and the step's reason lists the admitted methods.

### 4. `mcp_harness`

This step validates the MCP results already embedded by `contract_strict`; it does not start a second daemon or repeat the harness. The shared operation drives both framings against a seeded daemon ([docs/qa.md](qa.md#mcp-harness), [ADR 0054](adr/0054-delete-duplicate-work-and-keep-one-owner.md)).

### 5. `pack_dictation`

`dettivo-qa pack dictation` runs the hotkeys through the compositor, the pill on both drivers, the insertion matrix, the self-insertion refusal, the history round trip, the first-insert timing and the Whisper WER fixture ([docs/qa.md](qa.md#the-dictation-pack), [ADR 0017](adr/0017-qa-packs-and-the-dictation-report.md)). The pack runs with `--continue` so every row is in the report; its blockers join the gate's list, each classified, and a row that fails on either driver fails the step: an `atspi` pass does not establish that a `cua` failure is the driver's, so the gate keeps the failure and a person diagnoses it ([docs/qa.md](qa.md#drivers), [ADR 0051](adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md)). The release notes quote time to first insert (p50 and p95 with the NFR-1 target beside them), insertion reliability against NFR-3 and the Whisper word error rate on the backend that ran. A missed target is quoted, never gated.

### 6. `pack_gui`

`dettivo-qa pack gui` runs onboarding, settings, history and the Omarchy plugin on both drivers against the release profile of the binaries, scans every route for developer text and unnamed controls, and passes with `negative_text.findings` empty and `a11y_coverage` at 1.0 on every surface ([docs/qa.md](qa.md#the-gui-pack), [ADR 0037](adr/0037-gui-packs-per-surface-and-the-tree-checks.md)). The Omarchy desktop steps run for real only on the live desktop under `DETTIVO_QA_OMARCHY_LIVE=1`; under Xvfb they are external blockers by name.

### 7. `pack_meetings`

`dettivo-qa pack meetings` runs the rig capture with the device swap, the kill and recovery, the live transcript, diarization against the turns golden, the export goldens, the three meetings screens on both drivers, the token fixture, then the five-minute import timed through the daemon's own finalisation path, the diarization engine over the same track and the GPU workload proof ([docs/qa.md](qa.md#the-meetings-pack), [ADR 0039](adr/0039-meetings-pack-finalisation-path-throughput-and-the-gpu-proof.md)). `--engines <dir>` names the Vulkan build so the figures are the GPU tier's; `--record` files them into the benchmark report. The release notes quote the meeting realtime factor against NFR-4 or NFR-5, the diarization factor against NFR-4, the token coverage and the harness share.

### 8. `bench_report`

The newest checked-in benchmark report for this host on the `gpu` tier and for each host on the `cpu` tier under [docs/reports/benchmarks/](reports/benchmarks/README.md) is read as the typed report the suite writes and judged on its contents ([docs/qa.md](qa.md#benchmarks), [ADR 0029](adr/0029-nfr-calibration-and-the-benchmark-suite.md), [ADR 0051](adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md)): it parses, it is the tier's, it has steps and none failed or lacks its value, its commit and its `meetings` block's commit are ancestors of this commit, and the block carries the meeting and diarization factors; a file that lacks any of these fails the step naming the fact, and an older file never stands in for a newer one that fails. The step embeds each report's steps with their verdicts. A CPU row measured on this desktop under `DETTIVO_FORCE_CPU=1` is an external blocker naming the CPU-only VM that has not run `just bench-cpu` and `just qa-pack-meetings-cpu`; a clean CPU report from any other host lifts it. The release notes quote the table verbatim; a target missed two releases in a row is recalibrated in a decision record.

### 9. `install_test`

`install-test.json` (`DETTIVO_INSTALL_TEST_REPORT`, else `build/install-test.json` from `just install-test`) is a receipt for this version with every one of the seventeen required checks present once and passed with exit 0: the file list against `packaging/manifest.txt`, the desktop entry and the units, the CLI's version and the three completions, Home rendered from the installed module, every engine's help, the CPU-fallback smoke over the three engines, the natural backend and one mock-microphone dictation through the installed daemon ([docs/install.md](install.md), [ADR 0034](adr/0034-install-layout-cuda-drop-in-and-release-workflow.md)). A skipped check, an unknown status, a receipt for another version or package, and a missing file each fail the step naming what is unproven ([ADR 0051](adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md)). A receipt from a prefix install on this desktop adds the external blocker that `scripts/packaging/install-test.sh --session` on a fresh CPU-only VM has not run for this version; the VM's receipt, read through `DETTIVO_INSTALL_TEST_REPORT`, lifts it.

### 10. `evidence_map`

`dettivo-qa evidence-map` reports coverage 1.0 of every R-ID in every spec with every reference resolved: a `unit` reference is a function carrying `#[test]`, a `visual` reference names a manifest surface and, with a `/state` suffix, one of its states, and a `human` reference is the receipt a person files ([docs/guides/qa.md](guides/qa.md#the-evidence-map)). The map is the inventory of routes, not their results: the steps above are what run them, and a human receipt on file is not proof that the walk happened ([ADR 0051](adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md)). The report is embedded and the checked-in copy is [docs/reports/evidence-map.md](reports/evidence-map.md).

### 11. `docs_build`

`scripts/check-docs.sh` passes: the records indexed and value-first, the links resolving, the contract copies pinned, every page reachable, every schema key documented, the CLI tree current ([ADR 0041](adr/0041-guides-docs-build-evidence-map-and-the-release-gate-script.md)).

### 12. `notice_lint`

`cargo run -p xtask -- lint-notice` passes: every crate in `Cargo.lock` has its row in `NOTICE.md` and every reuse marker has its row ([docs/reports/voxtype-reuse-review.md](reports/voxtype-reuse-review.md)).

### 13. `real_headset`

`dettivo-qa audio-check` proves the PipeWire capture path with the virtual rig, and `docs/reports/release-gate/real-headset.json` records that a person dictated once through the real microphone on this desktop for this version (the version, the date, the device, the words, the backend). Without the receipt the step skips with that external blocker; the automated rig cannot stand in for a real microphone.

## The changelog, the docs and the tag

```
scripts/packaging/release-notes.sh <version>     # the section the release will publish
just docs                                        # the docs build
git tag v<version> && git push origin v<version>
```

`CHANGELOG.md` keeps one `## [<version>]` section per release in the Keep a Changelog shape; the workflow takes it as the release notes and fails before building when the section is missing or the tag differs from `Cargo.toml`'s version. The tag runs [`.github/workflows/release.yml`](../.github/workflows/release.yml): `prepare` decides through [scripts/packaging/release-decide.sh](../scripts/packaging/release-decide.sh) (a tag push publishes; a dispatch publishes only with `dry_run=false` on that tag; every other run rehearses, whatever ref it runs on) and, when it is about to publish, checks the gate's receipt through [scripts/packaging/release-receipt.sh](../scripts/packaging/release-receipt.sh): `docs/reports/release-gate/<version>.json` says the gate passed for this version with no failed step and no unexplained blocker, at a commit this one descends from with nothing but `docs/reports/` changed since, so the published bytes are the gated code plus its reports and nothing else ([ADR 0051](adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md)). The reusable rig then runs `just build test lint` and `just package-bin` once, verifies the package checksums and the exact installed commit in a second clean container, and `publish` ships that same tarball, `SHA256SUMS` and the notes with `gh release create`, downloading the published files to verify their checksums. Dispatching the workflow by hand with `dry_run` (the default) does everything but the receipt check and the publication and leaves the three files as workflow artifacts, which is how a release candidate is rehearsed on a branch or on the tag itself. `scripts/packaging/test-release-scripts.sh` proves both scripts in `just lint`: tag pushes, branch and tag rehearsals, and missing, failed, stale, corrupt and mismatched receipts.

Once the release is published and its download verified, `release.yml` calls [`.github/workflows/distribute.yml`](../.github/workflows/distribute.yml) ([ADR 0064](adr/0064-a-release-publishes-itself-to-the-aur-and-the-plugin-mirror.md)). Its `aur` job runs `packaging/aur/publish.sh <version>`, which pins the sums and pushes `dettivo-bin` and `dettivo` to the AUR, and its `plugin-mirror` job writes the released `omarchy/` folder into `gmickel/omarchy-dettivo` with `scripts/omarchy/export-plugin.sh` and pushes it as `v<version>` ([docs/omarchy.md](omarchy.md#the-plugin-folder-and-the-mirror)). Both jobs need two repository secrets. `AUR_SSH_PRIVATE_KEY` is a private key whose public half is registered on the AUR account that maintains the packages, and `OMARCHY_MIRROR_DEPLOY_KEY` is the private half of a deploy key with write access on the mirror. A missing secret fails its own job and names it. To distribute a release again, or the first release after the secrets are added, dispatch the workflow by hand: `gh workflow run distribute.yml -f version=<version>`.

## Setting up distribution once

Each release keeps the AUR and the plugin mirror current without anyone touching them, once three things exist. The mirror has to be its own repository because `omarchy plugin add` clones a URL and reads `manifest.json` from its root.

1. The mirror repository, with its issues pointing people back here:
   ```bash
   gh repo create gmickel/omarchy-dettivo --public --disable-issues --disable-wiki \
     --homepage https://github.com/gmickel/dettivo-linux \
     --description "Omarchy bar plugin for Dettivo: local dictation and meeting transcription in the bar, with a panel and the recording pill. Mirror of omarchy/ in gmickel/dettivo-linux."
   gh repo edit gmickel/omarchy-dettivo --enable-projects=false \
     $(for t in omarchy omarchy-plugin hyprland quickshell wayland speech-to-text dictation voice-typing transcription whisper offline linux qml; do printf -- '--add-topic %s ' "$t"; done)
   ```
2. A write deploy key on the mirror, with its private half stored here:
   ```bash
   ssh-keygen -t ed25519 -N '' -C dettivo-mirror -f /tmp/dettivo-mirror-key
   gh repo deploy-key add /tmp/dettivo-mirror-key.pub -R gmickel/omarchy-dettivo --allow-write --title dettivo-release
   gh secret set OMARCHY_MIRROR_DEPLOY_KEY -R gmickel/dettivo-linux < /tmp/dettivo-mirror-key
   trash /tmp/dettivo-mirror-key /tmp/dettivo-mirror-key.pub
   ```
3. An SSH key on the AUR account that maintains `dettivo` and `dettivo-bin`, used for nothing else. Paste the public half into AUR, My Account, SSH Public Key, and store the private half here:
   ```bash
   ssh-keygen -t ed25519 -N '' -C dettivo-aur -f ~/.ssh/aur-dettivo
   gh secret set AUR_SSH_PRIVATE_KEY -R gmickel/dettivo-linux < ~/.ssh/aur-dettivo
   ```

To check that every channel carries the latest release:

```bash
gh release view -R gmickel/dettivo-linux --json tagName --jq .tagName       # the release
gh api repos/gmickel/omarchy-dettivo/tags --jq '.[0].name'                  # the mirror
curl -s 'https://aur.archlinux.org/rpc/v5/info?arg[]=dettivo-bin&arg[]=dettivo' | jq -r '.results[] | "\(.Name) \(.Version)"'
```

A channel that lags gets the release again with `gh workflow run distribute.yml -f version=<version>`.

## The report

`docs/reports/release-gate/<version>.json` carries `schema_version`, `version`, `git_sha`, `machine`, `binaries`, `started_unix`, `duration_ms`, `passed`, `skipped`, `steps` (`id`, `outcome`, `duration_ms`, `command`, `exit_code`, `reason`, `evidence`, `report`, `blockers`), `blockers`, `external_blockers` and `unexplained_blockers` (`step`, `reason`, `external`, `needs`); the `.md` beside it is the same for a person. The 0.1.0 candidate's run is [docs/reports/release-gate/0.1.0.md](reports/release-gate/0.1.0.md).

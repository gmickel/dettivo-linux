# 0040. Fast PR gates and one tested release package

Status: Accepted 2026-09-05; amended by [0051](0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md): `prepare` decides through `release-decide.sh` and checks the gate's receipt before publishing; the rig's visual job moved to the desktop by [0066](0066-visual-regression-runs-on-the-desktop-not-in-ci.md)

## What this gives you

A pull request keeps the full build/test/lint gate and the headless smoke. A release builds its package once and publishes the bytes that passed the clean-container install. Obsolete PR runs cancel, while a release runs to completion under a repository-wide concurrency group.

## Situation

The earlier split put extensive regression in the nightly rig, but its compiled cache was immutable for a lockfile and all rig jobs shared one scope. A successful thin build could become the cache every larger build restored. Rolling Arch packages could also change the installed Rust, Qt and native libraries without changing that key. The release workflow built and installed a package, then called a rig that repeated both jobs. Its uncalibrated benchmark was part of that release dependency.

## Decision

- `ci.yml` runs on PRs and pushes to main. `gate` runs `just build test lint`; `smoke` retains the contract, MCP, REST and headless drive checks. The stable `CI` check runs with `always()` and requires both results to equal `success`, including when a prerequisite fails, cancels or skips. Each build has a 60-minute timeout. Concurrency uses the PR number or branch ref.
- `setup-toolchain` stores dependency downloads separately from compiled output. The compiled key includes job and scope, OS and architecture, the installed package inventory, Rust identity, workspace path, build configuration and revision. Its only restore prefix retains the entire compatibility fingerprint. A successful new revision therefore saves a fresh cache without restoring across toolchain/configuration boundaries.
- The same compiled cache carries `build/qt` and `build/qt-release`. CMake configures on every invocation; Ninja checks source freshness. Including the absolute workspace path prevents restoring CMake trees into another checkout location. Checkout timestamps can still force Qt recompilation; cold/warm remote measurements are required before claiming a speedup.
- `rig.yml` keeps extensive nightly model, drive, visual/canary, package/source-package and clean-install regression. `qa-weekly.yml` keeps every scenario on the fallback driver and tests rolling Arch dependency drift even if the source revision is unchanged. Rig and weekly jobs have 120-minute bounds and cancel obsolete standalone runs.
- For release calls, the rig package job also runs `just build test lint` with the test models. `just package-bin` invokes its package prerequisite once. The uploaded package, archive and revision marker have a common checksum receipt. The clean install verifies those bytes and requires the installed CLI to print the exact source SHA. The release publisher verifies the same receipt and source SHA before publishing the same tarball. The source-package job remains a separate source-build acceptance check.
- Release calls skip the advisory CPU benchmark and redundant advisory Vulkan compile jobs. The real release package still builds all three Vulkan engines. Standalone rig runs retain the advisory jobs; the benchmark cannot block a release. Physical GPU, microphone, hotkey, compositor and release-pack acceptance remain the documented local gates.
- Release publishing is serialized with `cancel-in-progress: false`; the reusable release rig also disables cancellation. Manual release rehearsal keeps `dry_run` true by default.
- Request Copilot when a PR is ready, then explicitly after substantial changes. Keep automatic draft and every-push reviews off. The existing requirement for a reviewed head and zero open threads remains intact.

## Consequences

Compiled output gets a new immutable entry per successful revision, bounded by GitHub cache eviction. A package or native-toolchain update deliberately causes a cold compile. The dependency-download cache can still reuse fetched sources. The stable aggregate is available for the repository merge gate; configuring a required check remains a repository setting.

The implementation has local full-gate and workflow-contract evidence. Remote cold/warm runs and the nonpublishing release rehearsal remain unmeasured while the repository workflows are manually disabled. Workflow code does not re-enable them, and this change does not publish a release or satisfy physical acceptance.

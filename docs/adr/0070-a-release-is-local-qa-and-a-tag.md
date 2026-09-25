# 0070. A release is local QA and a tag

Status: Accepted 2026-09-25, supersedes the receipt requirement of 0051 and amends 0040 (what the rig runs) and 0034 (what the tag runs)

## What this gives you

A version ships about 45 minutes after its tag, with nothing running on your machine. You prove a change the way every change is proven: `just build test lint` locally and by using it. The tag's CI run then proves what only a clean machine can show: that the package it just built installs and starts on a fresh Arch.

## Situation

The build ran as an autonomous loop, and the gates stood in for a person. `dettivo-qa pack release` ran thirteen steps on the development machine (every pack, the benchmark, the install test, a real-microphone receipt), and `release.yml` refused to publish without its receipt. The tag's rig added GUI drives (78 minutes), a benchmark (22), three separate Vulkan builds and a source-package build (45) on top of the package and the clean install.

The product is built now, and Gordon uses it every day. Most of those checks repeat what local QA and daily use already show. The benchmark measures CI runner hardware, which says nothing about a user's machine. The separate Vulkan builds repeat what the package build already compiles. The source package matters only once the AUR is published (ADR 0069). The clean-container install is the one check a development machine cannot make, and it caught real faults in September 2026: files missing from the package manifest, and runtime paths absent without a session.

## Decision

- A release is a version bump merged like any change, then a `v*` tag.
- `release.yml` no longer requires a release-gate receipt. `scripts/packaging/release-receipt.sh` and its tests are removed.
- The rig, called with `release: true`, runs `just toolchain test lint`, builds the package once and installs it on a clean container. The GUI drives, the benchmark, the separate Vulkan builds and the source package run only when the rig is dispatched by hand, and the nightly schedule is gone.
- `just qa-release` and every pack stay as optional tools.

## Consequences

A regression that only the GUI drives or the benchmark would catch now reaches a release unless someone runs them, local QA notices, or daily use finds it. Every pull request still runs the gate and two GUI drives in CI, so the checks that ran on every change still do. The deep rig is one `gh workflow run rig.yml` away. When the AUR is switched on, the source-package build returns to the release run.

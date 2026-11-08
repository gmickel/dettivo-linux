# 0105. Build sources are fetched with the job token and cached by checksum

Status: Accepted 2026-11-08

## What this gives you

A release, or any CI build that misses its target cache, gets its pinned C and C++ sources from an Actions cache instead of from GitHub. The downloads that remain go to github.com as an authenticated client. A GitHub incident that refuses anonymous downloads no longer fails the build.

## Situation

The three `-sys` bindings download twelve pinned files at build time, seven archives from github.com and five ONNX Runtime headers from raw.githubusercontent.com, about 66 MB in all, and verify each against its SHA-256. They called curl with no credentials. Each download landed in the binding's `OUT_DIR`, which the target cache kept, so a warm build never downloaded. The 0.5.0 version bump changed the bindings' version, Cargo gave them new build directories, and every job of release PR #59 downloaded the archives afresh. GitHub's incident "Increase in timeouts for some unauthenticated users" answered with 503s, and the PR's CI failed three times on 2026-11-08, each time inside `sherpa-onnx-sys` and `parakeet-cpp-sys`.

The per-binding `*_ARCHIVE_DIR` variables already let a build read archives from a directory. They name files differently per binding and leave out the sherpa-onnx headers, so a CI cache built on them needs three variables and still misses the headers.

## Decision

- The three bindings share one `fetch` helper, copied identically into `crates/sherpa-onnx-sys/build.rs`, `crates/parakeet-cpp-sys/build.rs` and `crates/nemo-speech-cpp-sys/fetch.rs`. It reads `DETTIVO_SOURCE_CACHE/<sha256>` before the network and copies every verified download there. The checksum verification is unchanged, and a cache that cannot be written produces a cargo warning instead of a failed build.
- A download from `https://github.com/` sends `GITHUB_TOKEN`, or `GH_TOKEN`, as `Authorization: Bearer`. curl reads the header from standard input (`-H @-`), so the token appears in no argument list, panic or log. raw.githubusercontent.com gets no token, because it answers an invalid one with 404, while github.com ignores an invalid token on public archives. `scripts/models/fetch-test-model.sh` and `fetch-diarization-model.sh` do the same for their github.com downloads.
- Neither variable is declared with `rerun-if-env-changed`. They change where the bytes come from and nothing that is built, so the job token, which changes on every run, never triggers a rebuild.
- The `setup-toolchain` action restores `.ci-data/sources` with `actions/cache` and sets `DETTIVO_SOURCE_CACHE`. The key hashes the pinned checksums found in `crates/*-sys/*.rs`, so a version bump keeps it. Each job keeps its own key, because jobs build different crates. A changed pin restores the previous set, and the action deletes the files no pin names before the cache is saved. ci.yml, rig.yml and qa-weekly.yml pass `GITHUB_TOKEN: ${{ github.token }}` in their workflow `env`. release.yml builds through rig.yml and inherits it there. The rig's cua-driver install step blanks the token, so the third-party installer never receives it.

## Consequences

- When the curl request follows GitHub's redirect to codeload.github.com or release-assets.githubusercontent.com, it drops the header, as curl does for every cross-host redirect, so that last request stays anonymous. The cache is what keeps a cold build off the network.
- The first run of a job after a pin change, or after the cache is evicted, still downloads.
- Each job's cache holds about 66 MB, the size of the twelve files, in the workspace beside the model caches.

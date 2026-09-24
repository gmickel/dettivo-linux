#!/usr/bin/env bash
# The release gate's receipt, checked before anything publishes: the
# checked-in `docs/reports/release-gate/<version>.json` says the gate
# passed for this version with no failed step and no unexplained blocker,
# and the commit it gated is this release's code: an ancestor of the
# released commit with nothing but `docs/reports/` changed since. The
# workflow runs this in `prepare` when it is about to publish; it reads
# the report and the history, never the network, so it cannot wait for
# its own run.
#
# Usage: scripts/packaging/release-receipt.sh <version> <released-sha> [repo-root]
# Exits 0 with one line naming the gated commit, 1 naming what is missing.
set -euo pipefail

version="${1:?usage: release-receipt.sh <version> <released-sha> [repo-root]}"
version="${version#v}"
released="${2:?usage: release-receipt.sh <version> <released-sha> [repo-root]}"
root="${3:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
receipt="$root/docs/reports/release-gate/${version}.json"

fail() { echo "release-receipt: $1" >&2; exit 1; }

command -v jq >/dev/null || fail "jq is not installed"
[ -f "$receipt" ] || fail "${receipt#"$root"/} is missing: run the gate (docs/RELEASING.md) and commit its report before tagging"
jq -e . "$receipt" >/dev/null 2>&1 || fail "${receipt#"$root"/} is not JSON"

got_version="$(jq -r '.version // ""' "$receipt")"
[ "$got_version" = "$version" ] || fail "${receipt#"$root"/} is the report for version '${got_version}', not ${version}"
[ "$(jq -r '.pack // ""' "$receipt")" = release ] || fail "${receipt#"$root"/} is not a release gate report"
[ "$(jq -r '.passed' "$receipt")" = true ] || fail "${receipt#"$root"/} records the gate as not passed"
failed="$(jq -r '[.steps[] | select(.outcome == "fail") | .id] | join(", ")' "$receipt")"
[ -z "$failed" ] || fail "${receipt#"$root"/} has failed steps: ${failed}"
unexplained="$(jq -r '[.unexplained_blockers[]? | .step] | join(", ")' "$receipt")"
[ -z "$unexplained" ] || fail "${receipt#"$root"/} has unexplained blockers: ${unexplained}"

gated="$(jq -r '.git_sha // ""' "$receipt")"
[ -n "$gated" ] && [ "$gated" != unknown ] || fail "${receipt#"$root"/} names no commit"
git -C "$root" cat-file -e "${gated}^{commit}" 2>/dev/null || fail "the gated commit ${gated:0:12} is not in this repository (a shallow checkout needs fetch-depth 0)"
git -C "$root" merge-base --is-ancestor "$gated" "$released" || fail "the gated commit ${gated:0:12} is not an ancestor of the released ${released:0:12}"
changed="$(git -C "$root" diff --name-only "$gated" "$released" -- . ':(exclude)docs/reports' | head -n 20)"
[ -z "$changed" ] || fail "code changed after the gate at ${gated:0:12}: $(echo "$changed" | tr '\n' ' ')"

echo "release-receipt: ${version} passed the gate at ${gated:0:12}; only docs/reports/ changed since"

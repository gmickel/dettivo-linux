#!/usr/bin/env bash
# The release decision and the release receipt prove themselves: a tag push
# publishes, a rehearsal never does whatever ref it runs on, a mismatched
# tag refuses; a missing, failed, stale or mismatched receipt blocks
# publication and the matching successful one permits it.
#
# Usage: scripts/packaging/test-release-scripts.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
decide="$root/scripts/packaging/release-decide.sh"
receipt="$root/scripts/packaging/release-receipt.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() { echo "test-release-scripts: $1" >&2; exit 1; }

# --- the decision
printf 'version = "0.1.0"\n' >"$work/Cargo.toml"
decision() {
  GITHUB_EVENT_NAME="$1" GITHUB_REF_TYPE="$2" GITHUB_REF_NAME="$3" DRY_RUN="${4:-}" "$decide" "$work/Cargo.toml" 2>/dev/null
}
[ "$(decision push tag v0.1.0)" = $'version=0.1.0\npublish=true' ] || fail "a tag push must publish"
decision push tag v0.2.0 >/dev/null && fail "a mismatched tag must refuse"
decision push branch main >/dev/null && fail "a branch push must refuse"
[ "$(decision workflow_dispatch branch main true)" = $'version=0.1.0\npublish=false' ] || fail "a branch rehearsal must not publish"
[ "$(decision workflow_dispatch tag v0.1.0 true)" = $'version=0.1.0\npublish=false' ] || fail "a rehearsal on the tag must not publish (dry_run wins)"
[ "$(decision workflow_dispatch tag v0.1.0 '')" = $'version=0.1.0\npublish=false' ] || fail "an unset dry_run counts as a rehearsal"
[ "$(decision workflow_dispatch tag v0.1.0 false)" = $'version=0.1.0\npublish=true' ] || fail "a publishing dispatch on the tag must publish"
decision workflow_dispatch branch main false >/dev/null && fail "a publishing dispatch off the tag must refuse"
decision workflow_dispatch tag v0.2.0 false >/dev/null && fail "a publishing dispatch on a mismatched tag must refuse"
decision schedule branch main >/dev/null && fail "another event must refuse"

# --- the receipt, over a small history: gated commit, the report commit, then a code change
repo="$work/repo"
mkdir -p "$repo/docs/reports/release-gate" "$repo/src"
git -C "$repo" init -q
git -C "$repo" config user.email t@example.com
git -C "$repo" config user.name t
echo one >"$repo/src/lib.rs"
git -C "$repo" add -A && git -C "$repo" commit -q -m gated
gated="$(git -C "$repo" rev-parse HEAD)"
write_receipt() {
  local version="$1" sha="$2" passed="$3" step_outcome="$4" unexplained="$5"
  mkdir -p "$repo/docs/reports/release-gate"
  cat >"$repo/docs/reports/release-gate/${version}.json" <<EOF
{"schema_version": 1, "pack": "release", "version": "${version}", "git_sha": "${sha}", "passed": ${passed},
 "steps": [{"id": "version_match", "outcome": "pass"}, {"id": "pack_gui", "outcome": "${step_outcome}"}],
 "blockers": [], "external_blockers": [], "unexplained_blockers": ${unexplained}}
EOF
}
# The receipt has to exit 1 and name the reason; a pipeline's status would
# hide the exit code, so the output goes to a file.
blocks() {
  local pattern="$1"; shift
  local code=0
  "$receipt" "$@" >"$work/out" 2>&1 || code=$?
  [ "$code" -ne 0 ] || return 1
  grep -q "$pattern" "$work/out"
}
blocks "is missing" 0.1.0 "$gated" "$repo" || fail "a missing receipt must block"
write_receipt 0.1.0 "$gated" true pass '[]'
git -C "$repo" add -A && git -C "$repo" commit -q -m "the report"
released="$(git -C "$repo" rev-parse HEAD)"
"$receipt" 0.1.0 "$released" "$repo" >/dev/null || fail "the matching successful receipt must permit publication"
"$receipt" v0.1.0 "$released" "$repo" >/dev/null || fail "the tag form of the version must be accepted"
blocks "is missing" 0.2.0 "$released" "$repo" || fail "a version without a receipt must block"
write_receipt 0.1.0 "$gated" true pass '[]'
sed -i 's/"version": "0.1.0"/"version": "0.0.9"/' "$repo/docs/reports/release-gate/0.1.0.json"
blocks "for version '0.0.9', not 0.1.0" 0.1.0 "$released" "$repo" || fail "another version's receipt must block"
write_receipt 0.1.0 "$gated" false pass '[]'
blocks "not passed" 0.1.0 "$released" "$repo" || fail "a failed gate must block"
write_receipt 0.1.0 "$gated" true fail '[]'
blocks "failed steps: pack_gui" 0.1.0 "$released" "$repo" || fail "a failed step must block by name"
write_receipt 0.1.0 "$gated" true pass '[{"step": "bench_report", "reason": "r", "external": false}]'
blocks "unexplained blockers: bench_report" 0.1.0 "$released" "$repo" || fail "an unexplained blocker must block by name"
write_receipt 0.1.0 0000000000000000000000000000000000000000 true pass '[]'
blocks "not in this repository" 0.1.0 "$released" "$repo" || fail "a receipt for an unknown commit must block"
echo '{' >"$repo/docs/reports/release-gate/0.1.0.json"
blocks "not JSON" 0.1.0 "$released" "$repo" || fail "a corrupt receipt must block"
write_receipt 0.1.0 "$gated" true pass '[]'
echo "the evidence map" >"$repo/docs/reports/evidence-map.md"
git -C "$repo" add -A && git -C "$repo" commit -q -m "another report"
released="$(git -C "$repo" rev-parse HEAD)"
"$receipt" 0.1.0 "$released" "$repo" >/dev/null || fail "reports may change after the gate"
echo two >"$repo/src/lib.rs"
git -C "$repo" add -A && git -C "$repo" commit -q -m "code after the gate"
released="$(git -C "$repo" rev-parse HEAD)"
blocks "code changed after the gate" 0.1.0 "$released" "$repo" || fail "code changed after the gate must block"
git -C "$repo" checkout -q -b other "$gated"
echo three >"$repo/src/lib.rs"
git -C "$repo" add -A && git -C "$repo" commit -q -m "elsewhere"
write_receipt 0.1.0 "$released" true pass '[]'
git -C "$repo" add -A && git -C "$repo" commit -q -m "a receipt for a commit off this line"
blocks "not an ancestor" 0.1.0 "$(git -C "$repo" rev-parse HEAD)" "$repo" || fail "a receipt for a commit outside the released line must block"

echo "test-release-scripts: ok"

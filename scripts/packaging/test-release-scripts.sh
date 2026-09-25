#!/usr/bin/env bash
# The release decision proves itself: a tag push publishes, a rehearsal
# never does whatever ref it runs on, and a mismatched tag refuses.
#
# Usage: scripts/packaging/test-release-scripts.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
decide="$root/scripts/packaging/release-decide.sh"
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

echo "test-release-scripts: ok"

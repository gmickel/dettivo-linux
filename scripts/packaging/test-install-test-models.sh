#!/usr/bin/env bash
# The install test's `--models` value must be absolute: the daemon check
# symlinks it under its own data directory, where a relative path would
# resolve beside the link (the workflow passes `.ci-data/dettivo/models`).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
source "$root/scripts/packaging/install-test-checks.sh"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/.ci-data/dettivo/models"
cd "$fixture"

# The workflow's relative spelling resolves against the working directory.
got="$(resolve_models_dir .ci-data/dettivo/models)"
[ "$got" = "$fixture/.ci-data/dettivo/models" ] || { echo "relative --models resolved to $got" >&2; exit 1; }
# An absolute path is kept as it is.
got="$(resolve_models_dir "$fixture/.ci-data/dettivo/models")"
[ "$got" = "$fixture/.ci-data/dettivo/models" ] || { echo "absolute --models became $got" >&2; exit 1; }
# A symlink under the daemon's data directory then reaches the models.
mkdir -p "$fixture/daemon/data/dettivo"
ln -sfn "$got" "$fixture/daemon/data/dettivo/models"
[ -d "$fixture/daemon/data/dettivo/models/" ] || { echo "the resolved link is dangling" >&2; exit 1; }
# A directory that does not exist is refused by name, never linked.
code=0
message="$(resolve_models_dir .ci-data/nowhere 2>&1)" || code=$?
[ "$code" -eq 2 ] || { echo "a missing --models directory was accepted (exit $code)" >&2; exit 1; }
[[ "$message" == *".ci-data/nowhere"* ]] || { echo "the refusal does not name the directory: $message" >&2; exit 1; }
echo 'install-test models path: 4 cases passed'

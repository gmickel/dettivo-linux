#!/usr/bin/env bash
# Run one build step and name it on failure.
#
# Usage: scripts/run-step.sh <toolchain> <label> <command...>
#
# Every just recipe fans out through this wrapper so that a failing step
# reports which toolchain (cargo, cmake, qmllint, ...) and which command
# failed, and the recipe stops at the first failure.
set -euo pipefail

if [ "$#" -lt 3 ]; then
  echo "usage: $0 <toolchain> <label> <command...>" >&2
  exit 2
fi

toolchain="$1"
label="$2"
shift 2

printf '==> [%s] %s\n' "$toolchain" "$label"
# Capture the command's status directly: after a failed `if` condition, `$?`
# holds the status of the `if` statement itself, which is 0 when no branch ran.
status=0
"$@" || status=$?
if [ "$status" -eq 0 ]; then
  exit 0
fi
printf 'FAILED [%s] %s (exit %d): %s\n' "$toolchain" "$label" "$status" "$*" >&2
exit "$status"

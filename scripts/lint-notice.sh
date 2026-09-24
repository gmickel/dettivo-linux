#!/usr/bin/env bash
# The notice lint (ADR 0034, ADR 0041): every external crate in Cargo.lock
# has its row in NOTICE.md, and every source span reused from another
# project carries the marker with its row in the reuse table. The rules live
# in tools/xtask/src/notice.rs and notice_markers.rs; this script is the
# name docs and the release gate call them by.
#
# Usage: scripts/lint-notice.sh [--write]
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
exec cargo run -q -p xtask -- lint-notice "$@"

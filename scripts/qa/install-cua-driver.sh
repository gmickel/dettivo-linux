#!/usr/bin/env bash
# Install the cua-driver release this repository pins (ADR 0011: pinned per
# release, Wayland support re-checked at every upgrade).
#
# Usage: scripts/qa/install-cua-driver.sh [--check]
#   --check   only report whether the pinned version is on PATH
set -euo pipefail

# The single source of the pinned version is the driver crate.
here="$(cd "$(dirname "$0")/../.." && pwd)"
pinned="$(sed -n 's/^pub const PINNED_VERSION: &str = "\([^"]*\)";/\1/p' "${here}/crates/dettivo-qa/src/driver/cua.rs")"
[[ -n "${pinned}" ]] || { echo "install-cua-driver: PINNED_VERSION not found in crates/dettivo-qa/src/driver/cua.rs" >&2; exit 2; }

if command -v cua-driver >/dev/null 2>&1; then
    have="$(cua-driver --version 2>/dev/null | awk '{print $2}')"
else
    have=""
fi
if [[ "${1:-}" == "--check" ]]; then
    if [[ "${have}" == "${pinned}" ]]; then echo "cua-driver ${have} (pinned)"; exit 0; fi
    echo "cua-driver: have '${have:-none}', pinned ${pinned}" >&2; exit 1
fi
if [[ "${have}" == "${pinned}" ]]; then
    echo "install-cua-driver: ${pinned} already installed"
    exit 0
fi
export CUA_DRIVER_RS_VERSION="${pinned}"
# The installer is fetched to a file over TLS and run explicitly, so a
# failed download never reaches a shell and the script can be inspected.
installer="$(mktemp -t cua-driver-install.XXXXXX.sh)"
trap 'rm -f "${installer}"' EXIT
curl --proto '=https' --tlsv1.2 -fsSL -o "${installer}" https://cua.ai/driver/install.sh
[[ -s "${installer}" ]] || { echo "install-cua-driver: empty installer download" >&2; exit 1; }
bash "${installer}"
export PATH="${HOME}/.local/bin:${PATH}"
have="$(cua-driver --version 2>/dev/null | awk '{print $2}')"
[[ "${have}" == "${pinned}" ]] || { echo "install-cua-driver: installed '${have:-none}', wanted ${pinned}" >&2; exit 1; }
echo "install-cua-driver: ${have} installed"

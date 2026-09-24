#!/usr/bin/env bash
# R1 on a real systemd user session: with the units installed and no daemon
# running, one `dettivo status ping` activates the service and the socket
# answers within 300 ms. Prints the measured time and exits 1 when it is
# over budget or the daemon did not start.
#
# Usage: scripts/qa-activation.sh   (after scripts/install-user-units.sh)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cli="${repo_root}/target/debug/dettivo"
budget_ms=300

if ! systemctl --user is-active --quiet dettivod.socket; then
    echo "qa-activation: dettivod.socket is not active; run scripts/install-user-units.sh" >&2
    exit 2
fi
if [[ ! -x "${cli}" ]]; then
    echo "qa-activation: ${cli} missing; run just build-rust" >&2
    exit 2
fi

systemctl --user stop dettivod.service
if systemctl --user is-active --quiet dettivod.service; then
    echo "qa-activation: could not stop dettivod.service" >&2
    exit 1
fi

start_ns="$(date +%s%N)"
rc=0
"${cli}" --quiet status ping || rc=$?
if [[ ${rc} -ne 0 ]]; then
    echo "qa-activation: ping failed (exit ${rc})" >&2
    exit 1
fi
end_ns="$(date +%s%N)"
elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))

if ! systemctl --user is-active --quiet dettivod.service; then
    echo "qa-activation: ping answered but dettivod.service is not active" >&2
    exit 1
fi

echo "qa-activation: socket activation answered in ${elapsed_ms} ms (budget ${budget_ms} ms)"
if [[ ${elapsed_ms} -gt ${budget_ms} ]]; then
    echo "qa-activation: FAILED, over budget" >&2
    exit 1
fi

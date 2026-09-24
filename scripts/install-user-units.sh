#!/usr/bin/env bash
# Install the systemd user units for a development build and activate the
# socket, so `dettivo status ping` starts the daemon from this checkout, and
# the recording pill's unit pointed at this checkout's dettivo-osd (left
# disabled: `systemctl --user enable --now dettivo-osd.service` starts it).
#
# Usage: scripts/install-user-units.sh [--bin PATH] [--osd-bin PATH] [--uninstall]
#
# The units ship with ExecStart=/usr/bin/... for packaged installs; this
# script writes drop-ins that point ExecStart at the given binaries
# (default: target/debug/dettivod and build/qt/apps/dettivo-osd/dettivo-osd).
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
bin="${repo_root}/target/debug/dettivod"
osd_bin="${repo_root}/build/qt/apps/dettivo-osd/dettivo-osd"
uninstall=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin) bin="$2"; shift 2 ;;
        --osd-bin) osd_bin="$2"; shift 2 ;;
        --uninstall) uninstall=1; shift ;;
        *) echo "install-user-units: unknown argument $1" >&2; exit 2 ;;
    esac
done

if [[ ${uninstall} -eq 1 ]]; then
    systemctl --user disable --now dettivod.socket 2>/dev/null || true
    systemctl --user stop dettivod.service 2>/dev/null || true
    systemctl --user disable --now dettivo-osd.service 2>/dev/null || true
    rm -f "${unit_dir}/dettivod.socket" "${unit_dir}/dettivod.service" "${unit_dir}/dettivo-osd.service"
    rm -rf "${unit_dir}/dettivod.service.d" "${unit_dir}/dettivo-osd.service.d"
    systemctl --user daemon-reload
    echo "install-user-units: removed"
    exit 0
fi

if [[ ! -x "${bin}" ]]; then
    echo "install-user-units: ${bin} is not an executable; build first (just build-rust) or pass --bin" >&2
    exit 2
fi
bin="$(realpath "${bin}")"

mkdir -p "${unit_dir}/dettivod.service.d"
install -m 644 "${repo_root}/systemd/user/dettivod.socket" "${unit_dir}/dettivod.socket"
install -m 644 "${repo_root}/systemd/user/dettivod.service" "${unit_dir}/dettivod.service"
printf '[Service]\nExecStart=\nExecStart=%s\n' "${bin}" > "${unit_dir}/dettivod.service.d/override.conf"

if [[ -x "${osd_bin}" ]]; then
    osd_bin="$(realpath "${osd_bin}")"
    mkdir -p "${unit_dir}/dettivo-osd.service.d"
    install -m 644 "${repo_root}/systemd/user/dettivo-osd.service" "${unit_dir}/dettivo-osd.service"
    printf '[Service]\nExecStart=\nExecStart=%s\n' "${osd_bin}" > "${unit_dir}/dettivo-osd.service.d/override.conf"
fi

systemctl --user daemon-reload
systemctl --user enable --now dettivod.socket
echo "install-user-units: dettivod.socket active, ExecStart=${bin}"
echo "  try: dettivo status ping   (journalctl --user -u dettivod -f for logs)"
if [[ -x "${osd_bin}" ]]; then
    echo "install-user-units: dettivo-osd.service installed, ExecStart=${osd_bin} (enable it with: systemctl --user enable --now dettivo-osd.service)"
fi

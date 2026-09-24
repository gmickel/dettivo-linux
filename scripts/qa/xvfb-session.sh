#!/usr/bin/env bash
# A headless desktop for drives (ADR 0011, R1): Xvfb, a session bus, the
# accessibility bus and a minimal window manager around one command. Used
# by the CI drive job; on a Hyprland desktop the drives run on XWayland
# instead and this script is not needed.
#
# Usage: scripts/qa/xvfb-session.sh <command...>
# Every missing piece is named before anything starts.
set -euo pipefail

need() { command -v "$1" >/dev/null 2>&1 || { echo "xvfb-session: $1 is missing ($2)" >&2; exit 2; }; }
need Xvfb "pacman -S xorg-server-xvfb"
need dbus-run-session "pacman -S dbus"
need openbox "pacman -S openbox"
need busctl "pacman -S systemd"
[[ -x /usr/lib/at-spi-bus-launcher ]] || { echo "xvfb-session: /usr/lib/at-spi-bus-launcher is missing (pacman -S at-spi2-core)" >&2; exit 2; }

display=":${XVFB_DISPLAY_NUMBER:-99}"
# Per-run logs, so parallel or repeated sessions never overwrite each other.
log_dir="${TMPDIR:-/tmp}"
xvfb_log="${log_dir}/xvfb-session-${display#:}-$$.log"
# A previous session on the same display may still be shutting down: its
# socket file would make the readiness check below pass for a server that
# is about to die, so the start waits until the display is free.
for _ in $(seq 1 100); do
    [[ -S "/tmp/.X11-unix/X${display#:}" ]] || break
    sleep 0.05
done
[[ -S "/tmp/.X11-unix/X${display#:}" ]] && { echo "xvfb-session: display ${display} is still in use" >&2; exit 1; }
# The screen holds the app window at its design size (1280 by 820,
# docs/app.md) under the window manager's title bar; a window larger than
# the screen is placed partly off it, and X11 refuses a screenshot of the
# part that is not on screen.
Xvfb "${display}" -screen 0 1600x1000x24 -nolisten tcp >"${xvfb_log}" 2>&1 &
xvfb_pid=$!
# A runtime directory the caller did not provide is ours: created 0700 and
# removed on exit; a caller's directory is left exactly as it is.
owned_runtime_dir=""
if [[ -z "${XDG_RUNTIME_DIR:-}" ]]; then
    owned_runtime_dir="$(mktemp -d -t xvfb-session-runtime.XXXXXX)"
    chmod 700 "${owned_runtime_dir}"
    export XDG_RUNTIME_DIR="${owned_runtime_dir}"
fi
cleanup() {
    kill "${xvfb_pid}" 2>/dev/null || true
    wait "${xvfb_pid}" 2>/dev/null || true
    if [[ -n "${owned_runtime_dir}" ]]; then rm -r -f -- "${owned_runtime_dir}"; fi
    if [[ -n "${bus_conf:-}" ]]; then rm -f -- "${bus_conf}"; fi
    return 0
}
trap cleanup EXIT
for _ in $(seq 1 100); do
    [[ -S "/tmp/.X11-unix/X${display#:}" ]] && break
    sleep 0.05
done
[[ -S "/tmp/.X11-unix/X${display#:}" ]] || { echo "xvfb-session: Xvfb did not open ${display} (log: ${xvfb_log})" >&2; exit 1; }
export DISPLAY="${display}"
# The session is an X11 desktop whatever started it: a Hyprland signature or
# a Wayland display inherited from the caller's desktop would make the
# daemon probe the live compositor's focused window instead of the Xvfb
# window the drive opened (dettivo-insert reads both).
unset WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE
export XDG_SESSION_TYPE=x11
# The private bus activates no service: with the standard service
# directories, a lookup of org.freedesktop.secrets (the CLI's token lookup
# through secret-tool) or of a portal waits out the 120 s activation
# timeout on a machine that has the service files but no session manager
# to start them under this bus. The accessibility bus and the registry
# are started by hand below; nothing else is wanted on it.
bus_conf="$(mktemp -t xvfb-session-bus.XXXXXX.conf)"
cat >"${bus_conf}" <<'CONF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <keep_umask/>
  <listen>unix:tmpdir=/tmp</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF

# Inside the session bus: launch the accessibility bus and the WM, then run.
# Not exec: the EXIT trap above must still run to stop Xvfb and remove an
# owned runtime directory, so the session is a child and its status is
# passed through.
dbus-run-session --config-file="${bus_conf}" -- bash -c '
    set -euo pipefail
    /usr/lib/at-spi-bus-launcher --launch-immediately &
    a11y=$!
    openbox >"${TMPDIR:-/tmp}/xvfb-session-openbox-$$.log" 2>&1 &
    wm=$!
    trap "kill ${wm} ${a11y} 2>/dev/null || true" EXIT
    for _ in $(seq 1 100); do
        busctl --user call org.a11y.Bus /org/a11y/bus org.a11y.Bus GetAddress >/dev/null 2>&1 && break
        sleep 0.05
    done
    # The registry is started by hand: on a machine whose accessibility
    # bus is dbus-broker, its activation goes through systemd, which a
    # private session has no manager for, and every snapshot would fail
    # naming org.a11y.atspi.Registry. Started first, it owns the name and
    # no activation is attempted.
    if [[ -x /usr/lib/at-spi2-registryd ]]; then
        /usr/lib/at-spi2-registryd >/dev/null 2>&1 &
        registry=$!
        trap "kill ${wm} ${a11y} ${registry} 2>/dev/null || true" EXIT
    fi
    # The window manager owns focus; a drive that starts before it is
    # managing windows sees no active window. xprop names the manager once
    # it is up; without xprop a short grace period stands in.
    if command -v xprop >/dev/null 2>&1; then
        for _ in $(seq 1 400); do
            xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q "window id" && break
            sleep 0.05
        done
        xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q "window id" \
            || {
                echo "xvfb-session: openbox is not managing ${DISPLAY} yet after 20 s; the drive decides" >&2
                echo "xvfb-session: openbox log:" >&2
                tail -n 20 "${TMPDIR:-/tmp}/xvfb-session-openbox-$$.log" >&2 || true
                xprop -root 2>&1 | head -n 20 >&2 || true
            }
    else
        sleep 1
    fi
    busctl --user call org.a11y.Bus /org/a11y/bus org.a11y.Bus GetAddress >/dev/null 2>&1 \
        || { echo "xvfb-session: the accessibility bus (org.a11y.Bus) did not answer" >&2; exit 1; }
    export QT_QPA_PLATFORM=xcb QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
    # Input may focus windows only on this disposable desktop. CUA cannot
    # deliver Return or respect selection on every background Qt field.
    export CUA_DRIVER_DELIVERY_MODE=foreground
    "$@"
' xvfb-session "$@"
status=$?
exit "${status}"

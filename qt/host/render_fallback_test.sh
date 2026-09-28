#!/usr/bin/env bash
# A host whose GPU scene graph cannot start still draws its window, on the
# software renderer, instead of aborting (ADR 0077). A private Xvfb with
# Qt's xcb GL integration switched off leaves Qt Quick no OpenGL at all,
# which fails the same way a GPU with no free memory did, without using a
# GPU. The host's `--render` proves the window drew: the PNG exists.
#
# Usage: render_fallback_test.sh <host binary> <work directory>
# The host's render variables (QA mode, states) come from the environment.
set -euo pipefail

binary="$1"
work="$2"
name="$(basename "${binary}")"
command -v Xvfb >/dev/null 2>&1 || { echo "render-fallback: Xvfb is missing (pacman -S xorg-server-xvfb)" >&2; exit 2; }
mkdir -p "${work}"

# A display nobody holds: no socket and no lock. -displayfd is never used:
# Xvfb then unlinks a running server's socket (the desktop's XWayland on
# :0) before it finds that display taken. Xvfb's own lock settles a race
# with a parallel run, and the loser tries the next number. No GLX means
# the X server loads no GL driver either.
xvfb_pid=""
trap '[[ -n "${xvfb_pid}" ]] && kill "${xvfb_pid}" 2>/dev/null || true' EXIT
for n in $(seq 100 199); do
    [[ -e "/tmp/.X11-unix/X${n}" || -e "/tmp/.X11-unix/X${n}_" || -e "/tmp/.X${n}-lock" ]] && continue
    Xvfb ":${n}" -screen 0 1600x1000x24 -nolisten tcp -extension GLX 2>>"${work}/xvfb.log" &
    xvfb_pid=$!
    for _ in $(seq 1 200); do
        [[ -S "/tmp/.X11-unix/X${n}" ]] && break
        kill -0 "${xvfb_pid}" 2>/dev/null || break
        sleep 0.05
    done
    if [[ -S "/tmp/.X11-unix/X${n}" ]] && kill -0 "${xvfb_pid}" 2>/dev/null; then
        export DISPLAY=":${n}"
        break
    fi
    kill "${xvfb_pid}" 2>/dev/null || true
    wait "${xvfb_pid}" 2>/dev/null || true
    xvfb_pid=""
done
[[ -n "${xvfb_pid}" ]] || { echo "render-fallback: no Xvfb display from :100 to :199 started (${work}/xvfb.log)" >&2; exit 2; }

export QT_QPA_PLATFORM=xcb QT_XCB_GL_INTEGRATION=none
unset WAYLAND_DISPLAY QT_QPA_PLATFORMTHEME QT_QUICK_BACKEND QSG_RHI_BACKEND DETTIVO_RENDER_FALLBACK

failures=0
fail() {
    echo "render-fallback: ${name}: $*" >&2
    failures=$((failures + 1))
}

# run <case> [VAR=value...]: one render under the extra variables; leaves
# the exit status in `code` and the output in `log`.
run() {
    local case="$1"
    shift
    log="${work}/${case}.log"
    png="${work}/${case}.png"
    rm -f "${png}"
    set +e
    env "$@" timeout 60 "${binary}" --render "${png}" >"${log}" 2>&1
    code=$?
    set -e
}
restarts() { grep -c "restarting with the software renderer" "${log}" || true; }
drew() { [[ ${code} -eq 0 && -s "${png}" ]]; }

# The GPU path fails: one warning naming Qt's error, one restart, a window.
run fallback
drew || fail "fallback: exit ${code}, no picture (${log})"
[[ "$(restarts)" -eq 1 ]] || fail "fallback: expected exactly one restart warning (${log})"
grep -q "warning: the GPU could not start the scene graph (Failed to initialize graphics backend" "${log}" \
    || fail "fallback: the warning does not name Qt's error (${log})"
grep -q "renderer: fallback: Failed to initialize graphics backend" "${log}" \
    || fail "fallback: the restarted process does not say why it draws in software (${log})"

# A process that is already the restart exits 1 with a clear error and
# never restarts again.
run guard DETTIVO_RENDER_FALLBACK="an earlier scene-graph error"
[[ ${code} -eq 1 ]] || fail "guard: expected exit 1, got ${code} (${log})"
[[ "$(restarts)" -eq 0 ]] || fail "guard: the restart ran twice (${log})"
grep -q "cannot draw the window with the software renderer either" "${log}" \
    || fail "guard: no clear error (${log})"

# `[app] renderer = "software"` draws in software from the start.
printf '[app]\nrenderer = "software"\n' >"${work}/config.toml"
run config DETTIVO_CONFIG="${work}/config.toml"
drew || fail "config: exit ${code}, no picture (${log})"
[[ "$(restarts)" -eq 0 ]] || fail "config: the GPU path ran despite the setting (${log})"
grep -q "renderer: config: \[app\] renderer = software" "${log}" || fail "config: the setting is not reported (${log})"

# A user-set QT_QUICK_BACKEND beats the file: the GPU path is tried again.
run environment DETTIVO_CONFIG="${work}/config.toml" QT_QUICK_BACKEND=rhi
drew || fail "environment: exit ${code}, no picture (${log})"
[[ "$(restarts)" -eq 1 ]] || fail "environment: QT_QUICK_BACKEND did not win over the file (${log})"

[[ ${failures} -eq 0 ]] || exit 1
echo "render-fallback: ${name}: the window draws in software after a GPU failure, once"

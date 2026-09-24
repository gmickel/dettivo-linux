#!/usr/bin/env bash
# Install the built package and prove it works: the file list against
# packaging/manifest.txt, the icons against the artboard's crops, the
# desktop entry, the units, the CLI's version
# and completions, the app rendered from the installed QML module, every
# engine's help, the CPU-fallback smoke and one mock-microphone dictation
# through the installed daemon; with --session the real systemd user units
# too. Every check lands in install-test.json naming its step, outcome,
# detail and exit code (ADR 0034).
#
# Usage: scripts/packaging/install-test.sh [options] <package.pkg.tar.zst>
#   --root <dir>     unpack the package under <dir> instead of installing it
#                    with pacman (a development machine; needs no root and
#                    touches nothing outside <dir>)
#   --session        also enable the socket, prove activation under 300 ms,
#                    check the compositor setup and start dettivo-osd.service
#                    (a real user session; implies a pacman install)
#   --models <dir>   the model directory (default: the XDG one); a missing
#                    model skips its smoke naming the fetch script
#   --report <file>  where install-test.json goes (default: ./install-test.json)
#   --out <dir>      the render and the logs (default: beside the report)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/packaging/install-test-checks.sh
source "$repo_root/scripts/packaging/install-test-checks.sh"

root=""
session=0
models="${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models"
report="$PWD/install-test.json"
out=""
package=""
while [ $# -gt 0 ]; do
  case "$1" in
    --root) root="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
    --session) session=1; shift ;;
    --models) models="$(resolve_models_dir "$2")" || exit 2; shift 2 ;;
    --report) report="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    -*) echo "install-test: unknown option $1" >&2; exit 2 ;;
    *)
      [ -z "$package" ] || { echo "install-test: expected exactly one package" >&2; exit 2; }
      package="$1"; shift ;;
  esac
done
[ -n "$package" ] || { echo "usage: install-test.sh [--root <dir>] [--session] <package.pkg.tar.zst>" >&2; exit 2; }
case "$(basename "$package")" in
  dettivo-bin-debug-*|dettivo-debug-*) echo "install-test: expected an application package, not debug symbols" >&2; exit 2 ;;
  dettivo-bin-[0-9]*.pkg.tar.zst|dettivo-[0-9]*.pkg.tar.zst) ;;
  *) echo "install-test: expected a dettivo application package" >&2; exit 2 ;;
esac
[ -f "$package" ] || { echo "install-test: $package does not exist" >&2; exit 2; }
if [ "$session" = 1 ] && [ -n "$root" ]; then
  echo "install-test: --session needs a real install; drop --root" >&2
  exit 2
fi
mkdir -p "$(dirname "$report")"
out="${out:-$(dirname "$report")/install-test}"
mkdir -p "$out"

# Where the installed files live: a prefix root, or the system.
prefix="${root:-}"
bin="$prefix/usr/bin"
engines="$prefix/usr/lib/dettivo/engines"
qml="$prefix/usr/lib/dettivo/qml"
version="$(grep -m1 -E '^version = ' "$repo_root/Cargo.toml" | sed -E 's/version = "([^"]+)"/\1/')"

steps_json=()
failed=0
# record <step> <status> <exit> <detail>
record() {
  local step="$1" status="$2" code="$3" detail="$4" escaped
  printf '%-6s %-22s %s\n' "$status" "$step" "$detail"
  escaped="${detail//\\/\\\\}"; escaped="${escaped//\"/\\\"}"; escaped="${escaped//$'\n'/ }"
  steps_json+=("{\"step\":\"$step\",\"status\":\"$status\",\"exit_code\":$code,\"detail\":\"$escaped\"}")
  [ "$status" = pass ] || failed=$((failed + 1))
  return 0
}
# run <step> <detail-on-pass> <command...>: pass on exit 0, else fail naming the exit code and the log.
run() {
  local step="$1" detail="$2"; shift 2
  local log="$out/$step.log" code=0
  "$@" >"$log" 2>&1 || code=$?
  if [ "$code" -eq 0 ]; then
    record "$step" pass 0 "$detail"
  else
    record "$step" fail "$code" "$* exited $code; see $log: $(tail -n 3 "$log" | tr '\n' ' ')"
  fi
}

# The installed binaries and libraries in a prefix; nothing set on a system
# install, which is what proves the compiled-in paths.
prefix_env() {
  if [ -n "$prefix" ]; then
    echo "PATH=$bin:$engines:$PATH"
    echo "LD_LIBRARY_PATH=$qml/Dettivo:$qml/DettivoStyle${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    echo "QML_IMPORT_PATH=$qml"
  fi
}

check_install
check_files
check_icons
check_desktop_and_units
check_cli
check_completions
check_render
check_engines_help
check_cpu_smoke
check_daemon_dictation
if [ "$session" = 1 ]; then
  # shellcheck source=scripts/packaging/install-test-session.sh
  source "$repo_root/scripts/packaging/install-test-session.sh"
  check_session
fi

{
  echo "{"
  echo "  \"schema_version\": 1,"
  echo "  \"package\": \"$(basename "$package")\","
  echo "  \"version\": \"$version\","
  echo "  \"mode\": \"$([ -n "$prefix" ] && echo prefix || echo system)\","
  echo "  \"session\": $([ "$session" = 1 ] && echo true || echo false),"
  echo "  \"passed\": $([ "$failed" -eq 0 ] && echo true || echo false),"
  echo "  \"steps\": ["
  printf '    %s' "${steps_json[0]}"
  for s in "${steps_json[@]:1}"; do printf ',\n    %s' "$s"; done
  echo
  echo "  ]"
  echo "}"
} >"$report"
if [ "$failed" -ne 0 ]; then
  echo "install-test: $failed step(s) failed; report $report" >&2
  exit 1
fi
echo "install-test: every step passed; report $report"

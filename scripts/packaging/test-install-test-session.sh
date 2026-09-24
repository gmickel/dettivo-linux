#!/usr/bin/env bash
# The --session arm judges `dettivo setup <compositor> --check` by its exit
# status and picks the compositor from the desktop, not from the CLI's
# help text: a failed check is a failed step, Omarchy is Omarchy only when
# its command is on PATH, and DETTIVO_INSTALL_TEST_COMPOSITOR names the
# target outright.
#
# Usage: scripts/packaging/test-install-test-session.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/packaging/install-test-session.sh
source "$root/scripts/packaging/install-test-session.sh"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
out="$fixture/out"
mkdir -p "$fixture/bin" "$out"

# Stubs: dettivo answers ping, writes the snippet, and fails --check when
# STUB_CHECK_FAILS is set; systemctl and sleep do nothing.
cat >"$fixture/bin/dettivo" <<'CLI'
#!/usr/bin/env bash
echo "$*" >>"$STUB_CALLS"
case "$*" in
  *--check*) if [ -n "${STUB_CHECK_FAILS:-}" ]; then echo "not written"; exit 1; fi; echo "sourced" ;;
esac
exit 0
CLI
cat >"$fixture/bin/systemctl" <<'STUB'
#!/usr/bin/env bash
exit 0
STUB
cat >"$fixture/bin/sleep" <<'STUB'
#!/usr/bin/env bash
exit 0
STUB
cat >"$fixture/bin/omarchy" <<'STUB'
#!/usr/bin/env bash
exit 0
STUB
chmod +x "$fixture/bin/"*
export STUB_CALLS="$fixture/calls"
# Only the stubs and the coreutils: the machine's own omarchy must not
# reach the check.
mkdir -p "$fixture/tools"
for tool in bash env date tr grep cat mkdir tail rm; do ln -s "$(command -v "$tool")" "$fixture/tools/$tool"; done
export PATH="$fixture/bin:$fixture/tools"

declare -A outcome
record() { outcome["$1"]="$2"; }
run() { local step="$1"; shift 2; if "$@" >/dev/null 2>&1; then record "$step" pass; else record "$step" fail; fi; }

fail() { echo "test-install-test-session: $1" >&2; exit 1; }

# Omarchy on PATH picks omarchy; a passing check passes.
: >"$STUB_CALLS"
check_session
[ "${outcome[session_setup_check]}" = pass ] || fail "a passing --check must pass"
grep -q '^setup omarchy --check$' "$STUB_CALLS" || fail "omarchy on PATH must pick omarchy: $(cat "$STUB_CALLS")"

# A failed check is a failed step.
: >"$STUB_CALLS"
STUB_CHECK_FAILS=1 check_session
[ "${outcome[session_setup_check]}" = fail ] || fail "a failed --check must fail the step"

# Without omarchy on PATH the desktop is plain Hyprland.
rm "$fixture/bin/omarchy"
hash -r
: >"$STUB_CALLS"
check_session
grep -q '^setup hyprland --check$' "$STUB_CALLS" || fail "no omarchy on PATH must pick hyprland: $(cat "$STUB_CALLS")"

# An explicit target wins.
: >"$STUB_CALLS"
DETTIVO_INSTALL_TEST_COMPOSITOR=sway check_session
grep -q '^setup sway --check$' "$STUB_CALLS" || fail "the explicit target must win: $(cat "$STUB_CALLS")"

echo "test-install-test-session: 4 cases passed"

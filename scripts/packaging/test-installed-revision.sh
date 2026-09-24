#!/usr/bin/env bash
# The clean-install release gate must reject an older binary of the same version.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
source "$root/scripts/packaging/install-test-checks.sh"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
bin="$fixture"
version=0.1.0
cat > "$bin/dettivo" <<'CLI'
#!/usr/bin/env bash
printf 'dettivo 0.1.0 (%s)\n' "$FIXTURE_SHA"
CLI
chmod +x "$bin/dettivo"
record() { outcome="$2"; }
export FIXTURE_SHA=abc123
export DETTIVO_EXPECTED_GIT_SHA=abc123
check_cli
[ "$outcome" = pass ]
export DETTIVO_EXPECTED_GIT_SHA=def456
check_cli
[ "$outcome" = fail ] || { echo 'older same-version binary was accepted' >&2; exit 1; }
unset DETTIVO_EXPECTED_GIT_SHA
check_cli
[ "$outcome" = pass ]
echo 'installed revision: 3 cases passed'

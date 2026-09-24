#!/usr/bin/env bash
# Required install checks cannot pass through a skip or an unknown backend.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/scripts/packaging" "$work/models/whisper/tiny.en" "$work/engines"
cp "$repo/Cargo.toml" "$work/"
cp "$repo/scripts/packaging/install-test.sh" "$work/scripts/packaging/"
for check in install files icons desktop_and_units cli completions render engines_help cpu_smoke daemon_dictation; do
  printf 'check_%s() { record %s pass 0 fixture; }\n' "$check" "$check" >>"$work/scripts/packaging/install-test-checks.sh"
done
printf 'check_render() { record render skip 0 "missing renderer"; }\n' >>"$work/scripts/packaging/install-test-checks.sh"
touch "$work/dettivo-bin-0.1.0-1-x86_64.pkg.tar.zst"
if bash "$work/scripts/packaging/install-test.sh" --root "$work/prefix" --report "$work/report.json" "$work/dettivo-bin-0.1.0-1-x86_64.pkg.tar.zst" >"$work/log" 2>&1; then
  echo 'test-install-receipt: skipped required check passed' >&2; exit 1
fi
jq -e '.passed == false and any(.steps[]; .step == "render" and .status == "skip")' "$work/report.json" >/dev/null
# shellcheck source=scripts/packaging/install-test-checks.sh
source "$repo/scripts/packaging/install-test-checks.sh"
models="$work/models"
engines="$work/engines"
touch "$models/whisper/tiny.en/ggml-tiny.en.bin"
printf '#!/usr/bin/env bash\necho broken >&2\nexit 7\n' >"$engines/dettivo-engine-whisper"
chmod +x "$engines/dettivo-engine-whisper"
cpu_smoke() { :; }
record() { printf '%s %s %s\n' "$1" "$2" "$3" >"$work/natural"; }
check_cpu_smoke
grep -q '^natural_backend fail ' "$work/natural"
echo 'test-install-receipt: skipped required check and broken backend fail'

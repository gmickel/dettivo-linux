#!/usr/bin/env bash
# The docs build proves itself (ADR 0041): a clean copy of this repository
# passes scripts/check-docs.sh, and each plant fails it naming the file:
# an unreachable page, a schema key missing from
# docs/config.md, a changed CLI verb, an unindexed decision record, a
# migration name the chain does not have, a `name=value` argument to a
# just recipe and an argument past a recipe's parameters.
#
# Usage: scripts/test-check-docs.sh
#   DETTIVO_CLI=<path>   the `dettivo` binary (default: target/debug/dettivo,
#                        built on demand)
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cli="${DETTIVO_CLI:-$repo/target/debug/dettivo}"
if [ ! -x "$cli" ]; then
  (cd "$repo" && cargo build -q -p dettivo-cli)
fi
[ -x "$cli" ] || { echo "test-check-docs: $cli is not an executable" >&2; exit 2; }
cli="$(realpath "$cli")"

work="$(mktemp -d -t dettivo-check-docs.XXXXXX)"
trap 'rm -r -f -- "$work"' EXIT
git clone -q --shared --no-checkout "$repo" "$work/clone"
clone="$work/clone"
# The working tree as it is now, staged changes included, so the test
# judges what a commit would carry.
git -C "$repo" ls-files -z | (cd "$repo" && tar --null -T - -cf - 2>/dev/null) | tar -C "$clone" -xf -
git -C "$clone" add -A >/dev/null 2>&1
git -C "$clone" -c user.name=test -c user.email=test@example.invalid commit -q --allow-empty -m baseline

fail() { echo "test-check-docs: $1" >&2; exit 1; }
run() {
  # $1: the expected substring of the findings; the rest is the plant.
  local expect="$1" out
  shift
  "$@"
  out="$(DETTIVO_CLI="$cli" "$clone/scripts/check-docs.sh" 2>&1 || true)"
  case "$out" in
    *"$expect"*) ;;
    *) fail "the plant '$expect' was not named; output was:
$out" ;;
  esac
  git -C "$clone" reset -q --hard >/dev/null 2>&1
  git -C "$clone" clean -q -fd >/dev/null 2>&1 || true
}

if ! DETTIVO_CLI="$cli" "$clone/scripts/check-docs.sh" >/dev/null 2>"$work/clean.log"; then
  fail "the clean tree must pass: $(cat "$work/clean.log")"
fi

plant_orphan() {
  printf '# Orphan\n\nA page nothing links.\n' >"$clone/docs/orphan.md"
  git -C "$clone" add docs/orphan.md
}
run "docs/orphan.md is not reachable from README.md" plant_orphan

plant_key() {
  sed -i '/shutdown_timeout_ms/d' "$clone/docs/config.md"
}
run "docs/config.md does not name the schema key daemon.shutdown_timeout_ms" plant_key

fake="$work/dettivo-fake"
cat >"$fake" <<EOF
#!/usr/bin/env bash
"$cli" "\$@" | sed 's/^├─ doctor\\*/├─ doctorx*/'
EOF
chmod +x "$fake"
plant_verb() { :; }
out="$(DETTIVO_CLI="$fake" "$clone/scripts/check-docs.sh" 2>&1 || true)"
case "$out" in
  *"docs/guides/agents.md:"*"the CLI tree differs"*) ;;
  *) fail "a changed CLI verb was not named with its line; output was:
$out" ;;
esac

plant_adr() {
  printf '# 0099. A record\n\nStatus: Proposed\n\n## What this gives you\n\nNothing yet.\n' >"$clone/docs/adr/0099-a-record.md"
  git -C "$clone" add docs/adr/0099-a-record.md
}
run "docs/adr/0099-a-record.md is not listed in docs/adr/README.md" plant_adr

plant_migration() {
  sed -i 's/`0007-notes-analysis`/`0005-notes-analysis`/' "$clone/docs/history.md"
}
run "docs/history.md:24 names migration \`0005-notes-analysis\`, which is not in crates/dettivo-storage/migrations/" plant_migration

plant_named_argument() {
  printf '\n`just qa-release engines=target/vulkan/debug` is the gate.\n' >>"$clone/docs/qa.md"
}
run "passes \`engines=target/vulkan/debug\` to \`just qa-release\`" plant_named_argument

plant_extra_argument() {
  printf '\n`just qa-pack gui --surface omarchy` narrows the pack.\n' >>"$clone/docs/qa.md"
}
run "passes \`--surface\` to \`just qa-pack\`, which takes 1 argument(s)" plant_extra_argument

echo "test-check-docs: ok (the clean tree passes; seven plants fail by file)"

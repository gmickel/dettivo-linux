#!/usr/bin/env bash
# Every key of the configuration schema has an editor on a settings route
# or a documented reason not to (fn-26 R1, ADR 0033). The schema's keys
# come from the `config.keys` fixture, which the proto tests hold to the
# daemon's registry; the routes' keys come from the one table the app
# compiles in (qt/host/app/settings_keys.cpp, between its markers), which
# the round-trip drive reads too; docs/config.md names every route key
# under "Keys by route". A key in the schema that no route edits and no
# reason covers fails by name, as does a route key the schema does not
# have or the documentation does not list.
#
# Usage: scripts/lint-settings-keys.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="${repo_root}/crates/dettivo-proto/fixtures/config/keys.json"
table="${repo_root}/qt/host/app/settings_keys.cpp"
doc="${repo_root}/docs/config.md"
status=0

for needed in "${fixture}" "${table}" "${doc}"; do
    if [[ ! -f "${needed}" ]]; then
        echo "lint-settings-keys: ${needed#"${repo_root}"/} missing" >&2
        exit 2
    fi
done
command -v jq >/dev/null 2>&1 || { echo "lint-settings-keys: jq missing (pacman -S jq)" >&2; exit 2; }

# The quoted strings of every `{"a", "b"}` line between two markers, one
# pair per line as `a<TAB>b`.
marked() {
    sed -n "/${1}/,/${2}/p" "${table}" \
        | grep -E '^[[:space:]]*\{"' \
        | sed -E 's/^[[:space:]]*\{"([^"]+)", "([^"]+)".*/\1\t\2/'
}

schema_keys="$(jq -r '.. | objects | select(has("key") and has("kind")) | .key' "${fixture}" | sort -u)"
route_pairs="$(marked 'routes:start' 'routes:end')"
route_keys="$(printf '%s\n' "${route_pairs}" | cut -f2 | sort -u)"
excluded_keys="$(marked 'excluded:start' 'excluded:end' | cut -f1 | sort -u)"

if [[ -z "${schema_keys}" || -z "${route_keys}" ]]; then
    echo "lint-settings-keys: no keys read from the fixture or the table" >&2
    exit 2
fi

while IFS= read -r key; do
    [[ -z "${key}" ]] && continue
    if ! grep -qxF "${key}" <<<"${route_keys}" && ! grep -qxF "${key}" <<<"${excluded_keys}"; then
        echo "lint-settings-keys: ${key} is in the schema but no settings route edits it and settings_keys.cpp gives no reason" >&2
        status=1
    fi
done <<<"${schema_keys}"

while IFS= read -r key; do
    [[ -z "${key}" ]] && continue
    if ! grep -qxF "${key}" <<<"${schema_keys}"; then
        echo "lint-settings-keys: ${key} is listed for a route but the schema has no such key" >&2
        status=1
    fi
    if grep -qxF "${key}" <<<"${excluded_keys}"; then
        echo "lint-settings-keys: ${key} is both edited by a route and excluded with a reason" >&2
        status=1
    fi
done <<<"${route_keys}"

# Every route key is documented under its route in docs/config.md.
by_route="$(sed -n '/^## Keys by route/,/^## /p' "${doc}")"
while IFS=$'\t' read -r section key; do
    [[ -z "${key}" ]] && continue
    if ! grep -qF "\`${key}\`" <<<"${by_route}"; then
        echo "lint-settings-keys: ${key} (${section}) is not listed under \"Keys by route\" in docs/config.md" >&2
        status=1
    fi
done <<<"${route_pairs}"
while IFS= read -r key; do
    [[ -z "${key}" ]] && continue
    if ! grep -qF "\`${key}\`" <<<"${by_route}"; then
        echo "lint-settings-keys: ${key} is excluded from the routes but not named under \"Keys by route\" in docs/config.md" >&2
        status=1
    fi
done <<<"${excluded_keys}"

if [[ ${status} -ne 0 ]]; then
    echo "lint-settings-keys: FAILED" >&2
    exit "${status}"
fi
echo "lint-settings-keys: OK ($(wc -l <<<"${schema_keys}") schema keys, $(wc -l <<<"${route_keys}") edited, $(wc -l <<<"${excluded_keys}") with a reason)"

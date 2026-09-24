#!/usr/bin/env bash
# The prerequisite check and the CI setup agree: every tool
# scripts/check-toolchain.sh names with a one-package `pacman -S <pkg>`
# hint is in the package list .github/actions/setup-toolchain/action.yml
# installs, so a tool a lint needs is missing from the pinned container
# only when the check would already have named it; and the check does name
# a missing tool, proven with jq hidden from PATH.
#
# Usage: scripts/test-check-toolchain.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"
check="scripts/check-toolchain.sh"
action=".github/actions/setup-toolchain/action.yml"
status=0
fail() { echo "test-check-toolchain: $*" >&2; status=1; }

# 1. Every single-package hint of the check names a package the CI setup installs.
installed="$(awk '/pacman -Syu --noconfirm --needed/ { on = 1; next } on && /^ *[a-z]/ { print } on && !/\\$/ { on = 0 }' "${action}" | tr -s ' \\' '\n' | sed '/^$/d')"
while IFS= read -r pkg; do
    [[ -n "${pkg}" ]] || continue
    if ! grep -qx "${pkg}" <<<"${installed}"; then
        fail "${check} asks for ${pkg} (pacman -S ${pkg}) but ${action} does not install it"
    fi
done < <(grep -oE 'pacman -S [a-z0-9-]+"' "${check}" | sed -E 's/pacman -S ([a-z0-9-]+)"/\1/' | sort -u)

# 2. A missing jq is named before anything compiles.
shadow="$(mktemp -d)"
trap 'rm -r -f -- "${shadow}"' EXIT
while IFS= read -r dir; do
    [[ -d "${dir}" ]] || continue
    for tool in "${dir}"/*; do
        name="$(basename "${tool}")"
        [[ "${name}" == "jq" ]] && continue
        [[ -e "${shadow}/${name}" ]] && continue
        ln -s "${tool}" "${shadow}/${name}" 2>/dev/null || true
    done
done < <(tr ':' '\n' <<<"${PATH}")
out="$(PATH="${shadow}" bash "${check}" 2>&1 || true)"
case "${out}" in
    *"jq (JSON tool"*) ;;
    *) fail "with jq hidden from PATH the check did not name it; output was:
${out}" ;;
esac

if [[ "${status}" -eq 0 ]]; then
    echo "test-check-toolchain: ok (the hints match the CI setup; a hidden jq is named)"
fi
exit "${status}"

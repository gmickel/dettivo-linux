#!/usr/bin/env bash
# Export the plugin folder into a checkout of the omarchy-dettivo mirror
# repository, whose root equals omarchy/ byte for byte: the mirror's history
# is written by the release from this script, never by hand. The manifest
# version already follows Cargo.toml (the lint keeps them equal), so the
# copy is exact; the checkout's .git is the only thing left untouched.
#
# Usage: scripts/omarchy/export-plugin.sh <mirror-checkout>
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${1:?mirror checkout}"
[[ -d "${target}" ]] || { echo "export-plugin: ${target} is not a directory" >&2; exit 2; }

version="$(grep -m1 -E '^version = ' "${repo_root}/Cargo.toml" | sed -E 's/version = "([^"]+)"/\1/')"
manifest_version="$(jq -r .version "${repo_root}/omarchy/manifest.json")"
if [[ "${manifest_version}" != "${version}" ]]; then
    echo "export-plugin: omarchy/manifest.json is ${manifest_version}, Cargo.toml is ${version}; run scripts/lint-omarchy-plugin.sh" >&2
    exit 1
fi

# Everything but .git goes; anything in the checkout that is not in omarchy/
# is removed so the two trees match.
find "${target}" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
cp -r "${repo_root}/omarchy/." "${target}/"

if command -v omarchy-plugin-validate >/dev/null 2>&1; then
    omarchy-plugin-validate "${target}"
fi
if ! diff -r --exclude=.git "${repo_root}/omarchy" "${target}" >/dev/null; then
    echo "export-plugin: ${target} differs from omarchy/ after the copy" >&2
    exit 1
fi
echo "export-plugin: ${target} now equals omarchy/ (version ${version})"

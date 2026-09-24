#!/usr/bin/env bash
# Extract one version's section from CHANGELOG.md as the release notes, and
# check that the tag, Cargo.toml and the changelog agree on the version.
#
# Usage: scripts/packaging/release-notes.sh <version> [notes-file]
#
# Prints the notes (or writes them to notes-file) and exits 1 naming the
# problem when Cargo.toml carries another version or CHANGELOG.md has no
# `## [<version>]` heading, so a release fails before anything is built.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="${1:?usage: release-notes.sh <version> [notes-file]}"
version="${version#v}"
notes="${2:-}"

cargo_version="$(grep -m1 -E '^version = ' "$root/Cargo.toml" | sed -E 's/version = "([^"]+)"/\1/')"
if [ "$cargo_version" != "$version" ]; then
  echo "release-notes: the tag says $version but Cargo.toml says $cargo_version" >&2
  exit 1
fi

section="$(awk -v v="$version" '
  /^## \[/ { in_section = ($0 ~ "^## \\[" v "\\]") ; if (in_section) { found = 1; next } }
  in_section { print }
  END { if (!found) exit 3 }
' "$root/CHANGELOG.md")" || {
  echo "release-notes: CHANGELOG.md has no '## [$version]' section" >&2
  exit 1
}
section="$(printf '%s\n' "$section" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')"
if [ -z "$(tr -d '[:space:]' <<<"$section")" ]; then
  echo "release-notes: the '## [$version]' section of CHANGELOG.md is empty" >&2
  exit 1
fi
if [ -n "$notes" ]; then
  printf '%s\n' "$section" >"$notes"
  echo "release-notes: $notes ($(wc -l <"$notes") lines for $version)"
else
  printf '%s\n' "$section"
fi

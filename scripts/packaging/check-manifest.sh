#!/usr/bin/env bash
# Compare a release tree, or an installed package's file list, with
# packaging/manifest.txt.
#
# Usage: scripts/packaging/check-manifest.sh [--cuda] <root>
#        scripts/packaging/check-manifest.sh --files <list>
#        scripts/packaging/check-manifest.sh --update <root>
#
# The manifest lists every file the package installs, relative to the root
# (`usr/bin/dettivod`), the Omarchy plugin's files among them. Every file
# under the root must be listed and every listed file must exist; a missing
# or extra path is printed with its name and the script exits 1.
#
# `--files <list>` compares a file list instead of walking a root: one path
# per line as `pacman -Ql` prints it (an absolute path, directories ending
# in `/`), so an installed package is checked without enumerating the
# machine. `--update <root>` rewrites the file list from a staged tree,
# keeping the header.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest="$repo_root/packaging/manifest.txt"
if [ "${1:-}" = --cuda ]; then
  manifest="$repo_root/packaging/manifest-cuda.txt"
  shift
fi

mode=root
case "${1:-}" in
  --files) mode=files; list="${2:?usage: check-manifest.sh --files <list>}" ;;
  --update) mode=update; root="${2:?usage: check-manifest.sh --update <root>}" ;;
  "") echo "usage: check-manifest.sh <root> | --files <list> | --update <root>" >&2; exit 2 ;;
  *) root="$1" ;;
esac

expected=()
while IFS= read -r line; do
  case "$line" in
    ""|"#"*) continue ;;
    *) expected+=("$line") ;;
  esac
done <"$manifest"

# The files under a root, relative to it.
walk() {
  (cd "$1" && find . \( -type f -o -type l \) -print | sed 's|^\./||' | LC_ALL=C sort)
}

# The files of a `pacman -Ql` style list as relative paths.
filter_list() {
  # shellcheck disable=SC2016
  local awk_prog='{ sub(/^[^ ]+ /, ""); sub(/^\//, ""); if ($0 !~ /\/$/ && $0 != "") print }'
  awk "$awk_prog" "$1" | LC_ALL=C sort
}

if [ "$mode" = update ]; then
  root="$(cd "$root" && pwd)"
  {
    sed -n '/^#/p' "$manifest"
    walk "$root"
  } >"$manifest.tmp"
  mv "$manifest.tmp" "$manifest"
  echo "manifest: rewritten from $root"
  exit 0
fi

findings=0
if [ "$mode" = root ]; then
  root="$(cd "$root" && pwd)"
  actual="$(walk "$root")"
  if [ -d "$root/usr/share/dettivo/models" ]; then
    echo "manifest: usr/share/dettivo/models exists; models never ship in the package" >&2
    findings=$((findings + 1))
  fi
else
  actual="$(filter_list "$list")"
  if grep -q '/usr/share/dettivo/models' "$list"; then
    echo "manifest: usr/share/dettivo/models is in the package; models never ship" >&2
    findings=$((findings + 1))
  fi
fi
wanted="$(printf '%s\n' "${expected[@]}" | LC_ALL=C sort)"

while IFS= read -r path; do
  [ -n "$path" ] || continue
  echo "manifest: missing $path" >&2
  findings=$((findings + 1))
done < <(LC_ALL=C comm -23 <(echo "$wanted") <(echo "$actual"))
while IFS= read -r path; do
  [ -n "$path" ] || continue
  echo "manifest: extra $path (add it to $manifest or stop installing it)" >&2
  findings=$((findings + 1))
done < <(LC_ALL=C comm -13 <(echo "$wanted") <(echo "$actual"))

if [ "$findings" -ne 0 ]; then
  echo "manifest: $findings finding(s) against $manifest" >&2
  exit 1
fi
echo "manifest: ok (${#expected[@]} files)"

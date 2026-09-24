#!/usr/bin/env bash
# Select the current recipe's application archive, excluding debug and stale builds.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
name="${1:?usage: select-package.sh <recipe-name> <package-dir>}"
out="${2:?usage: select-package.sh <recipe-name> <package-dir>}"
case "$name" in dettivo|dettivo-bin|dettivo-engines-cuda) ;; *) echo "select-package: unknown recipe $name" >&2; exit 2 ;; esac
recipe="$root/packaging/aur/$name/PKGBUILD"
version="$(sed -n -E 's/^pkgver=(.*)/\1/p' "$recipe")"
release="$(sed -n -E 's/^pkgrel=(.*)/\1/p' "$recipe")"
package="$out/$name-$version-$release-x86_64.pkg.tar.zst"
[ -f "$package" ] || { echo "select-package: current application package $package is missing" >&2; exit 1; }
printf '%s\n' "$package"

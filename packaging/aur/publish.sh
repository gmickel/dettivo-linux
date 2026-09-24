#!/usr/bin/env bash
# Push the two recipes to the AUR for a published release. The distribute
# workflow runs it after every release (ADR 0064); a maintainer can run it
# by hand with the same key to repair a release.
#
# Usage: packaging/aur/publish.sh <version> [--dry-run]
#
# For dettivo-bin and dettivo in turn: clone the AUR repository over SSH,
# copy the recipe and its install script, set pkgver and pkgrel=1, pin the
# source sums with updpkgsums (which downloads the release tarball, its
# SHA256SUMS and the source archive from GitHub), regenerate .SRCINFO,
# commit and push. `--dry-run` stops before the push and leaves the clones
# under build/aur-publish for inspection. Needs an SSH key registered with
# the AUR account and DETTIVO_AUR_PUBLISH=1 in the environment as the gate.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="${1:?usage: publish.sh <version> [--dry-run]}"
version="${version#v}"
dry_run=0
[ "${2:-}" = "--dry-run" ] && dry_run=1

if [ "${DETTIVO_AUR_PUBLISH:-0}" != 1 ] && [ "$dry_run" = 0 ]; then
  echo "publish: set DETTIVO_AUR_PUBLISH=1 to push, or pass --dry-run" >&2
  exit 2
fi
for tool in git updpkgsums makepkg; do
  command -v "$tool" >/dev/null || { echo "publish: $tool is missing (pacman -S pacman-contrib)" >&2; exit 2; }
done

work="$root/build/aur-publish"
rm -rf "$work"
mkdir -p "$work"
for name in dettivo-bin dettivo; do
  clone="$work/$name"
  git clone -q "ssh://aur@aur.archlinux.org/$name.git" "$clone"
  cp "$root/packaging/aur/$name/PKGBUILD" "$root/packaging/aur/$name/$name.install" "$clone/"
  sed -i -E "s/^pkgver=.*/pkgver=$version/; s/^pkgrel=.*/pkgrel=1/" "$clone/PKGBUILD"
  (cd "$clone" && updpkgsums && makepkg --printsrcinfo >.SRCINFO)
  git -C "$clone" add PKGBUILD .SRCINFO "$name.install"
  if git -C "$clone" diff --cached --quiet; then
    echo "publish: $name is already at $version on the AUR"
    continue
  fi
  git -C "$clone" commit -q -m "Update to $version"
  if [ "$dry_run" = 1 ]; then
    echo "publish: $name prepared under $clone (dry run, not pushed)"
  else
    git -C "$clone" push -q origin master
    echo "publish: $name $version pushed to the AUR"
  fi
done

#!/usr/bin/env bash
# Build one AUR recipe with makepkg from local sources, then run namcap on
# the recipe and the package (ADR 0034).
#
# Usage: scripts/packaging/build-package.sh bin <dist-dir> [out-dir]
#        scripts/packaging/build-package.sh cuda <dist-dir> [out-dir]
#        scripts/packaging/build-package.sh source [out-dir]
#
# `bin` builds packaging/aur/dettivo-bin from the tarball and SHA256SUMS
# `scripts/package.sh` wrote under <dist-dir>; `source` builds
# packaging/aur/dettivo from an archive of the checked-out tree. Either
# way the recipe is copied to build/aur/<name>/ with the local source
# dropped beside it, so makepkg downloads nothing and the pinned sums are
# skipped (the release pins them; here the sources are the ones just
# built). The package lands in <out-dir> (default build/aur) and its path
# is the last line printed. Run as root (the CI container) the build drops
# to a `builder` user, as makepkg requires; the runtime dependencies are
# not checked here (`--nodeps`), the install test proves them on a clean
# root. namcap errors fail the build; warnings are printed.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
mode="${1:?usage: build-package.sh bin <dist-dir> [out-dir] | source [out-dir]}"
version="$(grep -m1 -E '^version = ' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
tree="dettivo-${version}-linux-x86_64"

case "$mode" in
  bin|cuda)
    dist="$(cd "${2:?usage: build-package.sh bin <dist-dir>}" && pwd)"
    out="${3:-build/aur}"
    name=dettivo-bin
    sums=SHA256SUMS
    if [ "$mode" = cuda ]; then
      name=dettivo-engines-cuda
      tree="dettivo-engines-cuda-${version}-linux-x86_64"
      sums=SHA256SUMS-cuda
    fi
    ;;
  source)
    out="${2:-build/aur}"
    name=dettivo
    ;;
  *) echo "build-package: mode must be bin, cuda or source, not $mode" >&2; exit 2 ;;
esac
mkdir -p "$out"
out="$(cd "$out" && pwd)"
work="$root/build/aur/$name"
rm -rf "$work"
mkdir -p "$work"
cp "packaging/aur/$name/PKGBUILD" "$work/"
if [ -f "packaging/aur/$name/$name.install" ]; then
  cp "packaging/aur/$name/$name.install" "$work/"
fi

if [ "$mode" != source ]; then
  [ -f "$dist/$tree.tar.zst" ] || { echo "build-package: $dist/$tree.tar.zst is missing; run just package" >&2; exit 2; }
  cp "$dist/$tree.tar.zst" "$work/"
  cp "$dist/$sums" "$work/$tree.SHA256SUMS"
else
  git -C "$root" archive --format=tar.gz --prefix="dettivo-$version/" -o "$work/dettivo-$version.tar.gz" HEAD
fi

# The version the recipe builds must be the tree's.
pkgver="$(sed -n -E 's/^pkgver=(.*)$/\1/p' "$work/PKGBUILD")"
if [ "$pkgver" != "$version" ]; then
  echo "build-package: packaging/aur/$name/PKGBUILD says pkgver=$pkgver but Cargo.toml says $version" >&2
  exit 1
fi

makepkg_env=(PKGDEST="$out" SRCDEST="$work" BUILDDIR="$work/build")
if [ "$(id -u)" = 0 ]; then
  id builder >/dev/null 2>&1 || useradd -m -s /bin/bash builder
  chown -R builder "$work" "$out"
  # The builder needs the cargo cache the CI restores for root, and the
  # network for parakeet.cpp's pinned archives; cargo's home moves with it.
  if [ "$mode" = source ]; then
    cargo_home="$work/cargo-home"
    mkdir -p "$cargo_home"
    [ -d "$HOME/.cargo/registry" ] && cp -a "$HOME/.cargo/registry" "$cargo_home/"
    chown -R builder "$cargo_home"
    makepkg_env+=(CARGO_HOME="$cargo_home")
  fi
  runuser -u builder -- env "${makepkg_env[@]}" bash -c "cd '$work' && makepkg --nodeps --skipchecksums --noconfirm --force"
else
  (cd "$work" && env "${makepkg_env[@]}" makepkg --nodeps --skipchecksums --noconfirm --force)
fi
pkg="$(scripts/packaging/select-package.sh "$name" "$out")"
scripts/packaging/check-namcap.sh "$work/PKGBUILD" "$pkg"
echo "$pkg"

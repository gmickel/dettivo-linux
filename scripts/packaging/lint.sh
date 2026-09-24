#!/usr/bin/env bash
# The packaging lint `just lint` runs: the scripts are shellcheck-clean
# when shellcheck is installed, the manifest check and the release
# decision and receipt prove themselves, every
# PKGBUILD carries Cargo.toml's version with a `.SRCINFO` that matches
# `makepkg --printsrcinfo`, every Qt module the binaries link has its
# package in both depends lists, the two install scripts are one text, the
# desktop entry validates when desktop-file-utils is installed, and the
# changelog carries the current version's section.
#
# Usage: scripts/packaging/lint.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
findings=0
report() { echo "packaging: $1" >&2; findings=$((findings + 1)); }

scripts=(scripts/package.sh scripts/packaging/*.sh packaging/aur/publish.sh)
if command -v shellcheck >/dev/null; then
  shellcheck -x "${scripts[@]}" || report "shellcheck found the issues above"
else
  echo "packaging: shellcheck is not installed, the scripts are not linted (pacman -S shellcheck)"
fi

scripts/packaging/test-install-receipt.sh || report "install receipt regression failed"
scripts/packaging/test-namcap.sh || report "namcap failure regression failed"

scripts/packaging/test-installed-revision.sh || report "installed revision regression failed"

scripts/packaging/test-install-test-models.sh || report "install-test --models path regression failed"
scripts/packaging/test-install-test-session.sh >/dev/null || report "scripts/packaging/test-install-test-session.sh failed"

scripts/packaging/test-check-manifest.sh >/dev/null || report "scripts/packaging/test-check-manifest.sh failed"
scripts/packaging/test-package-cuda.sh >/dev/null || report "scripts/packaging/test-package-cuda.sh failed"

scripts/packaging/test-release-scripts.sh >/dev/null || report "scripts/packaging/test-release-scripts.sh failed"

version="$(grep -m1 -E '^version = ' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
for name in dettivo-bin dettivo dettivo-engines-cuda; do
  dir="packaging/aur/$name"
  pkgver="$(sed -n -E 's/^pkgver=(.*)$/\1/p' "$dir/PKGBUILD")"
  [ "$pkgver" = "$version" ] || report "$dir/PKGBUILD says pkgver=$pkgver, Cargo.toml says $version"
  if command -v makepkg >/dev/null; then
    want="$(cd "$dir" && makepkg --printsrcinfo 2>/dev/null)" || { report "$dir: makepkg --printsrcinfo failed"; continue; }
    if ! diff -u <(echo "$want") "$dir/.SRCINFO" >/dev/null; then
      report "$dir/.SRCINFO is out of step; run (cd $dir && makepkg --printsrcinfo > .SRCINFO)"
    fi
  fi
done
# Every Qt module a shipped binary links has its package in both depends
# lists; the test-only modules link into test binaries the package never
# installs. A module this table does not know is a finding too.
qt_package_for() {
  case "$1" in
    Core|Gui|Network|DBus|Widgets|Concurrent|Sql|Xml|OpenGL) echo qt6-base ;;
    Qml|Quick|QuickControls2|QuickLayouts|QuickTemplates2) echo qt6-declarative ;;
    Svg) echo qt6-svg ;;
    Multimedia) echo qt6-multimedia ;;
    WaylandClient|WaylandCompositor) echo qt6-wayland ;;
    Test|QuickTest) echo "" ;;
    *) echo "?" ;;
  esac
}
while IFS= read -r module; do
  pkg="$(qt_package_for "$module")"
  [ -n "$pkg" ] || continue
  if [ "$pkg" = "?" ]; then
    report "qt/ links Qt6::$module, which scripts/packaging/lint.sh cannot map to a package; add it to qt_package_for"
    continue
  fi
  for name in dettivo-bin dettivo; do
    if ! sed -n '/^depends=(/,/^)/p' "packaging/aur/$name/PKGBUILD" | grep -q "'$pkg'"; then
      report "packaging/aur/$name/PKGBUILD depends lacks '$pkg', which qt/ links as Qt6::$module"
    fi
  done
done < <(git ls-files 'qt/*CMakeLists.txt' | xargs grep -hoE 'Qt6::[A-Za-z0-9]+' | sed 's/Qt6:://' | sort -u)

if ! cmp -s packaging/aur/dettivo-bin/dettivo-bin.install packaging/aur/dettivo/dettivo.install; then
  report "the two .install scripts differ; keep them one text"
fi

if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate qt/apps/dettivo-app/dettivo.desktop || report "dettivo.desktop does not validate"
fi

scripts/packaging/release-notes.sh "$version" >/dev/null || report "CHANGELOG.md has no usable section for $version"

if [ "$findings" -ne 0 ]; then
  echo "packaging: $findings finding(s)" >&2
  exit 1
fi
echo "packaging: ok"

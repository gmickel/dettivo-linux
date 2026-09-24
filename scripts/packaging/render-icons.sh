#!/usr/bin/env bash
# Render the six-bar mark (qt/qml/Dettivo/icons/dettivo.svg) to the PNG sizes
# the package installs under /usr/share/icons/hicolor/<n>x<n>/apps/dettivo.png.
#
# Usage: scripts/packaging/render-icons.sh
#
# The PNGs are checked in under packaging/icons/ so the package build needs no
# renderer; run this after a change to the mark and commit the result. Needs
# rsvg-convert (pacman -S librsvg).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
command -v rsvg-convert >/dev/null || { echo "render-icons: rsvg-convert is missing (pacman -S librsvg)" >&2; exit 2; }
for size in 16 32 64 128; do
  rsvg-convert -w "$size" -h "$size" "$root/qt/qml/Dettivo/icons/dettivo.svg" -o "$root/packaging/icons/dettivo-${size}.png"
  echo "render-icons: packaging/icons/dettivo-${size}.png"
done

#!/usr/bin/env bash
# Assemble the release tree and tarball the AUR packages repackage (ADR 0034).
#
# Usage: scripts/package.sh [--cuda] [dist-dir]
# --cuda builds only the optional diarization drop-in, in a separate tree.
#
# Produces <dist-dir>/dettivo-<version>-linux-x86_64/ laid out as it lands on
# the root filesystem (usr/bin, usr/lib/dettivo/engines, usr/lib/dettivo/qml,
# the systemd user units, the desktop entry, the icons, the shell completions,
# the licence and NOTICE.md), then the .tar.zst beside it with its SHA256SUMS.
# The tree carries no models and no telemetry. Every path is checked against
# packaging/manifest.txt before the tarball is written; a missing or extra
# path fails naming it.
#
# The three ggml engines are built with the Vulkan backend and fall back to
# the CPU at run time; the diarization engine runs on the CPU and ships the
# sherpa-onnx and ONNX Runtime libraries beside itself; DETTIVO_PACKAGE_CPU_ONLY=1 is the explicit opt-out for
# a machine without the Vulkan headers (never the default, never used for a
# release). DETTIVO_GIT_SHA names the commit `dettivo --version` prints when
# the tree is built from a tarball without .git.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cuda=0
if [ "${1:-}" = --cuda ]; then cuda=1; shift; fi
[ "$#" -le 1 ] || { echo 'usage: package.sh [--cuda] [dist-dir]' >&2; exit 2; }
dist="${1:-dist}"
version="$(grep -m1 -E '^version = ' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
name="dettivo-${version}-linux-x86_64"
[ "$cuda" = 0 ] || name="dettivo-engines-cuda-${version}-linux-x86_64"
mkdir -p "$dist"
dist="$(cd "$dist" && pwd)"
stage="$dist/$name"
build_messages="$(mktemp)"
trap 'rm -f "$build_messages"' EXIT

stage_sherpa() {
  local destination="$1"
  shift
  local lib_dir
  lib_dir="$(jq -er -s '[.[] | select(.reason == "build-script-executed" and (.package_id | test("[/#]sherpa-onnx-sys[@#]"))) | .out_dir] | unique | if length == 1 then .[0] + "/lib" else error("expected one sherpa-onnx-sys OUT_DIR") end' "$build_messages")"
  for library in "$@"; do
    install -m 755 "$lib_dir/$library" "$destination/$library"
  done
}

archive() {
  local sums="$1"
  tar --zstd -C "$dist" --owner=0 --group=0 --numeric-owner --sort=name -cf "$dist/$name.tar.zst" "$name"
  (cd "$dist" && sha256sum "$name.tar.zst" >"$sums")
  echo "package: $dist/$name.tar.zst"
}

if [ -z "${DETTIVO_GIT_SHA:-}" ] && git -C "$root" rev-parse --short=12 HEAD >/dev/null 2>&1; then
  DETTIVO_GIT_SHA="$(git -C "$root" rev-parse --short=12 HEAD)"
  export DETTIVO_GIT_SHA
fi

if [ "$cuda" = 1 ]; then
  DETTIVO_PACKAGE=1 cargo build -p dettivo-engine-diarize --release --features cuda --message-format=json-render-diagnostics >"$build_messages"
  rm -rf "$stage"
  engines="$stage/usr/lib/dettivo/engines-cuda"
  mkdir -p "$engines"
  install -m 755 "${CARGO_TARGET_DIR:-target}/release/dettivo-engine-diarize" "$engines/"
  stage_sherpa "$engines" libsherpa-onnx-c-api.so libonnxruntime.so \
    libonnxruntime_providers_cuda.so libonnxruntime_providers_shared.so
  install -Dm644 LICENSE "$stage/usr/share/licenses/dettivo-engines-cuda/LICENSE"
  install -Dm644 NOTICE.md "$stage/usr/share/doc/dettivo-engines-cuda/NOTICE.md"
  scripts/packaging/check-manifest.sh --cuda "$stage"
  archive SHA256SUMS-cuda
  exit 0
fi

# One cargo argument: the comma-separated feature list is deliberate.
# shellcheck disable=SC2054
features=(--features dettivo-engine-whisper/vulkan,dettivo-engine-parakeet/vulkan,dettivo-engine-llm/vulkan)
if [ "${DETTIVO_PACKAGE_CPU_ONLY:-0}" = "1" ]; then
  echo "package: DETTIVO_PACKAGE_CPU_ONLY=1, the engines are built without the Vulkan backend (not a release build)" >&2
  features=()
fi

rm -rf "$stage"
mkdir -p "$stage/usr/bin" "$stage/usr/lib/dettivo/engines"

DETTIVO_PACKAGE=1 cargo build --workspace --release --bins "${features[@]}" --message-format=json-render-diagnostics >"$build_messages"
target="${CARGO_TARGET_DIR:-target}/release"
for bin in dettivod dettivo dettivo-mcp dettivo-qa; do
  install -m 755 "$target/$bin" "$stage/usr/bin/$bin"
done
for engine in whisper parakeet llm diarize; do
  install -m 755 "$target/dettivo-engine-$engine" "$stage/usr/lib/dettivo/engines/dettivo-engine-$engine"
done

# The diarization engine links sherpa-onnx's C API dynamically and finds it
# beside itself ($ORIGIN, ADR 0035); the two libraries come from where the
# sherpa-onnx-sys build script unpacked the pinned release.
stage_sherpa "$stage/usr/lib/dettivo/engines" libsherpa-onnx-c-api.so libonnxruntime.so

qt_build="${DETTIVO_QT_BUILD_DIR:-build/qt-release}"
cmake -S qt -B "$qt_build" -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr
cmake --build "$qt_build"
DESTDIR="$stage" cmake --install "$qt_build"

install -Dm644 LICENSE "$stage/usr/share/licenses/dettivo/LICENSE"
install -Dm644 LICENSE "$stage/usr/share/licenses/dettivo-bin/LICENSE"
install -Dm644 NOTICE.md "$stage/usr/share/doc/dettivo/NOTICE.md"
cargo run --quiet --release -p dettivo-core --example print_config \
  >"$stage/usr/share/doc/dettivo/config.example.toml"

# The Omarchy plugin folder, as `dettivo setup omarchy` copies it into the
# shell's plugin directory (docs/omarchy.md).
mkdir -p "$stage/usr/share/dettivo/omarchy"
cp -r omarchy/. "$stage/usr/share/dettivo/omarchy/"

install -Dm644 -t "$stage/usr/lib/systemd/user" \
  systemd/user/dettivod.socket systemd/user/dettivod.service systemd/user/dettivo-osd.service
install -Dm644 packaging/systemd/90-dettivo.preset "$stage/usr/lib/systemd/user-preset/90-dettivo.preset"

# Shell completions come from the binary that ships, so they never drift.
install -d "$stage/usr/share/bash-completion/completions" "$stage/usr/share/zsh/site-functions" \
  "$stage/usr/share/fish/vendor_completions.d"
"$target/dettivo" completions bash >"$stage/usr/share/bash-completion/completions/dettivo"
"$target/dettivo" completions zsh >"$stage/usr/share/zsh/site-functions/_dettivo"
"$target/dettivo" completions fish >"$stage/usr/share/fish/vendor_completions.d/dettivo.fish"

# The six-bar mark at the sizes launchers ask for (scripts/packaging/render-icons.sh).
for size in 16 32 64 128; do
  install -Dm644 "packaging/icons/dettivo-${size}.png" "$stage/usr/share/icons/hicolor/${size}x${size}/apps/dettivo.png"
done

scripts/packaging/check-manifest.sh "$stage"

archive SHA256SUMS

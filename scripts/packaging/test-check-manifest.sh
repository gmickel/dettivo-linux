#!/usr/bin/env bash
# The manifest check names a missing and an extra path, the Omarchy
# plugin's files among them, and accepts a tree and a `pacman -Ql` list
# that match packaging/manifest.txt exactly.
#
# Usage: scripts/packaging/test-check-manifest.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
check="$root/scripts/packaging/check-manifest.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

tree="$work/tree"
list="$work/list"
grep -v -E '^(#|$)' "$root/packaging/manifest.txt" | while IFS= read -r path; do
  mkdir -p "$tree/$(dirname "$path")"
  : >"$tree/$path"
  echo "dettivo /$path"
done >"$list"
{
  echo "dettivo /usr/share/dettivo/omarchy/"
  echo "dettivo /usr/bin/"
} >>"$list"

fail() { echo "test-check-manifest: $1" >&2; exit 1; }

"$check" "$tree" >/dev/null || fail "a complete tree must pass"
"$check" --files "$list" >/dev/null || fail "a complete pacman list must pass"

rm "$tree/usr/bin/dettivod"
out="$("$check" "$tree" 2>&1 || true)"
grep -q 'missing usr/bin/dettivod' <<<"$out" || fail "a missing file must be named: $out"
: >"$tree/usr/bin/dettivod"

: >"$tree/usr/bin/stray"
out="$("$check" "$tree" 2>&1 || true)"
grep -q 'extra usr/bin/stray' <<<"$out" || fail "an extra file must be named: $out"
rm "$tree/usr/bin/stray"

mkdir -p "$tree/usr/share/dettivo/models"
: >"$tree/usr/share/dettivo/models/weights.bin"
out="$("$check" "$tree" 2>&1 || true)"
grep -q 'models never ship' <<<"$out" || fail "a model in the tree must be refused: $out"
rm -r "$tree/usr/share/dettivo/models"

sed -i '/usr\/bin\/dettivo-qa$/d' "$list"
out="$("$check" --files "$list" 2>&1 || true)"
grep -q 'missing usr/bin/dettivo-qa' <<<"$out" || fail "a missing file in a list must be named: $out"
echo "dettivo /usr/bin/dettivo-qa" >>"$list"

# The plugin is checked file by file in both modes: a package without it,
# one without an entry point, and one with a stray file all fail.
plugin="usr/share/dettivo/omarchy"
grep -q "^$plugin/manifest.json\$" "$root/packaging/manifest.txt" || fail "the manifest must list the plugin's manifest.json"
grep -q "^$plugin/BarWidget.qml\$" "$root/packaging/manifest.txt" || fail "the manifest must list the plugin's BarWidget.qml"
sed -i "\|/$plugin/|d" "$list"
out="$("$check" --files "$list" 2>&1 || true)"
grep -q "missing $plugin/manifest.json" <<<"$out" || fail "a list without the plugin must fail: $out"
rm -r "${tree:?}/$plugin"
out="$("$check" "$tree" 2>&1 || true)"
grep -q "missing $plugin/BarWidget.qml" <<<"$out" || fail "a tree without the plugin must fail: $out"
mkdir -p "$tree/$plugin"
grep "^$plugin/" "$root/packaging/manifest.txt" | while IFS= read -r path; do : >"$tree/$path"; done
rm "$tree/$plugin/BarWidget.qml"
out="$("$check" "$tree" 2>&1 || true)"
grep -q "missing $plugin/BarWidget.qml" <<<"$out" || fail "a plugin without its widget must fail: $out"
: >"$tree/$plugin/BarWidget.qml"
: >"$tree/$plugin/Stray.qml"
out="$("$check" "$tree" 2>&1 || true)"
grep -q "extra $plugin/Stray.qml" <<<"$out" || fail "a stray plugin file must be named: $out"
rm "$tree/$plugin/Stray.qml"
"$check" "$tree" >/dev/null || fail "the restored tree must pass"

echo "test-check-manifest: ok"

cuda_tree="$work/cuda"
cuda_libs=usr/lib/dettivo/engines-cuda
mkdir -p "$cuda_tree/$cuda_libs"
for file in dettivo-engine-diarize libsherpa-onnx-c-api.so libonnxruntime.so libonnxruntime_providers_cuda.so libonnxruntime_providers_shared.so; do
  : >"$cuda_tree/$cuda_libs/$file"
done
mkdir -p "$cuda_tree/usr/share/licenses/dettivo-engines-cuda" "$cuda_tree/usr/share/doc/dettivo-engines-cuda"
: >"$cuda_tree/usr/share/licenses/dettivo-engines-cuda/LICENSE"
: >"$cuda_tree/usr/share/doc/dettivo-engines-cuda/NOTICE.md"
"$check" --cuda "$cuda_tree" >/dev/null || fail "the CUDA tree must use its own manifest"
rm "$cuda_tree/$cuda_libs/libonnxruntime_providers_cuda.so"
if out="$("$check" --cuda "$cuda_tree" 2>&1)"; then
  fail "a missing CUDA provider must fail"
fi
grep -q "missing $cuda_libs/libonnxruntime_providers_cuda.so" <<<"$out" || fail "the missing provider must be named: $out"
echo "test-check-manifest: CUDA ok"

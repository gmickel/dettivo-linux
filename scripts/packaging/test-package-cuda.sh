#!/usr/bin/env bash
# Exercise CUDA staging without rebuilding engines or installing a package.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/scripts/packaging" "$work/packaging" "$work/bin" "$work/dist" \
  "$work/target/release/build/sherpa-onnx-sys-selected/out/lib" \
  "$work/target/release/build/sherpa-onnx-sys-stale/out/lib"
cp "$root/scripts/package.sh" "$work/scripts/"
cp "$root/scripts/packaging/check-manifest.sh" "$work/scripts/packaging/"
cp "$root/packaging/manifest-cuda.txt" "$work/packaging/"
cp "$root/LICENSE" "$root/NOTICE.md" "$work/"
printf 'version = "0.1.0"\n' >"$work/Cargo.toml"
printf 'CPU checksum receipt\n' >"$work/dist/SHA256SUMS"
printf 'CUDA binary\n' >"$work/target/release/dettivo-engine-diarize"
for lib in libsherpa-onnx-c-api.so libonnxruntime.so libonnxruntime_providers_cuda.so libonnxruntime_providers_shared.so; do
  printf 'selected CUDA %s\n' "$lib" >"$work/target/release/build/sherpa-onnx-sys-selected/out/lib/$lib"
  printf 'stale CPU %s\n' "$lib" >"$work/target/release/build/sherpa-onnx-sys-stale/out/lib/$lib"
done
cat >"$work/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ "${DETTIVO_PACKAGE:-}" = 1 ]]
[[ "$*" = 'build -p dettivo-engine-diarize --release --features cuda --message-format=json-render-diagnostics' ]]
jq -cn --arg out "$PWD/target/release/build/sherpa-onnx-sys-selected/out" \
  '{reason:"build-script-executed",package_id:"path+file:///repo/crates/sherpa-onnx-sys#0.1.0",out_dir:$out}'
SH
chmod +x "$work/bin/cargo"
PATH="$work/bin:$PATH" CARGO_TARGET_DIR=target bash "$work/scripts/package.sh" --cuda "$work/dist" >/dev/null
tree="$work/dist/dettivo-engines-cuda-0.1.0-linux-x86_64"
"$root/scripts/packaging/check-manifest.sh" --cuda "$tree" >/dev/null
for lib in "$tree/usr/lib/dettivo/engines-cuda/"*.so; do
  grep -q '^selected CUDA ' "$lib"
done
grep -qx 'CPU checksum receipt' "$work/dist/SHA256SUMS"
(cd "$work/dist" && sha256sum --check --strict SHA256SUMS-cuda >/dev/null)
echo "test-package-cuda: ok"

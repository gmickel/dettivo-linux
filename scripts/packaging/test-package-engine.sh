#!/usr/bin/env bash
# Exercise the per-engine build (ADR 0079) without building an engine: each
# ggml engine gets a cargo build of its own with only its Vulkan feature,
# lands in <dist>/engines/ with Nemotron's libraries beside it, and --cuda
# with --engine is refused.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/scripts" "$work/bin" "$work/target/release/build/nemo-speech-cpp-sys-selected/out/lib"
cp "$root/scripts/package.sh" "$work/scripts/"
printf 'version = "0.1.0"\n' >"$work/Cargo.toml"
for lib in libnemo_speech_asr_c.so.1 libnemo_speech_asr.so; do
  printf 'selected %s\n' "$lib" >"$work/target/release/build/nemo-speech-cpp-sys-selected/out/lib/$lib"
done
cat >"$work/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ "${DETTIVO_PACKAGE:-}" = 1 ]]
[[ "$*" =~ ^build\ -p\ dettivo-engine-([a-z]+)\ --release\ --bins\ --features\ dettivo-engine-([a-z]+)/vulkan\ --message-format=json-render-diagnostics$ ]]
[[ "${BASH_REMATCH[1]}" = "${BASH_REMATCH[2]}" ]]
printf '#!/bin/sh\n[ "$1" = --help ]\n' >"target/release/dettivo-engine-${BASH_REMATCH[1]}"
chmod +x "target/release/dettivo-engine-${BASH_REMATCH[1]}"
echo "${BASH_REMATCH[1]}" >>cargo-calls
jq -cn --arg out "$PWD/target/release/build/nemo-speech-cpp-sys-selected/out" \
  '{reason:"build-script-executed",package_id:"path+file:///repo/crates/nemo-speech-cpp-sys#0.1.0",out_dir:$out}'
SH
chmod +x "$work/bin/cargo"
cd "$work"
for engine in whisper parakeet llm nemotron; do
  PATH="$work/bin:$PATH" CARGO_TARGET_DIR=target bash scripts/package.sh --engine "$engine" dist >/dev/null
done
[ "$(tr '\n' ' ' <cargo-calls)" = "whisper parakeet llm nemotron " ]
[ "$(cd dist/engines && LC_ALL=C ls | tr '\n' ' ')" = "dettivo-engine-llm dettivo-engine-nemotron dettivo-engine-parakeet dettivo-engine-whisper libnemo_speech_asr.so libnemo_speech_asr_c.so.1 " ]
grep -q '^selected ' dist/engines/libnemo_speech_asr.so
if PATH="$work/bin:$PATH" bash scripts/package.sh --engine diarize dist 2>/dev/null; then echo "test-package-engine: accepted a bad argument" >&2; exit 1; fi
if PATH="$work/bin:$PATH" bash scripts/package.sh --cuda --engine nemotron dist 2>/dev/null; then echo "test-package-engine: accepted a bad argument" >&2; exit 1; fi
if PATH="$work/bin:$PATH" bash scripts/package.sh --engine nemotron --cuda dist 2>/dev/null; then echo "test-package-engine: accepted a bad argument" >&2; exit 1; fi
echo "test-package-engine: ok"

#!/usr/bin/env bash
# The CUDA source patch applies once to its exact upstream context.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
patch_file="$root/crates/sherpa-onnx-sys/patches/cuda-conv-default.patch"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/sherpa-onnx/csrc"
source_file="$scratch/sherpa-onnx/csrc/session.cc"
cat > "$source_file" <<'SOURCE'
        } else {
          options.device_id = 0;
          // Default OrtCudnnConvAlgoSearchExhaustive is extremely slow
          options.cudnn_conv_algo_search = OrtCudnnConvAlgoSearchHeuristic;
          // set more options on need
        }
        sess_opts.AppendExecutionProvider_CUDA(options);
SOURCE
cp "$source_file" "$scratch/original"
apply() { patch --batch --forward --fuzz=0 --no-backup-if-mismatch -p1 -d "$scratch" -i "$patch_file"; }
apply > "$scratch/patch.log" 2>&1 || { cat "$scratch/patch.log" >&2; exit 1; }
grep -q 'cudnn_conv_algo_search = OrtCudnnConvAlgoSearchDefault;' "$source_file"
if apply > "$scratch/patch.log" 2>&1; then
  echo 'CUDA patch unexpectedly accepted an already patched source' >&2; exit 1
fi
sed 's/device_id = 0/device_id = 1/' "$scratch/original" > "$source_file"
if apply > "$scratch/patch.log" 2>&1; then
  echo 'CUDA patch unexpectedly accepted changed upstream context' >&2; exit 1
fi
echo 'sherpa CUDA patch: exact source, repeat refusal and drift refusal passed'

#!/usr/bin/env bash
# Convert a polish fine-tune from the macOS training pipeline into the
# sideload the Linux daemon loads (ADR 0032, docs/polish-models.md): a
# fused Hugging Face export becomes one Q4_K_M GGUF, a PEFT LoRA adapter
# becomes one GGUF adapter over its catalogue base, and a manifest in the
# macOS shape (`current.json`) points `[llm] polish_experiment` at it.
#
# The converters come from llama.cpp at the revision `llama-cpp-2`
# vendors (LLAMA_CPP_REV below), so the GGUF the engine loads is written
# by the same code that reads it. Every step is named; a converter or
# quantiser that exits non-zero stops the script naming the step. The
# conversion, the quantisation, the checksum and the verification run in
# a staging directory beside the sideload, and only a complete result
# replaces the previous model and manifest: a run that fails leaves the
# experiment the daemon loads exactly as it was.
#
# Usage:
#   convert-polish-finetune.sh --source <hf-dir> [--id <id>] [--name <display name>]
#                              [--quant Q4_K_M] [--out <experiments-dir>]
#                              [--manifest current] [--verify] [--dry-run]
#   convert-polish-finetune.sh --adapter <peft-dir> --base-hf <hf-dir-or-id>
#                              [--base qwen3-1.7b] [...]
#
#   --source     a fused export: config.json plus *.safetensors
#   --adapter    a PEFT adapter: adapter_config.json plus adapter_model.safetensors
#   --base-hf    the Hugging Face checkpoint the adapter was trained on (a
#                directory, or a hub id the converter downloads)
#   --base       the catalogue id Linux loads the adapter over (qwen3-1.7b)
#   --out        the experiments directory (default
#                $XDG_DATA_HOME/dettivo/models/polish-experiments)
#   --id         the experiment id (default qwen3-1.7b-private-best)
#   --name       the display name (default "Current 1.7B Experiment")
#   --quant      the quantisation of a fused model (default Q4_K_M)
#   --manifest   the manifest name to write (default current)
#   --llama-cpp  a llama.cpp checkout; cloned at LLAMA_CPP_REV under
#                $XDG_CACHE_HOME/dettivo/llama.cpp when absent
#   --python     the interpreter with torch, transformers and gguf
#                (default $XDG_CACHE_HOME/dettivo/polish-convert-venv/bin/python;
#                --setup-venv creates it with uv or python -m venv)
#   --setup-venv create the interpreter's venv when it is missing
#   --verify     load the result in dettivo-engine-llm's CLI mode and
#                print its answer to one prompt
#   --dry-run    check the toolchain and the paths, print the plan, convert nothing
set -uo pipefail

LLAMA_CPP_REV="e79e4bf660e19f2ad851e06c6913f7a8c5852621"
LLAMA_CPP_URL="https://github.com/ggml-org/llama.cpp.git"

source_dir=""
adapter_dir=""
base_hf=""
base_id="qwen3-1.7b"
out=""
id="qwen3-1.7b-private-best"
name="Current 1.7B Experiment"
quant="Q4_K_M"
manifest="current"
llama_cpp=""
python_bin=""
setup_venv=0
verify=0
dry_run=0

usage() {
  sed -n '2,/^set -uo/p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --source) source_dir="$2"; shift 2 ;;
    --adapter) adapter_dir="$2"; shift 2 ;;
    --base-hf) base_hf="$2"; shift 2 ;;
    --base) base_id="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --id) id="$2"; shift 2 ;;
    --name) name="$2"; shift 2 ;;
    --quant) quant="$2"; shift 2 ;;
    --manifest) manifest="$2"; shift 2 ;;
    --llama-cpp) llama_cpp="$2"; shift 2 ;;
    --python) python_bin="$2"; shift 2 ;;
    --setup-venv) setup_venv=1; shift ;;
    --verify) verify=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "convert-polish-finetune: unknown argument $1" >&2; usage >&2; exit 2 ;;
  esac
done

cache="${XDG_CACHE_HOME:-$HOME/.cache}/dettivo"
[ -n "$out" ] || out="${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models/polish-experiments"
[ -n "$llama_cpp" ] || llama_cpp="$cache/llama.cpp"
venv="$cache/polish-convert-venv"
[ -n "$python_bin" ] || python_bin="$venv/bin/python"

step=""
begin() { step="$1"; echo "convert-polish-finetune: $step"; }
fail() { echo "convert-polish-finetune: step $step failed: $1" >&2; exit 1; }
run() {
  "$@"
  local code=$?
  [ "$code" -eq 0 ] || fail "$1 exited $code"
}

if [ -n "$source_dir" ] && [ -n "$adapter_dir" ]; then
  echo "convert-polish-finetune: give --source or --adapter, not both" >&2; exit 2
fi
if [ -z "$source_dir" ] && [ -z "$adapter_dir" ]; then
  echo "convert-polish-finetune: --source <hf-dir> or --adapter <peft-dir> is required" >&2; usage >&2; exit 2
fi
if [ -n "$adapter_dir" ] && [ -z "$base_hf" ]; then
  echo "convert-polish-finetune: --adapter needs --base-hf <hf-dir-or-id>" >&2; exit 2
fi

# 1. The toolchain.
begin "toolchain"
for tool in git cmake sha256sum python3; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool is not installed"
done

# 2. The source.
begin "source"
if [ -n "$source_dir" ]; then
  [ -d "$source_dir" ] || fail "$source_dir is not a directory"
  [ -f "$source_dir/config.json" ] || fail "$source_dir has no config.json (not a Hugging Face export)"
  ls "$source_dir"/*.safetensors >/dev/null 2>&1 || fail "$source_dir has no *.safetensors"
  kind="fused"
else
  [ -d "$adapter_dir" ] || fail "$adapter_dir is not a directory"
  [ -f "$adapter_dir/adapter_config.json" ] || fail "$adapter_dir has no adapter_config.json (not a PEFT adapter)"
  [ -f "$adapter_dir/adapter_model.safetensors" ] || fail "$adapter_dir has no adapter_model.safetensors"
  kind="adapter"
fi

# 3. The llama.cpp checkout at the pinned revision.
begin "checkout"
checkout_ok=0
if [ -d "$llama_cpp/.git" ]; then
  have="$(git -C "$llama_cpp" rev-parse HEAD 2>/dev/null || true)"
  if [ "$have" = "$LLAMA_CPP_REV" ]; then
    checkout_ok=1
  elif [ "$dry_run" -eq 0 ]; then
    run git -C "$llama_cpp" fetch -q origin "$LLAMA_CPP_REV"
    run git -C "$llama_cpp" checkout -q "$LLAMA_CPP_REV"
    checkout_ok=1
  fi
elif [ "$dry_run" -eq 0 ]; then
  mkdir -p "$(dirname "$llama_cpp")"
  run git clone -q --filter=blob:none "$LLAMA_CPP_URL" "$llama_cpp"
  run git -C "$llama_cpp" checkout -q "$LLAMA_CPP_REV"
  checkout_ok=1
fi
if [ "$checkout_ok" -eq 1 ]; then
  [ -f "$llama_cpp/convert_hf_to_gguf.py" ] || fail "$llama_cpp has no convert_hf_to_gguf.py"
  [ -f "$llama_cpp/convert_lora_to_gguf.py" ] || fail "$llama_cpp has no convert_lora_to_gguf.py"
  [ -d "$llama_cpp/gguf-py" ] || fail "$llama_cpp has no gguf-py"
fi

# 4. The interpreter with the converters' dependencies.
begin "python"
if [ ! -x "$python_bin" ] && [ "$setup_venv" -eq 1 ] && [ "$dry_run" -eq 0 ]; then
  [ "$checkout_ok" -eq 1 ] || fail "the venv needs the checkout for gguf-py"
  if command -v uv >/dev/null 2>&1; then
    run uv venv -q "$venv"
    VIRTUAL_ENV="$venv" run uv pip install -q --python "$venv/bin/python" torch --index-url https://download.pytorch.org/whl/cpu
    VIRTUAL_ENV="$venv" run uv pip install -q --python "$venv/bin/python" numpy transformers safetensors sentencepiece -e "$llama_cpp/gguf-py"
  else
    run python3 -m venv "$venv"
    run "$venv/bin/pip" install -q torch --index-url https://download.pytorch.org/whl/cpu
    run "$venv/bin/pip" install -q numpy transformers safetensors sentencepiece -e "$llama_cpp/gguf-py"
  fi
fi
python_ok=0
if [ -x "$python_bin" ]; then
  if "$python_bin" -c 'import torch, transformers, gguf' >/dev/null 2>&1; then
    python_ok=1
  elif [ "$dry_run" -eq 0 ]; then
    fail "$python_bin lacks torch, transformers or gguf (pass --setup-venv, or --python <interpreter>)"
  fi
elif [ "$dry_run" -eq 0 ]; then
  fail "$python_bin is not an interpreter (pass --setup-venv, or --python <interpreter>)"
fi

model_dir="$out/$id"
manifest_file="$out/$manifest.json"
if [ "$kind" = "fused" ]; then
  final="$model_dir/$id-$quant.gguf"
else
  final="$model_dir/$id-adapter.gguf"
fi

if [ "$dry_run" -eq 1 ]; then
  cat <<PLAN
convert-polish-finetune: dry run
  kind        $kind
  source      ${source_dir:-$adapter_dir}
  llama.cpp   $llama_cpp at $LLAMA_CPP_REV ($([ "$checkout_ok" -eq 1 ] && echo present || echo "absent; the run clones it"))
  python      $python_bin ($([ "$python_ok" -eq 1 ] && echo "torch, transformers and gguf import" || echo "absent; the run needs --setup-venv or --python"))
  output      $final
  manifest    $manifest_file -> relativeModelPath $id$([ "$kind" = "adapter" ] && echo ", format gguf-lora, baseModel $base_id")
convert-polish-finetune: dry run ok
PLAN
  exit 0
fi

# 5. The quantiser (a fused model only).
if [ "$kind" = "fused" ]; then
  begin "quantizer"
  quantize="$llama_cpp/build/bin/llama-quantize"
  if [ ! -x "$quantize" ]; then
    run cmake -S "$llama_cpp" -B "$llama_cpp/build" -DCMAKE_BUILD_TYPE=Release \
      -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=OFF \
      -DLLAMA_BUILD_TOOLS=ON -DLLAMA_CURL=OFF
    run cmake --build "$llama_cpp/build" --target llama-quantize
  fi
  [ -x "$quantize" ] || fail "$quantize was not built"
fi

# The staging directory: everything up to the verification lands here,
# and the previous model and manifest stay in place until the new one
# is complete. A failed step leaves them untouched.
mkdir -p "$model_dir" || fail "cannot create $model_dir"
staging="$model_dir/.staging.$$"
mkdir -p "$staging" || fail "cannot create $staging"
trap 'rm -rf "$staging"' EXIT
staged="$staging/$(basename "$final")"

# 6. The conversion.
begin "convert"
if [ "$kind" = "fused" ]; then
  f16="$staging/$id-f16.gguf"
  run "$python_bin" "$llama_cpp/convert_hf_to_gguf.py" "$source_dir" --outtype f16 --outfile "$f16"
  # 7. The quantisation.
  begin "quantize"
  run "$quantize" "$f16" "$staged" "$quant"
  rm -f "$f16"
else
  base_args=()
  if [ -d "$base_hf" ]; then
    base_args=(--base "$base_hf")
  else
    base_args=(--base-model-id "$base_hf")
  fi
  run "$python_bin" "$llama_cpp/convert_lora_to_gguf.py" "${base_args[@]}" --outfile "$staged" "$adapter_dir"
fi
[ -f "$staged" ] || fail "$staged was not written"

# 8. The checksum.
begin "checksum"
(cd "$staging" && sha256sum "$(basename "$staged")" > "$(basename "$staged").sha256") || fail "sha256sum"

# 9. The engine loads it, before anything is published.
if [ "$verify" -eq 1 ]; then
  begin "verify"
  engine="$(command -v dettivo-engine-llm || true)"
  here="$(cd "$(dirname "$0")/../.." && pwd)"
  [ -n "$engine" ] || engine="$here/target/debug/dettivo-engine-llm"
  [ -x "$engine" ] || fail "dettivo-engine-llm is not built (cargo build -p dettivo-engine-llm)"
  if [ "$kind" = "fused" ]; then
    run "$engine" --prompt "um so i think we should ship this thing tomorrow" --system "Rewrite the dictated text as clean prose. Answer with the text only." --max-tokens 48 --cpu --model "$staged"
  else
    base_file="$(ls "${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models/llm/$base_id"/*.gguf 2>/dev/null | head -1)"
    [ -n "$base_file" ] || fail "the base llm/$base_id is not downloaded (dettivo llm download --model $base_id)"
    run "$engine" --prompt "um so i think we should ship this thing tomorrow" --system "Rewrite the dictated text as clean prose. Answer with the text only." --max-tokens 48 --cpu --model "$base_file" --lora "$staged"
  fi
fi

# 10. Publication: the previous GGUF gives way to the complete one (a
# sideload directory holds one GGUF), then the manifest names it.
begin "publish"
for old in "$model_dir"/*.gguf; do
  [ -e "$old" ] || continue
  echo "convert-polish-finetune: replacing the previous $old"
  rm -f "$old" "$old.sha256"
done
mv "$staged" "$final" || fail "cannot move $staged into place"
mv "$staged.sha256" "$final.sha256" || fail "cannot move the checksum into place"
rmdir "$staging" 2>/dev/null || true

# 11. The manifest.
begin "manifest"
if [ "$kind" = "fused" ]; then
  printf '{\n  "id": "%s",\n  "displayName": "%s",\n  "relativeModelPath": "%s"\n}\n' "$id" "$name" "$id" > "$manifest_file" || fail "cannot write $manifest_file"
else
  printf '{\n  "id": "%s",\n  "displayName": "%s",\n  "relativeModelPath": "%s",\n  "format": "gguf-lora",\n  "baseModel": "%s"\n}\n' "$id" "$name" "$id" "$base_id" > "$manifest_file" || fail "cannot write $manifest_file"
fi

cat <<DONE
convert-polish-finetune: done
  model       $final
  checksum    $(cut -d' ' -f1 "$final.sha256")
  manifest    $manifest_file
  next        dettivo llm experiment use $manifest   (or [llm] polish_experiment = "$manifest")
DONE

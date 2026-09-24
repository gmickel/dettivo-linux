#!/usr/bin/env bash
# Fetch the smallest catalogue language model, Qwen3 1.7B Q4_K_M, into the
# model layout the daemon reads (ADR 0005, ADR 0026) with its catalogue
# checksum, for the model-backed LLM tests (`crates/dettivo-engine-llm`,
# `crates/dettivod/tests/llm_local.rs`). Idempotent; CI does not fetch it,
# so those tests skip there by name.
#
# Usage: scripts/models/fetch-llm-test-model.sh [models-dir]
#   default models-dir: ${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models
set -euo pipefail

models="${1:-${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models}"
dir="${models}/llm/qwen3-1.7b"
url="https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf"
sha="b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897"
bytes=1107409472
out="${dir}/Qwen3-1.7B-Q4_K_M.gguf"

if [[ -f "${out}" ]]; then
    have="$(sha256sum "${out}" | awk '{print $1}')"
    if [[ "${have}" == "${sha}" ]]; then echo "fetch-llm-test-model: ${out} present"; exit 0; fi
    echo "fetch-llm-test-model: ${out} checksum mismatch, refetching" >&2
fi
mkdir -p "${dir}"
curl -fsSL -C - -o "${out}.part" "${url}"
have="$(sha256sum "${out}.part" | awk '{print $1}')"
if [[ "${have}" != "${sha}" ]]; then
    echo "fetch-llm-test-model: ${url} hashed ${have}, expected ${sha}; left as ${out}.part" >&2
    exit 1
fi
mv "${out}.part" "${out}"
printf '{"provider":"llm","id":"qwen3-1.7b","sha256":"%s","size_bytes":%s,"downloaded_at":%s,"catalogue_version":1,"verified":true}\n' \
    "${sha}" "${bytes}" "$(date +%s)" > "${dir}/manifest.json"
echo "fetch-llm-test-model: fetched ${out}"

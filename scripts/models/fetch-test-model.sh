#!/usr/bin/env bash
# Fetch the small Whisper model, the Parakeet TDT 0.6B v2 model and the
# speech fixture the engine tests use, into the model layout the daemon
# reads (ADR 0005), with pinned checksums. Idempotent; CI caches the
# directory.
#
# Usage: scripts/models/fetch-test-model.sh [models-dir]
#   default models-dir: ${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models
set -euo pipefail

models="${1:-${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models}"
model_dir="${models}/whisper/tiny.en"
model_url="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin"
model_sha="921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f"
parakeet_dir="${models}/parakeet/parakeet-v2"
parakeet_url="https://huggingface.co/mudler/parakeet-cpp-gguf/resolve/main/tdt-0.6b-v2-q8_0.gguf"
parakeet_sha="2027e2e1a4dc60ccdd8558f93b15e7c0db4ef8895b4e82e889f3a6275d8119c6"
parakeet_bytes=903835936
fixture_dir="${models}/fixtures"
fixture_url="https://github.com/ggml-org/whisper.cpp/raw/master/samples/jfk.wav"
fixture_sha="59dfb9a4acb36fe2a2affc14bacbee2920ff435cb13cc314a08c13f66ba7860e"

fetch() {
    local url="$1" out="$2" want="$3"
    if [[ -f "${out}" ]]; then
        local have; have="$(sha256sum "${out}" | awk '{print $1}')"
        if [[ "${have}" == "${want}" ]]; then echo "fetch-test-model: ${out} present"; return 0; fi
        echo "fetch-test-model: ${out} checksum mismatch, refetching" >&2
    fi
    mkdir -p "$(dirname "${out}")"
    curl -fsSL -o "${out}.part" "${url}"
    local have; have="$(sha256sum "${out}.part" | awk '{print $1}')"
    if [[ "${have}" != "${want}" ]]; then
        echo "fetch-test-model: ${url} hashed ${have}, expected ${want}; quarantined as ${out}.part" >&2
        return 1
    fi
    mv "${out}.part" "${out}"
    echo "fetch-test-model: fetched ${out}"
}

fetch "${model_url}" "${model_dir}/ggml-tiny.en.bin" "${model_sha}"
printf '{"provider":"whisper","id":"tiny.en","source":"%s","sha256":"%s","license":"MIT (OpenAI Whisper weights)"}\n' "${model_url}" "${model_sha}" > "${model_dir}/manifest.json"
fetch "${parakeet_url}" "${parakeet_dir}/tdt-0.6b-v2-q8_0.gguf" "${parakeet_sha}"
# The daemon's manifest shape (crates/dettivo-speech/src/models.rs), marked
# verified, so the catalogue reports the model ready without re-hashing.
# Idempotent: an existing manifest keeps its download time, a new one is
# stamped once, so a rerun never changes the directory.
if [[ -f "${parakeet_dir}/manifest.json" ]] && grep -q '"verified":true' "${parakeet_dir}/manifest.json"; then
    :
else
    now="$(date +%s)"
    printf '{"provider":"parakeet","id":"parakeet-v2","sha256":"%s","size_bytes":%s,"downloaded_at":%s,"catalogue_version":1,"verified":true}\n' "${parakeet_sha}" "${parakeet_bytes}" "${now}" > "${parakeet_dir}/manifest.json"
fi
fetch "${fixture_url}" "${fixture_dir}/jfk.wav" "${fixture_sha}"
cat > "${fixture_dir}/jfk.txt" <<'EOF'
And so my fellow Americans ask not what your country can do for you ask what you can do for your country
EOF
echo "fetch-test-model: ready under ${models}"

#!/usr/bin/env bash
# Fetch a speaker diarization model set into the model layout the daemon
# reads, under <models>/diarize/<model-id>/, with the pinned checksums the
# catalogue carries and a verified manifest: the sherpa-onnx sets (ADR 0035)
# are pyannote segmentation 3.0 (unpacked from the sherpa-onnx archive to
# segmentation.onnx) and 3D-Speaker ERes2Net (embedding.onnx); Nemotron 3
# Diarization (ADR 0073) is one GGUF. Idempotent; CI caches the directory.
#
# Usage: scripts/models/fetch-diarization-model.sh [models-dir] [diarization-en|diarization|nemotron-3-diarization]
#   default models-dir: ${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models
set -euo pipefail

models="${1:-${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models}"
model_id="${2:-diarization-en}"
case "${model_id}" in
    diarization-en)
        embedding_name="3dspeaker_speech_eres2net_sv_en_voxceleb_16k.onnx"
        embedding_sha="c59158379255ad66e161679cca6af8d52d51e389e3224ab7d7a7baae295c2db5"
        ;;
    diarization)
        embedding_name="3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx"
        embedding_sha="1a331345f04805badbb495c775a6ddffcdd1a732567d5ec8b3d5749e3c7a5e4b"
        ;;
    nemotron-3-diarization) embedding_name="" embedding_sha="" ;;
    *) echo "fetch-diarization-model: unknown model ${model_id}" >&2; exit 1 ;;
esac
dir="${models}/diarize/${model_id}"
archive_url="https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2"
archive_sha="24615ee884c897d9d2ba09bb4d30da6bb1b15e685065962db5b02e76e4996488"
archive_bytes=6958444
member="sherpa-onnx-pyannote-segmentation-3-0/model.onnx"
segmentation_sha="220ad67ca923bef2fa91f2390c786097bf305bceb5e261d4af67b38e938e1079"
embedding_url="https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/${embedding_name}"

sha() { sha256sum "$1" | awk '{print $1}'; }

fetch() {
    local url="$1" out="$2" want="$3"
    if [[ -f "${out}" ]] && [[ "$(sha "${out}")" == "${want}" ]]; then
        echo "fetch-diarization-model: ${out} present"
        return 0
    fi
    mkdir -p "$(dirname "${out}")"
    curl -fsSL --retry 3 -o "${out}.part" "${url}"
    local have; have="$(sha "${out}.part")"
    if [[ "${have}" != "${want}" ]]; then
        echo "fetch-diarization-model: ${url} hashed ${have}, expected ${want}; left as ${out}.part" >&2
        return 1
    fi
    mv "${out}.part" "${out}"
    echo "fetch-diarization-model: fetched ${out}"
}

# manifest <entry-sha256> <entry-size> <file>=<disk-sha256>...: the daemon's
# verified manifest (ModelStore::write_manifest, crates/dettivo-speech/src/
# models.rs), written once every file has hashed right this run. Each file
# carries its size and modification time, which readiness compares, so the
# catalogue reports the set ready without hashing it again. An existing
# manifest keeps its download time.
manifest() {
    python3 - "${dir}" "${model_id}" "$@" <<'PY'
import json, os, sys, time
d, model_id, sha, size, *files = sys.argv[1:]
path = os.path.join(d, "manifest.json")
try:
    with open(path) as f:
        downloaded_at = int(json.load(f)["downloaded_at"])
except (OSError, ValueError, KeyError, TypeError):
    downloaded_at = int(time.time())
verified = []
for spec in files:
    name, disk_sha = spec.split("=", 1)
    st = os.stat(os.path.join(d, name))
    verified.append({"file_name": name, "sha256": disk_sha, "size_bytes": st.st_size,
                     "modified_unix_nanos": st.st_mtime_ns})
manifest = {"provider": "diarize", "id": model_id, "sha256": sha, "size_bytes": int(size),
            "downloaded_at": downloaded_at, "catalogue_version": 1, "verified": True, "files": verified}
with open(path + ".part", "w") as f:
    f.write(json.dumps(manifest, indent=2) + "\n")
os.replace(path + ".part", path)
PY
    echo "fetch-diarization-model: ready under ${dir}"
}

mkdir -p "${dir}"
if [[ "${model_id}" == nemotron-3-diarization ]]; then
    gguf_sha="08456d9e22cd9a323c0364d98375f3746d6e68507ebb705cd46438c534c7a3a1"
    fetch "https://huggingface.co/nvidia/Nemotron-3-Diarization/resolve/f667ed73aee57d40cc39428eb768b4fd87a0a29e/Nemotron-3-Diarization.q8_0.gguf" \
        "${dir}/Nemotron-3-Diarization.q8_0.gguf" "${gguf_sha}"
    manifest "${gguf_sha}" 107012128 "Nemotron-3-Diarization.q8_0.gguf=${gguf_sha}"
    exit 0
fi
if [[ -f "${dir}/segmentation.onnx" ]] && [[ "$(sha "${dir}/segmentation.onnx")" == "${segmentation_sha}" ]]; then
    echo "fetch-diarization-model: ${dir}/segmentation.onnx present"
else
    fetch "${archive_url}" "${dir}/segmentation.tar.bz2" "${archive_sha}"
    scratch="$(mktemp -d "${dir}/.unpack.XXXXXX")"
    tar -xjf "${dir}/segmentation.tar.bz2" -C "${scratch}" "${member}"
    have="$(sha "${scratch}/${member}")"
    if [[ "${have}" != "${segmentation_sha}" ]]; then
        echo "fetch-diarization-model: ${member} hashed ${have}, expected ${segmentation_sha}" >&2
        exit 1
    fi
    mv "${scratch}/${member}" "${dir}/segmentation.onnx"
    rm -r "${scratch}" "${dir}/segmentation.tar.bz2"
    echo "fetch-diarization-model: unpacked ${dir}/segmentation.onnx"
fi
fetch "${embedding_url}" "${dir}/embedding.onnx" "${embedding_sha}"
manifest "${archive_sha}" "${archive_bytes}" "segmentation.onnx=${segmentation_sha}" "embedding.onnx=${embedding_sha}"

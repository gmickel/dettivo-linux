#!/usr/bin/env bash
# Fetch the speaker diarization model set (ADR 0035) into the model layout
# the daemon reads: pyannote segmentation 3.0 (unpacked from the sherpa-onnx
# archive to segmentation.onnx) and 3D-Speaker ERes2Net (embedding.onnx)
# under <models>/diarize/<model-id>/, with the pinned checksums the
# catalogue carries and a verified manifest. Idempotent; CI caches the
# directory.
#
# Usage: scripts/models/fetch-diarization-model.sh [models-dir] [diarization-en|diarization]
#   default models-dir: ${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models
set -euo pipefail

models="${1:-${XDG_DATA_HOME:-$HOME/.local/share}/dettivo/models}"
model_id="${2:-diarization-en}"
case "${model_id}" in
    diarization-en)
        embedding_name="3dspeaker_speech_eres2net_sv_en_voxceleb_16k.onnx"
        embedding_sha="c59158379255ad66e161679cca6af8d52d51e389e3224ab7d7a7baae295c2db5"
        embedding_bytes=26485263
        ;;
    diarization)
        embedding_name="3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx"
        embedding_sha="1a331345f04805badbb495c775a6ddffcdd1a732567d5ec8b3d5749e3c7a5e4b"
        embedding_bytes=39593761
        ;;
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

mkdir -p "${dir}"
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
# The daemon's manifest shape (crates/dettivo-speech/src/models.rs), marked
# verified so the catalogue reports the set ready without re-hashing; an
# existing verified manifest keeps its download time.
if [[ -f "${dir}/manifest.json" ]] && grep -q '"verified":true' "${dir}/manifest.json"; then
    :
else
    now="$(date +%s)"
    printf '{"provider":"diarize","id":"%s","sha256":"%s","size_bytes":%s,"downloaded_at":%s,"catalogue_version":1,"verified":true}\n' \
        "${model_id}" "${archive_sha}" "$((archive_bytes + embedding_bytes))" "${now}" > "${dir}/manifest.json"
fi
echo "fetch-diarization-model: ready under ${dir}"

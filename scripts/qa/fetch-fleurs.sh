#!/usr/bin/env bash
# Fetches the FLEURS test split for the languages the language benchmark
# scores (`dettivo-qa language-wer`, docs/qa.md) and prepares the first
# utterances of each as 16 kHz mono 16-bit WAV files (through ffmpeg)
# beside a manifest. FLEURS is Google's CC-BY-4.0 speech set
# (huggingface.co/datasets/google/fleurs); nothing of it enters the
# repository.
#
#   scripts/qa/fetch-fleurs.sh <dir> [utterances per language, default 100]
#
# <dir>/<code>/manifest.tsv lists `<wav>\t<reference>` with the set's
# normalised transcription as the reference.
set -euo pipefail

dir=${1:?usage: fetch-fleurs.sh <dir> [utterances]}
count=${2:-100}
base=https://huggingface.co/datasets/google/fleurs/resolve/main/data
# FLEURS config, then the language code the engines take.
languages=(en_us:en de_de:de fr_fr:fr es_419:es it_it:it nl_nl:nl pt_br:pt pl_pl:pl)

mkdir -p "$dir"
for pair in "${languages[@]}"; do
    config=${pair%%:*}
    code=${pair##*:}
    out="$dir/$code"
    if [[ -s "$out/manifest.tsv" ]] && (( $(wc -l <"$out/manifest.tsv") >= count )); then
        echo "fleurs: $code ready"
        continue
    fi
    mkdir -p "$out"
    echo "fleurs: fetching $config"
    curl -fsSL --retry 3 -o "$out/test.tsv" "$base/$config/test.tsv"
    [[ -s "$out/test.tar.gz" ]] || curl -fsSL --retry 3 -o "$out/test.tar.gz" "$base/$config/audio/test.tar.gz"
    python3 - "$out" "$count" <<'PY'
import csv, subprocess, sys, tarfile, wave
out, count = sys.argv[1], int(sys.argv[2])
rows = []
with open(f"{out}/test.tsv", newline="", encoding="utf-8") as f:
    for row in csv.reader(f, delimiter="\t", quoting=csv.QUOTE_NONE):
        rows.append((row[1], row[3]))
rows.sort()
wanted = dict(rows[:count])
written = []
with tarfile.open(f"{out}/test.tar.gz") as tar:
    for member in tar:
        name = member.name.rsplit("/", 1)[-1]
        if name not in wanted:
            continue
        # The set stores 32-bit float WAV; the engines take 16-bit PCM.
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-i", "pipe:0", "-ac", "1", "-ar", "16000",
             "-c:a", "pcm_s16le", f"{out}/{name}"],
            input=tar.extractfile(member).read(), check=True)
        with wave.open(f"{out}/{name}") as w:
            assert w.getframerate() == 16000 and w.getnchannels() == 1 and w.getsampwidth() == 2, name
        written.append(name)
with open(f"{out}/manifest.tsv", "w", encoding="utf-8") as m:
    for name in sorted(written):
        m.write(f"{name}\t{wanted[name]}\n")
print(f"fleurs: {out}: {len(written)} utterances")
PY
    rm -f "$out/test.tar.gz"
done

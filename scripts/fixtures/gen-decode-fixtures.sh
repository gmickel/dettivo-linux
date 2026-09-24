#!/usr/bin/env bash
# Generate the short decoding fixtures under crates/dettivo-audio/fixtures/decode/
# once, one file per import content type (FR-S6 plus the Linux additions),
# from the same one-second 440 Hz tone at amplitude 0.5. The lossless files
# are 22.05 kHz mono so the resampler runs; the lossy files are 44.1 kHz
# stereo so the downmix runs too. The files are checked in; rerun this only
# to regenerate them (needs ffmpeg with libmp3lame, aac and libvorbis).
#
# Usage: scripts/fixtures/gen-decode-fixtures.sh
set -euo pipefail

root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
out="$root/crates/dettivo-audio/fixtures/decode"
mkdir -p "$out"

wave="0.5*sin(440*2*PI*t)"
mono=(ffmpeg -y -loglevel error -f lavfi -i "aevalsrc=${wave}:s=22050:d=1")
stereo=(ffmpeg -y -loglevel error -f lavfi -i "aevalsrc=${wave}|${wave}:c=stereo:s=44100:d=1")

"${mono[@]}" -c:a pcm_s16le "$out/tone.wav"
"${mono[@]}" -c:a pcm_s16be "$out/tone.aiff"
"${mono[@]}" -c:a pcm_s16le -f caf "$out/tone.caf"
"${mono[@]}" -c:a flac "$out/tone.flac"
"${stereo[@]}" -c:a libmp3lame -b:a 64k "$out/tone.mp3"
"${stereo[@]}" -c:a aac -b:a 64k -movflags +faststart "$out/tone.m4a"
"${stereo[@]}" -c:a aac -b:a 64k -f adts "$out/tone.aac"
"${stereo[@]}" -c:a libvorbis -q:a 2 "$out/tone.ogg"

ls -l "$out"

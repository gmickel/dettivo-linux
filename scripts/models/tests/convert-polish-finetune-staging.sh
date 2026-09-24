#!/usr/bin/env bash
# Proves convert-polish-finetune.sh publishes a model only once it is
# complete (ops-and-record/F9, fn-43): with a fake llama.cpp, a fake
# interpreter and a fake quantiser under a temporary directory, a first
# conversion publishes a model, its checksum and the manifest; a
# conversion whose converter fails, and one whose quantiser fails, leave
# all three exactly as they were; a conversion that succeeds again
# replaces them. No network, no torch, no real checkpoint.
#
# Usage: scripts/models/tests/convert-polish-finetune-staging.sh
set -euo pipefail

here="$(cd "$(dirname "$0")/../../.." && pwd)"
script="$here/scripts/models/convert-polish-finetune.sh"
source_dir="$here/scripts/models/fixtures/hf-checkpoint"
rev="$(sed -n 's/^LLAMA_CPP_REV="\(.*\)"$/\1/p' "$script")"
[ -n "$rev" ] || { echo "staging test: LLAMA_CPP_REV not found in $script" >&2; exit 2; }

work="$(mktemp -d "${TMPDIR:-/tmp}/dtv-convert-XXXXXX")"
trap 'rm -rf "$work"' EXIT
fake="$work/llama.cpp"
bin="$work/bin"
out="$work/experiments"
mkdir -p "$fake/gguf-py" "$fake/build/bin" "$bin"
touch "$fake/convert_hf_to_gguf.py" "$fake/convert_lora_to_gguf.py"

# git answers the pinned revision for the fake checkout.
cat > "$bin/git" <<EOF
#!/usr/bin/env bash
case "\$*" in
  *"rev-parse HEAD"*) echo "$rev" ;;
  *) exit 0 ;;
esac
EOF
# The interpreter imports the converters' dependencies and writes the
# --outfile the converter is asked for, or fails when told to.
cat > "$bin/python" <<'EOF'
#!/usr/bin/env bash
if [ "${1:-}" = "-c" ]; then exit 0; fi
if [ "${FAKE_CONVERT_FAIL:-0}" = "1" ]; then echo "fake converter: refused" >&2; exit 1; fi
outfile=""
while [ $# -gt 0 ]; do
  if [ "$1" = "--outfile" ]; then outfile="$2"; shift 2; else shift; fi
done
[ -n "$outfile" ] || exit 2
printf 'converted %s\n' "$(date +%s%N)" > "$outfile"
EOF
# The quantiser copies its input to its output, or fails when told to.
cat > "$fake/build/bin/llama-quantize" <<'EOF'
#!/usr/bin/env bash
if [ "${FAKE_QUANTIZE_FAIL:-0}" = "1" ]; then echo "fake quantiser: refused" >&2; exit 1; fi
cp "$1" "$2"
EOF
chmod +x "$bin/git" "$bin/python" "$fake/build/bin/llama-quantize"

convert() {
  PATH="$bin:$PATH" "$script" --source "$source_dir" --out "$out" --id exp --name "Exp" \
    --llama-cpp "$fake" --python "$bin/python" "$@"
}
model="$out/exp/exp-Q4_K_M.gguf"
manifest="$out/current.json"

echo "staging test: first conversion publishes"
convert >/dev/null
[ -f "$model" ] || { echo "staging test: $model was not published" >&2; exit 1; }
[ -f "$model.sha256" ] || { echo "staging test: no checksum" >&2; exit 1; }
[ -f "$manifest" ] || { echo "staging test: no manifest" >&2; exit 1; }
first_model="$(cat "$model")"
first_sum="$(cat "$model.sha256")"
first_manifest="$(cat "$manifest")"

check_intact() {
  [ "$(cat "$model")" = "$first_model" ] || { echo "staging test: $1: the model changed" >&2; exit 1; }
  [ "$(cat "$model.sha256")" = "$first_sum" ] || { echo "staging test: $1: the checksum changed" >&2; exit 1; }
  [ "$(cat "$manifest")" = "$first_manifest" ] || { echo "staging test: $1: the manifest changed" >&2; exit 1; }
  if ls "$out/exp"/.staging* >/dev/null 2>&1; then echo "staging test: $1: a staging directory was left behind" >&2; exit 1; fi
}

echo "staging test: a failed converter keeps the previous model"
if FAKE_CONVERT_FAIL=1 convert >/dev/null 2>&1; then echo "staging test: the failed converter did not fail the script" >&2; exit 1; fi
check_intact "converter"

echo "staging test: a failed quantiser keeps the previous model"
if FAKE_QUANTIZE_FAIL=1 convert >/dev/null 2>&1; then echo "staging test: the failed quantiser did not fail the script" >&2; exit 1; fi
check_intact "quantiser"

echo "staging test: a conversion that succeeds replaces it"
convert >/dev/null
[ "$(cat "$model")" != "$first_model" ] || { echo "staging test: the second conversion did not replace the model" >&2; exit 1; }
[ "$(cut -d' ' -f1 "$model.sha256")" = "$(sha256sum "$model" | cut -d' ' -f1)" ] || { echo "staging test: the checksum does not match the model" >&2; exit 1; }
[ "$(ls "$out/exp"/*.gguf | wc -l)" -eq 1 ] || { echo "staging test: more than one GGUF in the sideload directory" >&2; exit 1; }
echo "staging test: ok"

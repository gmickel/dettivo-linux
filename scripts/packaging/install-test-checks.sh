#!/usr/bin/env bash
# The checks scripts/packaging/install-test.sh runs, one function each;
# sourced, never run. Every function ends in `record` or `run`, which
# append the step to install-test.json and count a failure. The variables
# below (package, prefix, bin, engines, qml, models, out, version,
# repo_root) are the caller's.
# shellcheck disable=SC2154

# The models directory as an absolute path. `check_daemon_dictation` links
# it under the daemon's own data directory, and a symlink to a relative
# path resolves beside the link, not beside the caller, so the workflow's
# `--models .ci-data/dettivo/models` used to leave the installed daemon
# without its models while the smokes found them from the working
# directory. A directory that does not exist is refused by name.
resolve_models_dir() {
  local dir="$1"
  if [ ! -d "$dir" ]; then
    echo "install-test: --models $dir is not a directory" >&2
    return 2
  fi
  (cd "$dir" && pwd)
}

# Install the package: pacman on the system, an unpack under the prefix.
check_install() {
  if [ -n "$prefix" ]; then
    run install "unpacked $(basename "$package") under $prefix" \
      bsdtar -xf "$package" -C "$prefix" --exclude .BUILDINFO --exclude .MTREE --exclude .PKGINFO --exclude .INSTALL
  elif [ "$(id -u)" = 0 ]; then
    run install "pacman -U $(basename "$package")" pacman -U --noconfirm "$package"
  else
    run install "sudo pacman -U $(basename "$package")" sudo pacman -U --noconfirm "$package"
  fi
}

# The file list is exactly packaging/manifest.txt, and no model ships.
check_files() {
  if [ -n "$prefix" ]; then
    run files "every path in packaging/manifest.txt, nothing else" \
      "$repo_root/scripts/packaging/check-manifest.sh" "$prefix"
  else
    pacman -Qlp "$package" >"$out/files.list"
    run files "pacman -Qlp matches packaging/manifest.txt" \
      "$repo_root/scripts/packaging/check-manifest.sh" --files "$out/files.list"
  fi
}

# The packaged icons (docs/design/checklist.md, C-23): every size under
# hicolor is byte for byte the checked-in render, and where the build tree
# has dettivo-visual-diff each is scored against the artboard's crop under
# docs/design/studio/baselines/icon/.
check_icons() {
  local size installed reference crop tool="$repo_root/build/qt/tools/visual-diff/dettivo-visual-diff" code=0 detail=""
  for size in 16 32 64 128; do
    installed="$prefix/usr/share/icons/hicolor/${size}x${size}/apps/dettivo.png"
    reference="$repo_root/packaging/icons/dettivo-${size}.png"
    if [ ! -f "$installed" ]; then
      record icons fail 1 "$installed is not installed"
      return
    fi
    if ! cmp -s "$installed" "$reference"; then
      record icons fail 1 "$installed differs from packaging/icons/dettivo-${size}.png"
      return
    fi
    crop="$repo_root/docs/design/studio/baselines/icon/icon-${size}.png"
    if [ -x "$tool" ] && [ -f "$crop" ]; then
      if ! QT_QPA_PLATFORM=offscreen "$tool" compare "$crop" "$installed" >"$out/icon-${size}.log" 2>&1; then
        code=1
        detail="$detail ${size}: $(grep -m1 score "$out/icon-${size}.log")"
      else
        detail="$detail ${size}: $(grep -m1 -o 'score [0-9.]*' "$out/icon-${size}.log")"
      fi
    else
      detail="$detail ${size}: same bytes"
    fi
  done
  if [ "$code" -ne 0 ]; then
    record icons fail 1 "an icon is under the artboard crop's threshold:$detail"
  else
    record icons pass 0 "four sizes match packaging/icons and the artboard crops:$detail"
  fi
}

# desktop-file-validate and systemd-analyze verify, when the tools exist.
check_desktop_and_units() {
  if command -v desktop-file-validate >/dev/null; then
    run desktop "dettivo.desktop validates" desktop-file-validate "$prefix/usr/share/applications/dettivo.desktop"
  else
    record desktop skip 0 "desktop-file-validate is not installed (pacman -S desktop-file-utils)"
  fi
  if command -v systemd-analyze >/dev/null; then
    local units=() u
    if [ -n "$prefix" ]; then
      # Under a prefix the ExecStart binaries are not at /usr/bin; verify
      # copies that point at the prefix, so the units and the binaries they
      # name are both checked.
      mkdir -p "$out/units"
      for u in dettivod.socket dettivod.service dettivo-osd.service; do
        sed "s|^ExecStart=/usr/bin/|ExecStart=$bin/|" "$prefix/usr/lib/systemd/user/$u" >"$out/units/$u"
        units+=("$out/units/$u")
      done
    else
      for u in dettivod.socket dettivod.service dettivo-osd.service; do units+=("/usr/lib/systemd/user/$u"); done
    fi
    # --user verify resolves RuntimeDirectory= under XDG_RUNTIME_DIR, which a
    # root CI container has no session to provide; an empty one is enough.
    local runtime="${XDG_RUNTIME_DIR:-}"
    [ -n "$runtime" ] || runtime="$(mktemp -d)"
    run units "the three user units verify" env XDG_RUNTIME_DIR="$runtime" systemd-analyze --user verify "${units[@]}"
  else
    record units skip 0 "systemd-analyze is not installed"
  fi
}

# `dettivo --version` prints the package version and the commit.
check_cli() {
  local got
  got="$("$bin/dettivo" --version 2>&1 || true)"
  if [ -n "${DETTIVO_EXPECTED_GIT_SHA:-}" ]; then
    if [ "$got" = "dettivo $version ($DETTIVO_EXPECTED_GIT_SHA)" ]; then
      record version pass 0 "$got"
    else
      record version fail 1 "expected dettivo $version ($DETTIVO_EXPECTED_GIT_SHA), got '$got'"
    fi
    return
  fi
  case "$got" in
    "dettivo $version ("*")") record version pass 0 "$got" ;;
    *) record version fail 1 "expected 'dettivo $version (<sha>)', got '$got'" ;;
  esac
}

# The three completion scripts load in their shells.
check_completions() {
  local bashc="$prefix/usr/share/bash-completion/completions/dettivo"
  local zshd="$prefix/usr/share/zsh/site-functions"
  local fishc="$prefix/usr/share/fish/vendor_completions.d/dettivo.fish"
  run completions_bash "bash sources $bashc" bash -c "source '$bashc' && complete -p dettivo >/dev/null"
  if command -v zsh >/dev/null; then
    run completions_zsh "zsh compinit loads _dettivo" \
      zsh -fc "fpath=('$zshd' \$fpath); autoload -U compinit; compinit -u -D; [[ \$_comps[dettivo] == _dettivo ]]"
  else
    record completions_zsh skip 0 "zsh is not installed"
  fi
  if command -v fish >/dev/null; then
    run completions_fish "fish sources dettivo.fish" fish -c "source '$fishc'"
  else
    record completions_fish skip 0 "fish is not installed"
  fi
}

# The app renders Home from the installed module on the offscreen platform;
# on a system install nothing points it at the module, which proves the
# compiled-in import path and the runtime path.
check_render() {
  local png="$out/home.png" log="$out/render.log" code=0 penv
  mapfile -t penv < <(prefix_env)
  rm -f "$png"
  env "${penv[@]}" QT_QPA_PLATFORM=offscreen "$bin/dettivo-app" --render "$png" --sample --open home >"$log" 2>&1 || code=$?
  if [ "$code" -ne 0 ]; then
    record render fail "$code" "dettivo-app --render exited $code; see $log: $(tail -n 2 "$log" | tr '\n' ' ')"
    return
  fi
  local magic size
  magic="$(head -c 4 "$png" 2>/dev/null | od -An -tx1 | tr -d ' \n')"
  size="$(stat -c %s "$png" 2>/dev/null || echo 0)"
  if [ "$magic" = "89504e47" ] && [ "$size" -gt 10000 ]; then
    record render pass 0 "Home rendered from $qml into $png ($size bytes)"
  else
    record render fail 1 "$png is not a PNG or is too small ($size bytes)"
  fi
}

# Every engine binary starts and prints its help.
check_engines_help() {
  local e
  # The diarization engine's help proves the sherpa-onnx libraries load
  # from beside it (ADR 0035).
  for e in whisper parakeet llm diarize; do
    run "engine_help_$e" "dettivo-engine-$e --help" "$engines/dettivo-engine-$e" --help
  done
}

# One engine's CLI mode under DETTIVO_FORCE_CPU=1 must report the CPU backend.
cpu_smoke() {
  local engine="$1" model="$2" fetch="$3"; shift 3
  local step="cpu_smoke_$engine" log="$out/cpu_smoke_$engine.log" code=0
  if [ ! -f "$model" ]; then
    record "$step" skip 0 "$model is missing ($fetch)"
    return
  fi
  DETTIVO_FORCE_CPU=1 "$engines/dettivo-engine-$engine" --model "$model" --json "$@" >"$log" 2>"$log.err" || code=$?
  if [ "$code" -ne 0 ]; then
    record "$step" fail "$code" "dettivo-engine-$engine exited $code; see $log.err: $(tail -n 2 "$log.err" | tr '\n' ' ')"
  elif grep -q '"backend": "cpu"' "$log"; then
    record "$step" pass 0 "backend cpu under DETTIVO_FORCE_CPU=1 on $(basename "$model")"
  else
    record "$step" fail 1 "backend is not cpu: $(grep -o '"backend": "[a-z]*"' "$log" | head -n1)"
  fi
}

# The CPU-fallback smoke over the three engines, and the backend the
# Whisper engine picks on its own (vulkan on a machine with a device).
check_cpu_smoke() {
  local wav="$models/fixtures/jfk.wav"
  local fetch="scripts/models/fetch-test-model.sh"
  cpu_smoke whisper "$models/whisper/tiny.en/ggml-tiny.en.bin" "$fetch" --wav "$wav"
  cpu_smoke parakeet "$models/parakeet/parakeet-v2/tdt-0.6b-v2-q8_0.gguf" "$fetch" --wav "$wav"
  cpu_smoke llm "$models/llm/qwen3-1.7b/Qwen3-1.7B-Q4_K_M.gguf" "scripts/models/fetch-llm-test-model.sh" \
    --prompt "Say hello" --max-tokens 4
  local model="$models/whisper/tiny.en/ggml-tiny.en.bin" natural code=0
  if [ -f "$model" ]; then
    natural="$("$engines/dettivo-engine-whisper" --wav "$wav" --model "$model" --json 2>/dev/null)" || code=$?
    natural="$(sed -n -E 's/.*"backend": "([a-z]+)".*/\1/p' <<<"$natural" | head -n1)"
    if [ "$code" -eq 0 ] && [[ "$natural" = cpu || "$natural" = vulkan ]]; then
      record natural_backend pass 0 "whisper without DETTIVO_FORCE_CPU picks $natural"
    else
      record natural_backend fail 1 "whisper backend probe exited $code with backend '${natural:-unknown}'"
    fi
  else
    record natural_backend skip 0 "$model is missing ($fetch)"
  fi
}

# One dictation of the fixture through the installed daemon on the CPU
# tier: the mock microphone feeds jfk.wav, the mock insert receives the
# text, the history keeps it, and the doctor names cpu per engine with the
# reason. The measured stop-to-transcript time is held to NFR-2's initial
# 2500 ms figure on a single warm take.
check_daemon_dictation() {
  local model="$models/whisper/tiny.en/ggml-tiny.en.bin" wav="$models/fixtures/jfk.wav"
  if [ ! -f "$model" ] || [ ! -f "$wav" ]; then
    record dictation skip 0 "tiny.en or jfk.wav is missing (scripts/models/fetch-test-model.sh)"
    return
  fi
  if ! command -v jq >/dev/null; then
    record dictation skip 0 "jq is not installed"
    return
  fi
  # The socket path has to fit sun_path, so it lives under the runtime
  # directory rather than the (possibly long) output directory.
  local tmp="$out/daemon" run_dir sock cfg log
  run_dir="$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/dettivo-it.XXXXXX")"
  mkdir -p "$tmp/data/dettivo" "$tmp/config" "$tmp/state"
  ln -sfn "$models" "$tmp/data/dettivo/models"
  sock="$run_dir/dettivo.sock"; cfg="$tmp/config.toml"; log="$tmp/dettivod.log"
  {
    [ -n "$prefix" ] && printf '[engines]\ndirectory = "%s"\n' "$engines"
    printf '[hotkeys]\nbackend = "none"\n[speech]\nmodel = "tiny.en"\n[dictation]\nlanguage = "en"\n'
  } >"$cfg"
  local penv
  mapfile -t penv < <(prefix_env)
  env "${penv[@]}" DETTIVO_QA_MODE=1 DETTIVO_QA_ALLOW_RELEASE=1 DETTIVO_FORCE_CPU=1 \
    DETTIVO_MOCK_MIC="$wav" DETTIVO_MOCK_INSERT=1 \
    XDG_DATA_HOME="$tmp/data" XDG_CONFIG_HOME="$tmp/config" XDG_STATE_HOME="$tmp/state" XDG_RUNTIME_DIR="$run_dir" \
    "$bin/dettivod" --socket "$sock" --config "$cfg" >"$log" 2>&1 &
  local pid=$!
  local cli=("$bin/dettivo" --socket "$sock")
  for _ in $(seq 1 50); do
    "${cli[@]}" --quiet status ping 2>/dev/null && break
    sleep 0.2
  done
  if ! "${cli[@]}" --quiet status ping 2>/dev/null; then
    record dictation fail 1 "dettivod did not answer on $sock within 10 s; see $log: $(tail -n 2 "$log" | tr '\n' ' ')"
    kill "$pid" 2>/dev/null || true
    rm -rf "$run_dir"
    return
  fi
  # The mock microphone plays the 11 s fixture in real time and ends the
  # take itself; the completion `dictation.state` event carries the split
  # (capture, transcribe, insert) the QA rig measures NFR-1 and NFR-2 from.
  # The warm take loads the engine, the measured one is the number.
  local events="$tmp/events.jsonl" timings ms=0 seen
  "${cli[@]}" events --follow >"$events" 2>>"$tmp/cli.log" &
  local events_pid=$!
  sleep 0.5
  for _ in warm measured; do
    seen="$(wc -l <"$events")"
    "${cli[@]}" --quiet dictation start >>"$tmp/cli.log" 2>&1 || true
    timings=""
    for _ in $(seq 1 200); do
      # Each line is `<time> <topic> <json>`; the completion is the
      # dictation.state event back to idle that carries the timings.
      timings="$(tail -n +"$((seen + 1))" "$events" | awk '$2 == "dictation.state" { print substr($0, index($0, "{")) }' \
        | jq -c 'select(.state == "idle" and .timings != null) | .timings' 2>/dev/null | head -n1 || true)"
      [ -n "$timings" ] && break
      sleep 0.2
    done
    if [ -n "$timings" ]; then
      ms="$(jq -r '.capture_ms + .transcribe_ms + .insert_ms' <<<"$timings")"
    else
      ms=-1
    fi
  done
  kill "$events_pid" 2>/dev/null || true
  local text
  text="$("${cli[@]}" history latest 2>/dev/null | tr '\n' ' ')"
  local doctor tier whisper_backend whisper_reason
  doctor="$("${cli[@]}" --json doctor 2>/dev/null || true)"
  tier="$(jq -r '.tier.tier // empty' <<<"$doctor")"
  whisper_backend="$(jq -r '.engines[] | select(.binary == "dettivo-engine-whisper") | .backend // empty' <<<"$doctor")"
  whisper_reason="$(jq -r '.engines[] | select(.binary == "dettivo-engine-whisper") | .reason // empty' <<<"$doctor")"
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  rm -rf "$run_dir"
  if [ "$ms" -lt 0 ]; then
    record dictation fail 1 "no completion event with timings arrived within 40 s; see $events, $tmp/cli.log and $log"
  elif ! grep -qi 'country' <<<"$text"; then
    record dictation fail 1 "the fixture's transcript did not reach the history: '${text:-}'; see $tmp/cli.log and $log"
  elif [ "$tier" != cpu ] || [ "$whisper_backend" != cpu ]; then
    record dictation fail 1 "doctor names tier '${tier:-}' and whisper backend '${whisper_backend:-}' instead of cpu"
  elif [ "$ms" -gt 2500 ]; then
    record dictation fail 1 "release to inserted took $ms ms ($timings), over NFR-2's 2500 ms"
  else
    record dictation pass 0 "fixture dictated on the cpu tier, release to inserted in $ms ms ($timings; NFR-2 2500 ms); doctor: whisper cpu, $whisper_reason"
  fi
}

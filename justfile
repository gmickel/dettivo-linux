# Dettivo for Linux build entry points.
#
# Five documented recipes: build, test, lint, qa, package. Each fans out to
# Cargo and CMake through scripts/run-step.sh, so the first failing toolchain
# stops the recipe and is named in the output. CI runs these same recipes.
# Install Just with `pacman -S just`; these recipes are the only build entry points.

set shell := ["bash", "-euo", "pipefail", "-c"]

qt_build_dir := "build/qt"
step := "scripts/run-step.sh"

# Build every Rust crate and every Qt binary.
default: build

# Name every missing toolchain piece before building anything.
toolchain:
    {{step}} toolchain "check toolchain" scripts/check-toolchain.sh

# Build every Rust crate and every Qt binary.
build: toolchain build-rust build-qt

build-rust:
    {{step}} cargo "build workspace" cargo build --workspace --all-targets

# The release profile of the binaries a drive runs (the Qt tree is Release
# already); `DETTIVO_BUILD_PROFILE=release just qa-pack gui` then proves the
# release routes (QA-4, ADR 0037). A drive picks `target/<profile>/` by that
# variable, `debug` by default, never the newer of the two builds.
build-release:
    {{step}} cargo "build release binaries" cargo build --release -p dettivod -p dettivo-cli -p dettivo-qa -p dettivo-mcp -p dettivo-engine-whisper

# The Whisper engine with the ggml Vulkan backend (needs vulkan-headers,
# vulkan-icd-loader and shaderc); CI builds it, `just build` does not.
build-whisper-vulkan:
    {{step}} cargo "whisper engine (vulkan)" cargo build -p dettivo-engine-whisper --features vulkan

# The Parakeet engine alone (parakeet.cpp built from the pinned source, CPU).
build-parakeet:
    {{step}} cargo "parakeet engine" cargo build -p dettivo-engine-parakeet

# The Parakeet engine with the ggml Vulkan backend (needs vulkan-headers,
# spirv-headers, vulkan-icd-loader and shaderc); CI builds it in the
# advisory parakeet-vulkan job.
build-parakeet-vulkan:
    {{step}} cargo "parakeet engine (vulkan)" cargo build -p dettivo-engine-parakeet --features vulkan

# The language model engine (llama.cpp through llama-cpp-2) on the CPU;
# `just build` compiles it too.
build-llm:
    {{step}} cargo "llm engine" cargo build -p dettivo-engine-llm

# The language model engine with the ggml Vulkan backend (needs
# vulkan-headers, vulkan-icd-loader and shaderc); CI builds it in the
# advisory llm-vulkan job.
build-llm-vulkan:
    {{step}} cargo "llm engine (vulkan)" cargo build -p dettivo-engine-llm --features vulkan

# The model-backed LLM tests on Vulkan (the protocol, partials, cancel and
# the CLI mode) on a development machine with the test model fetched by
# scripts/models/fetch-llm-test-model.sh. DETTIVO_TEST_BACKEND=vulkan makes
# every model-backed route load with the strict Vulkan preference and
# assert `backend = vulkan`, so a CPU fallback fails the suite instead of
# passing on the CPU.
test-llm-vulkan:
    DETTIVO_TEST_BACKEND=vulkan {{step}} cargo "llm engine tests (vulkan)" cargo test -p dettivo-engine-llm --features vulkan --test cli -- --nocapture

# The Parakeet WER fixtures on the Vulkan build (needs the models under
# $XDG_DATA_HOME/dettivo/models/parakeet and a Vulkan device).
test-parakeet-vulkan:
    {{step}} cargo "parakeet wer fixtures (vulkan)" cargo test -p dettivo-engine-parakeet --features vulkan --test cli -- --nocapture

build-qt:
    {{step}} cmake "configure qt" cmake -S qt -B {{qt_build_dir}} -G Ninja -DCMAKE_BUILD_TYPE=Release
    {{step}} cmake "build qt" cmake --build {{qt_build_dir}}

# Run Rust, Qt smoke and strict diarization scorer tests.
test: test-rust test-qt test-diarization-score

test-diarization-score:
    {{step}} python "strict diarization scorer" python3 scripts/qa/test_diarization_score.py
    {{step}} c++ "calibrated diarization clustering" bash scripts/qa/test-diarization-clustering.sh

test-rust:
    {{step}} cargo "unit tests" cargo test --workspace

test-qt: build-qt
    {{step}} ctest "qt smoke tests" ctest --test-dir {{qt_build_dir}} --output-on-failure

# Formatting, clippy, crate edges, file lengths, the notice, the packaging
# recipes, QML lint, the Omarchy plugin, the settings keys and docs.
lint: fmt clippy lint-edges lint-file-length lint-deltas lint-notice lint-packaging lint-qml lint-omarchy lint-settings-keys lint-toolchain docs

fmt:
    {{step}} rustfmt "format check" cargo fmt --all --check

clippy:
    {{step}} clippy "clippy" cargo clippy --workspace --all-targets -- -D warnings

# No crate depends on a crate it should not (tools/xtask/src/edges.rs).
lint-edges:
    {{step}} xtask "crate dependency edges" cargo run -q -p xtask -- lint-edges

# Rust and C++ files stay under 500 lines, QML under 300 (.file-length-allow).
lint-file-length:
    {{step}} xtask "file length" cargo run -q -p xtask -- lint-file-length

# Every contract item has one disposition in docs/api/linux-deltas.md.
lint-deltas:
    {{step}} xtask "contract delta register" cargo run -q -p xtask -- lint-deltas

# Every external crate in Cargo.lock has its row in NOTICE.md
# (`cargo run -p xtask -- lint-notice --write` regenerates the table).
lint-notice:
    {{step}} xtask "notice" cargo run -q -p xtask -- lint-notice

# The packaging scripts, the manifest check, the PKGBUILDs' versions and
# .SRCINFO, the desktop entry and the changelog section (ADR 0034).
lint-packaging:
    {{step}} packaging "packaging lint" scripts/packaging/lint.sh

# qmllint and qmlformat over every QML file, then the design-system rules:
# no literal colour or size outside the token singletons, icons on the 16 px
# grid, and every control with a documented accessible name.
lint-qml: build-qt
    {{step}} qmllint "qml lint and format" scripts/qml-lint.sh {{qt_build_dir}}
    {{step}} tokens "qml token lint" scripts/lint-qml-tokens.sh
    {{step}} icons "icon lint" scripts/lint-icons.sh
    {{step}} a11y "accessible name lint" scripts/lint-accessible-names.sh
    {{step}} copy "copy lint" scripts/lint-copy.sh
    {{step}} copy "copy lint self-test" scripts/lint-copy.sh --self-test

# Decision records are indexed and value-first; links resolve; rustdoc is clean.
# The Omarchy plugin folder: the manifest fields the shell needs, `omarchy
# plugin validate` where the shell is installed, and the plugin QML linted
# against the shell shim with the module from the build tree (docs/omarchy.md).
lint-omarchy: build-qt
    {{step}} omarchy "omarchy plugin lint" scripts/lint-omarchy-plugin.sh {{qt_build_dir}}

# The toolchain floor holds: a Qt API newer than 6.8 sits behind a version
# check (ADR 0013), and every tool scripts/check-toolchain.sh asks for is in
# the package list CI installs.
lint-toolchain:
    {{step}} qt-floor "qt floor" scripts/lint-qt-floor.sh
    {{step}} toolchain "prerequisite check proves itself" scripts/test-check-toolchain.sh
    {{step}} recipes "package entry points" bash scripts/test-build-entrypoints.sh

docs:
    {{step}} docs "documentation check" scripts/check-docs.sh
    {{step}} docs "documentation check proves itself" scripts/test-check-docs.sh
    {{step}} rustdoc "rustdoc" env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Install the systemd user units for this checkout and activate the socket.
install-units:
    {{step}} systemd "install user units" scripts/install-user-units.sh

# Measure socket activation end to end (needs install-units): under 300 ms.
qa-activation:
    {{step}} systemd "socket activation" scripts/qa-activation.sh

# The QA rig (ADR 0011): the runner's own tests, the contract replay against
# a fresh daemon, the scenario lint, the release-text lint and the drive pack
# on the fallback driver. Needs a display, a session bus and the a11y bus;
# CI wraps it in scripts/qa/xvfb-session.sh.
qa: qa-contract qa-mcp qa-rest lint-scenarios lint-release-text lint-a11y-names qa-drive-all

# Every schema key has a settings editor or a documented reason (ADR 0033).
lint-settings-keys:
    {{step}} settings "settings key coverage" scripts/lint-settings-keys.sh

# Every control and component has a documented accessible name.
lint-a11y-names:
    {{step}} a11y "accessible name lint" scripts/lint-accessible-names.sh
    {{step}} copy "copy lint" scripts/lint-copy.sh
    {{step}} copy "copy lint self-test" scripts/lint-copy.sh --self-test

# The fixture suite against a fresh daemon (pending methods are reported,
# `--strict` makes them fail: the release gate).
qa-contract: build-rust
    {{step}} cargo "qa runner tests" cargo test -p dettivo-qa
    {{step}} qa "contract replay" cargo run -q -p dettivo-qa -- contract

# The MCP harness (ADR 0019): the server binary driven in both framings
# against a seeded daemon, reported per tool and framing.
qa-mcp: build-rust
    {{step}} qa "mcp harness" cargo run -q -p dettivo-qa -- mcp

# The REST fixtures against the shim the seeded daemon hosts (ADR 0028).
qa-rest:
    {{step}} qa "rest harness" cargo run -q -p dettivo-qa -- rest

# One scenario on one driver: just qa-drive placeholder_window atspi|cua
qa-drive scenario driver="atspi": build-rust build-qt
    {{step}} qa "drive {{scenario}} on {{driver}}" cargo run -q -p dettivo-qa -- drive {{scenario}} --driver {{driver}}

# The whole pack on the fallback driver.
qa-drive-all: build-rust build-qt
    {{step}} qa "drive pack (atspi)" cargo run -q -p dettivo-qa -- drive all --driver atspi

# The whole pack on cua-driver (pinned; scripts/qa/install-cua-driver.sh).
qa-drive-cua: build-rust build-qt
    {{step}} qa "drive pack (cua)" cargo run -q -p dettivo-qa -- drive all --driver cua

# A named pack of scenarios into one report (docs/qa.md, docs/RELEASING.md):
# just qa-pack dictation, just qa-pack gui; `dettivo-qa pack list` names the
# packs. The report lands under qa-evidence/<run>/pack-<name>/. Allowed skips
# are recorded as blockers; a hard failure exits 1.
qa-pack name="dictation": build-rust build-qt
    {{step}} qa "pack {{name}}" cargo run -q -p dettivo-qa -- pack {{name}}

# The meetings pack (docs/qa.md, ADR 0039) with the Vulkan Whisper engine
# for the GPU tier's throughput figure, filed into docs/reports/benchmarks:
# just qa-pack-meetings target/vulkan/debug. The default engines
# directory is the workspace build.
qa-pack-meetings engines="target/debug": build-rust build-qt
    {{step}} qa "pack meetings" cargo run -q -p dettivo-qa -- pack meetings --engines {{engines}} --record

# The same pack on the CPU tier (DETTIVO_FORCE_CPU=1 for every daemon and
# engine, base.en for the throughput steps), filed beside the GPU row;
# the command the CPU-only VM runs.
qa-pack-meetings-cpu engines="target/debug": build-rust build-qt
    {{step}} qa "pack meetings (cpu)" cargo run -q -p dettivo-qa -- pack meetings --cpu --engines {{engines}} --record

# Every R-ID of every spec mapped to an evidence route that exists
# (qa/evidence-map.toml, docs/guides/qa.md); writes docs/reports/evidence-map.json.
qa-evidence-map: build-rust
    {{step}} qa "evidence map" cargo run -q -p dettivo-qa -- evidence-map --write

# The release gate (docs/RELEASING.md, ADR 0041): fifteen named steps as
# whole commands, every report embedded, every blocker named, filed under
# docs/reports/release-gate/<version>.json. Point it at the Vulkan
# engines for the GPU tier: just qa-release target/vulkan/debug;
# on this desktop wrap it in scripts/qa/xvfb-session.sh.
qa-release engines="target/debug" *flags: build-rust build-qt
    {{step}} qa "pack release" cargo run -q -p dettivo-qa -- pack release --engines {{engines}} --record {{flags}}

# The virtual audio rig round trip (needs PipeWire; reports skipped without it).
qa-audio: build-rust
    {{step}} qa "audio rig" cargo run -q -p dettivo-qa -- audio-check

# The pill's frame pacing on this display: ten seconds of the listening
# animation, zero dropped frames and under a millisecond a frame (docs/osd.md).
qa-osd-pacing: build-rust build-qt
    {{step}} qa "osd frame pacing" cargo run -q -p dettivo-qa -- osd-pacing --seconds 10

# Visual regression (ADR 0021): every manifest surface at 1x and 2x on
# every theme fixture and both built-in palettes against its approved
# baseline; exit 1 on an unapproved difference. Report and diff images
# under qa-evidence/visual/.
qa-visual: build-rust build-qt
    {{step}} qa "visual regression" cargo run -q -p dettivo-qa -- visual

# The canary: the same renders with a deliberate token regression; passes
# only when every diff fails, so a diff that stopped seeing is caught.
qa-visual-canary: build-rust build-qt
    {{step}} qa "visual canary" cargo run -q -p dettivo-qa -- visual --canary

# The beauty pass (ADR 0042): one contact sheet per surface on the five
# palettes and beauty-report.md with the checklist's machine verdicts
# filled and the human items open; `--themes all` adds a sheet across
# every theme installed under Omarchy's themes directory.
qa-beauty *flags: build-rust build-qt
    {{step}} qa "beauty pass" cargo run -q -p dettivo-qa -- beauty {{flags}}

# Approve fresh renders as baselines: just qa-visual-approve osd --theme tokyo-night --scale 2
qa-visual-approve surface *flags: build-rust build-qt
    {{step}} qa "visual approve {{surface}}" cargo run -q -p dettivo-qa -- visual approve {{surface}} {{flags}}

# The chunked import pipeline against the golden (ADR 0022): the jfk clip
# four times, cut by the chunker, each chunk through the whisper engine's
# CLI mode, merged, compared with crates/dettivo-qa/fixtures/import-merge/.
# Needs tiny.en and jfk.wav under $XDG_DATA_HOME/dettivo/models; skips
# without them.
qa-import-merge: build-rust
    {{step}} qa "import merge pipeline" cargo run -q -p dettivo-qa -- pipeline import-merge --cpu

# The polish evaluation harness on the sample set with the recorded
# rewrites (no model): the metrics must match the checked-in golden
# (docs/polish-models.md). CI runs it after the contract replay.
qa-polish-eval: build-rust
    {{step}} qa "polish eval (sample, golden)" cargo run -q -p dettivo-qa -- polish-eval --set crates/dettivo-qa/fixtures/polish-eval/sample.jsonl --model fixture:crates/dettivo-qa/fixtures/polish-eval/rewrites --golden crates/dettivo-qa/fixtures/polish-eval/golden.json --out none

# A held-out set through the Enhanced pipeline against a candidate model,
# scored with the macOS metrics and gates; the report lands under
# docs/reports/polish-eval/. just polish-eval <jsonl> <id|sideload|path> [flags]
polish-eval set model *flags: build-rust
    {{step}} qa "polish eval {{model}}" cargo run -q -p dettivo-qa -- polish-eval --set {{set}} --model {{model}} {{flags}}

# The fine-tune conversion runbook's toolchain check: the converters, the
# pinned llama.cpp revision and the paths, without converting anything.
polish-convert-check:
    {{step}} convert "polish fine-tune conversion (dry run)" scripts/models/convert-polish-finetune.sh --dry-run --source scripts/models/fixtures/hf-checkpoint --out build/polish-experiments
    {{step}} convert "polish fine-tune conversion publishes only a complete model" scripts/models/tests/convert-polish-finetune-staging.sh
# The benchmark suite (ADR 0029): first insert, transcription throughput,
# the language model, the idle footprint and startup from the fixtures the
# tests use, one report per host and tier filed under
# docs/reports/benchmarks with its README table (docs/qa.md). Point it at
# the Vulkan engines for the GPU tier: just bench <dir>.
bench engines="target/debug": build-rust
    {{step}} qa "bench" cargo run -q -p dettivo-qa -- bench --engines {{engines}} --out docs/reports/benchmarks

# The CPU tier on the same machine (DETTIVO_FORCE_CPU=1 for every engine),
# filed beside the GPU report.
bench-cpu engines="target/debug": build-rust
    {{step}} qa "bench (cpu)" cargo run -q -p dettivo-qa -- bench --cpu --engines {{engines}} --out docs/reports/benchmarks

# Three iterations on the CPU, recorded under qa-evidence and never gated
# (the CI unit job uploads it).
bench-quick: build-rust
    {{step}} qa "bench (quick, cpu)" cargo run -q -p dettivo-qa -- bench --quick --cpu

# The diarization pipeline (ADR 0035): the two-speaker fixture through the
# diarization engine's CLI mode, scored by diarization error rate against
# crates/dettivo-qa/fixtures/diarization/two-speakers.turns.json, with the
# realtime factor recorded. Needs the model set under
# $XDG_DATA_HOME/dettivo/models/diarize (scripts/models/fetch-diarization-model.sh);
# skips without it.
qa-diarization: build-rust
    {{step}} qa "diarization pipeline" cargo run -q -p dettivo-qa -- pipeline diarization --bench

# The app's startup and live-theme budgets on this display: first frame
# under 300 ms, RSS <=320 MiB and anonymous <=128 MiB after five seconds,
# the theme applied under 100 ms (docs/app.md). The drives record these; the gate
# variable makes them assertions here.
qa-app-timing driver="atspi": build-rust build-qt
    {{step}} qa "app routes with the timing gate ({{driver}})" env DETTIVO_TIMING_GATE=1 cargo run -q -p dettivo-qa -- drive app_routes --driver {{driver}}
    {{step}} qa "app live theme with the timing gate ({{driver}})" env DETTIVO_TIMING_GATE=1 cargo run -q -p dettivo-qa -- drive app_theme_live --driver {{driver}}

# Every scenario uses the driver interface only.
lint-scenarios: build-rust
    {{step}} qa "scenario lint" cargo run -q -p dettivo-qa -- lint-scenarios

# No release route carries developer text.
lint-release-text:
    {{step}} text "release text lint" scripts/lint-release-text.sh

# Release build of both toolchains staged under dist/ as a tarball.
# The release tree and tarball under dist/ (ADR 0034): every binary, the
# three engines with the Vulkan backend, the QML module, the units, the
# desktop entry, the icons, the completions, the licence and NOTICE.md,
# checked against packaging/manifest.txt.
package dist="dist":
    {{step}} package "release tree" scripts/package.sh {{dist}}

# The dettivo-bin package from the tarball `just package` wrote, with namcap.
package-bin dist="dist": (package dist)
    {{step}} makepkg "dettivo-bin package" scripts/packaging/build-package.sh bin {{dist}}

# The dettivo package from an archive of this tree (a full release build).
package-source:
    {{step}} makepkg "dettivo package" scripts/packaging/build-package.sh source

# The install test against the built dettivo-bin package under a throwaway
# prefix (`just install-test`), or a system install with the session arm
# (`just install-test "--session"`, a fresh VM).
install-test flags="--root build/install-root":
    {{step}} install "install test" scripts/packaging/install-test.sh {{flags}} --report build/install-test.json "$(scripts/packaging/select-package.sh dettivo-bin build/aur)"

# The release notes for a version from CHANGELOG.md, and the version check.
release-notes version:
    {{step}} notes "release notes {{version}}" scripts/packaging/release-notes.sh {{version}}

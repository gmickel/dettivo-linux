# Notice

Dettivo for Linux is licensed under the GNU General Public License, version 3 or any later version (see [LICENSE](LICENSE)). The binaries the package installs link or bundle the components below under their own licences; the package installs this file at `/usr/share/doc/dettivo/NOTICE.md` so the list travels with them. `cargo run -p xtask -- lint-notice` keeps the crate table in step with `Cargo.lock` and `just lint` runs it; `--write` regenerates the table.

## Inference engines and their ports

| Component | Where | License |
|---|---|---|
| whisper.cpp and its ggml | `dettivo-engine-whisper`, through the `whisper-rs` and `whisper-rs-sys` crates (built from the pinned source at build time) | MIT, Georgi Gerganov and the ggml authors |
| llama.cpp and its ggml | `dettivo-engine-llm`, through the `llama-cpp-2` and `llama-cpp-sys-2` crates (built from the pinned source at build time) | MIT, Georgi Gerganov and the ggml authors |
| parakeet.cpp 0.5.0 and its ggml 0.13.0 | `dettivo-engine-parakeet`, through the in-repo `parakeet-cpp-sys` crate (the pinned archives are fetched, verified and built at build time) | MIT, the parakeet.cpp authors; ggml MIT |
| The ggml Vulkan shaders | compiled into the three engines above with `glslc` from shaderc | MIT (ggml); the compiled SPIR-V carries no separate licence |
| sherpa-onnx 1.13.7 (`libsherpa-onnx-c-api.so`) | `dettivo-engine-diarize`, through the in-repo `sherpa-onnx-sys` crate; CPU uses the checksummed prebuilt archive, CUDA rebuilds the checksummed source with `cuda-conv-default.patch` selecting the default cuDNN convolution-search mode; the library is installed beside the engine | Apache-2.0, the k2-fsa authors (Xiaomi Corporation) |
| ONNX Runtime (`libonnxruntime.so`, the build sherpa-onnx 1.13.7 ships; the CUDA drop-in also carries `libonnxruntime_providers_cuda.so` and `libonnxruntime_providers_shared.so`) | loaded by `libsherpa-onnx-c-api.so` from the same directory; nothing else in the workspace links it | MIT, Microsoft Corporation |

The models the engines run (Whisper, Parakeet TDT, the Qwen3 GGUF catalogue, the diarization model set) are never part of the package; the catalogue downloads them on request under their own licences (see [docs/models.md](docs/models.md)).

## Desktop toolkit and fonts

| Component | Where | License |
|---|---|---|
| Qt 6 (Core, Gui, Qml, Quick, QuickControls2, DBus, Svg, Wayland) | `dettivo-app`, `dettivo-osd`, `dettivo-sheet`, `dettivo-insert-target` and the `Dettivo` and `DettivoStyle` QML modules, linked dynamically against the distribution's packages | LGPL-3.0-only (or GPL-2.0 or GPL-3.0), The Qt Company |
| layer-shell-qt | `dettivo-osd`, linked dynamically | LGPL-3.0-or-later, KDE |
| tomlplusplus | the theme reader in the `Dettivo` module, header-only | MIT, Mark Gillard |
| Cascadia Mono Nerd Font | the pill and the app read the monospace face Omarchy installs; nothing is bundled | OFL-1.1 (Cascadia Code, Microsoft) and MIT (the Nerd Fonts patches) |

## Ported code

| Component | Where | License |
|---|---|---|
| Dettivo for macOS: the IPC contract, the Polish text layers and their golden cases, the model catalogue | `docs/api/`, `crates/dettivo-language`, `crates/dettivo-speech` | The same author and licence as this repository |

## Reused source

Voxtype is prior art for this port and its source is MIT licensed; the default posture is a clean-room implementation from behaviour, and [docs/reports/voxtype-reuse-review.md](docs/reports/voxtype-reuse-review.md) walks every area with its verdict. A span copied from Voxtype carries the comment `Reused from Voxtype (MIT), see NOTICE.md` and one row below naming the upstream file and commit; `cargo run -p xtask -- lint-notice` fails on a marker without a row and on a row without a marker, by path. The table is empty because no span is reused today.

<!-- reuse:begin -->
| File | Upstream | Commit | License |
|---|---|---|---|
<!-- reuse:end -->

## Rust crates

Every crate below is linked into one or more of the installed binaries as `Cargo.lock` pins it. The table is generated; the licence column is each crate's declared SPDX expression.

<!-- crates:begin -->
| Crate | Version | License |
|---|---|---|
| `aho-corasick` | 1.1.5 | Unlicense OR MIT |
| `annotate-snippets` | 0.11.5 | MIT OR Apache-2.0 |
| `anstream` | 1.0.0 | MIT OR Apache-2.0 |
| `anstyle` | 1.0.14 | MIT OR Apache-2.0 |
| `anstyle-parse` | 1.0.0 | MIT OR Apache-2.0 |
| `anstyle-query` | 1.1.5 | MIT OR Apache-2.0 |
| `anstyle-wincon` | 3.0.11 | MIT OR Apache-2.0 |
| `ashpd` | 0.13.13 | MIT |
| `async-broadcast` | 0.7.2 | MIT OR Apache-2.0 |
| `async-channel` | 2.5.0 | Apache-2.0 OR MIT |
| `async-executor` | 1.14.0 | Apache-2.0 OR MIT |
| `async-io` | 2.6.0 | Apache-2.0 OR MIT |
| `async-lock` | 3.4.2 | Apache-2.0 OR MIT |
| `async-process` | 2.5.0 | Apache-2.0 OR MIT |
| `async-recursion` | 1.1.1 | MIT OR Apache-2.0 |
| `async-signal` | 0.2.14 | Apache-2.0 OR MIT |
| `async-task` | 4.7.1 | Apache-2.0 OR MIT |
| `async-trait` | 0.1.92 | MIT OR Apache-2.0 |
| `atomic-waker` | 1.1.2 | Apache-2.0 OR MIT |
| `atspi` | 0.30.0 | Apache-2.0 OR MIT |
| `atspi-common` | 0.14.0 | Apache-2.0 OR MIT |
| `atspi-connection` | 0.14.0 | Apache-2.0 OR MIT |
| `atspi-proxies` | 0.14.0 | Apache-2.0 OR MIT |
| `audio-core` | 0.2.1 | MIT OR Apache-2.0 |
| `audioadapter` | 4.0.0 | MIT OR Apache-2.0 |
| `audioadapter-buffers` | 4.0.0 | MIT OR Apache-2.0 |
| `audioadapter-sample` | 4.0.0 | MIT OR Apache-2.0 |
| `autocfg` | 1.5.1 | Apache-2.0 OR MIT |
| `base64` | 0.22.1 | MIT OR Apache-2.0 |
| `bindgen` | 0.72.1 | BSD-3-Clause |
| `bit-set` | 0.8.0 | Apache-2.0 OR MIT |
| `bit-vec` | 0.8.0 | Apache-2.0 OR MIT |
| `bitflags` | 2.13.1 | MIT OR Apache-2.0 |
| `bitvec` | 1.1.1 | MIT |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 |
| `blocking` | 1.7.0 | Apache-2.0 OR MIT |
| `bumpalo` | 3.20.3 | MIT OR Apache-2.0 |
| `bytemuck` | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| `bytes` | 1.12.1 | MIT |
| `cc` | 1.4.4 | MIT OR Apache-2.0 |
| `cexpr` | 0.6.0 | Apache-2.0 OR MIT |
| `cfg-expr` | 0.20.9 | MIT OR Apache-2.0 |
| `cfg-if` | 1.0.4 | MIT OR Apache-2.0 |
| `cfg_aliases` | 0.2.2 | MIT |
| `chacha20` | 0.10.2 | MIT OR Apache-2.0 |
| `clang-sys` | 1.9.1 | Apache-2.0 |
| `clap` | 4.6.6 | MIT OR Apache-2.0 |
| `clap_builder` | 4.6.6 | MIT OR Apache-2.0 |
| `clap_complete` | 4.6.9 | MIT OR Apache-2.0 |
| `clap_derive` | 4.6.4 | MIT OR Apache-2.0 |
| `clap_lex` | 1.1.0 | MIT OR Apache-2.0 |
| `cmake` | 0.1.58 | MIT OR Apache-2.0 |
| `colorchoice` | 1.0.5 | MIT OR Apache-2.0 |
| `concurrent-queue` | 2.5.0 | Apache-2.0 OR MIT |
| `cookie-factory` | 0.3.3 | MIT |
| `cpufeatures` | 0.2.17 | MIT OR Apache-2.0 |
| `cpufeatures` | 0.3.1 | MIT OR Apache-2.0 |
| `crossbeam-utils` | 0.8.22 | MIT OR Apache-2.0 |
| `crypto-common` | 0.1.7 | MIT OR Apache-2.0 |
| `digest` | 0.10.7 | MIT OR Apache-2.0 |
| `displaydoc` | 0.2.7 | MIT OR Apache-2.0 |
| `downcast-rs` | 1.2.1 | MIT OR Apache-2.0 |
| `either` | 1.18.0 | MIT OR Apache-2.0 |
| `encoding_rs` | 0.8.35 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| `endi` | 1.1.1 | MIT |
| `enumflags2` | 0.7.12 | MIT OR Apache-2.0 |
| `enumflags2_derive` | 0.7.12 | MIT OR Apache-2.0 |
| `equivalent` | 1.0.2 | Apache-2.0 OR MIT |
| `errno` | 0.3.14 | MIT OR Apache-2.0 |
| `evdev` | 0.13.2 | Apache-2.0 OR MIT |
| `event-listener` | 5.4.2 | Apache-2.0 OR MIT |
| `event-listener-strategy` | 0.5.4 | Apache-2.0 OR MIT |
| `extended` | 0.1.0 | MIT |
| `fallible-iterator` | 0.3.0 | MIT OR Apache-2.0 |
| `fallible-streaming-iterator` | 0.1.9 | MIT OR Apache-2.0 |
| `fancy-regex` | 0.19.0 | MIT |
| `fastrand` | 2.5.0 | Apache-2.0 OR MIT |
| `find-msvc-tools` | 0.1.11 | MIT OR Apache-2.0 |
| `find_cuda_helper` | 0.2.0 | MIT OR Apache-2.0 |
| `fixedbitset` | 0.5.7 | MIT OR Apache-2.0 |
| `foldhash` | 0.1.5 | Zlib |
| `form_urlencoded` | 1.2.2 | MIT OR Apache-2.0 |
| `fs_extra` | 1.3.0 | MIT |
| `funty` | 2.0.0 | MIT |
| `futures-channel` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-core` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-io` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-lite` | 2.6.1 | Apache-2.0 OR MIT |
| `futures-macro` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-sink` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-task` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-util` | 0.3.34 | MIT OR Apache-2.0 |
| `generic-array` | 0.14.7 | MIT |
| `gethostname` | 1.1.0 | Apache-2.0 |
| `getrandom` | 0.2.17 | MIT OR Apache-2.0 |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 |
| `glob` | 0.3.4 | MIT OR Apache-2.0 |
| `hashbrown` | 0.15.5 | MIT OR Apache-2.0 |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 |
| `hashlink` | 0.10.0 | MIT OR Apache-2.0 |
| `heck` | 0.5.0 | MIT OR Apache-2.0 |
| `hermit-abi` | 0.5.3 | MIT OR Apache-2.0 |
| `hex` | 0.4.3 | MIT OR Apache-2.0 |
| `hound` | 3.5.1 | Apache-2.0 |
| `http` | 1.5.0 | MIT OR Apache-2.0 |
| `http-body` | 1.1.0 | MIT |
| `http-body-util` | 0.1.5 | MIT |
| `httparse` | 1.10.1 | MIT OR Apache-2.0 |
| `hyper` | 1.11.1 | MIT |
| `hyper-rustls` | 0.27.9 | Apache-2.0 OR ISC OR MIT |
| `hyper-util` | 0.1.20 | MIT |
| `icu_collections` | 2.1.1 | Unicode-3.0 |
| `icu_locale_core` | 2.1.1 | Unicode-3.0 |
| `icu_normalizer` | 2.1.1 | Unicode-3.0 |
| `icu_normalizer_data` | 2.1.1 | Unicode-3.0 |
| `icu_properties` | 2.1.2 | Unicode-3.0 |
| `icu_properties_data` | 2.1.2 | Unicode-3.0 |
| `icu_provider` | 2.1.1 | Unicode-3.0 |
| `idna` | 1.1.0 | MIT OR Apache-2.0 |
| `idna_adapter` | 1.2.1 | Apache-2.0 OR MIT |
| `indexmap` | 2.14.1 | Apache-2.0 OR MIT |
| `ipnet` | 2.12.1 | MIT OR Apache-2.0 |
| `is_terminal_polyfill` | 1.70.2 | MIT OR Apache-2.0 |
| `itertools` | 0.13.0 | MIT OR Apache-2.0 |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 |
| `jobserver` | 0.1.35 | MIT OR Apache-2.0 |
| `js-sys` | 0.3.104 | MIT OR Apache-2.0 |
| `lazy_static` | 1.5.0 | MIT OR Apache-2.0 |
| `libc` | 0.2.189 | MIT OR Apache-2.0 |
| `libloading` | 0.8.9 | ISC |
| `libspa` | 0.10.1 | MIT |
| `libspa-sys` | 0.10.1 | MIT |
| `libsqlite3-sys` | 0.35.0 | MIT |
| `linux-raw-sys` | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `litemap` | 0.8.3 | Unicode-3.0 |
| `llama-cpp-2` | 0.1.156 | MIT OR Apache-2.0 |
| `llama-cpp-sys-2` | 0.1.156 | MIT OR Apache-2.0 |
| `log` | 0.4.34 | MIT OR Apache-2.0 |
| `lru-slab` | 0.1.2 | MIT OR Apache-2.0 OR Zlib |
| `matchers` | 0.2.0 | MIT |
| `memchr` | 2.8.3 | Unlicense OR MIT |
| `memmap2` | 0.9.11 | MIT OR Apache-2.0 |
| `memoffset` | 0.9.1 | MIT |
| `minimal-lexical` | 0.2.1 | MIT OR Apache-2.0 |
| `mio` | 1.2.3 | MIT |
| `nix` | 0.29.0 | MIT |
| `nom` | 7.1.3 | MIT |
| `nom` | 8.0.0 | MIT |
| `nu-ansi-term` | 0.50.3 | MIT |
| `num-complex` | 0.4.6 | MIT OR Apache-2.0 |
| `num-integer` | 0.1.47 | MIT OR Apache-2.0 |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 |
| `once_cell_polyfill` | 1.70.2 | MIT OR Apache-2.0 |
| `ordered-stream` | 0.2.0 | MIT OR Apache-2.0 |
| `os_pipe` | 1.2.3 | MIT |
| `parking` | 2.2.1 | Apache-2.0 OR MIT |
| `percent-encoding` | 2.3.2 | MIT OR Apache-2.0 |
| `petgraph` | 0.8.3 | MIT OR Apache-2.0 |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT |
| `piper` | 0.2.5 | MIT OR Apache-2.0 |
| `pipewire` | 0.10.1 | MIT |
| `pipewire-sys` | 0.10.1 | MIT |
| `pkg-config` | 0.3.34 | MIT OR Apache-2.0 |
| `polling` | 3.11.0 | Apache-2.0 OR MIT |
| `potential_utf` | 0.1.6 | Unicode-3.0 |
| `prettyplease` | 0.2.37 | MIT OR Apache-2.0 |
| `proc-macro-crate` | 3.5.0 | MIT OR Apache-2.0 |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 |
| `quick-xml` | 0.38.4 | MIT |
| `quick-xml` | 0.41.0 | MIT |
| `quinn` | 0.11.11 | MIT OR Apache-2.0 |
| `quinn-proto` | 0.11.17 | MIT OR Apache-2.0 |
| `quinn-udp` | 0.5.15 | MIT OR Apache-2.0 |
| `quote` | 1.0.47 | MIT OR Apache-2.0 |
| `r-efi` | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| `radium` | 0.7.0 | MIT |
| `rand` | 0.10.2 | MIT OR Apache-2.0 |
| `rand_core` | 0.10.1 | MIT OR Apache-2.0 |
| `rand_pcg` | 0.10.2 | MIT OR Apache-2.0 |
| `regex` | 1.13.1 | MIT OR Apache-2.0 |
| `regex-automata` | 0.4.18 | MIT OR Apache-2.0 |
| `regex-lite` | 0.1.9 | MIT OR Apache-2.0 |
| `regex-syntax` | 0.8.11 | MIT OR Apache-2.0 |
| `reis` | 0.7.1 | MIT |
| `reqwest` | 0.12.28 | MIT OR Apache-2.0 |
| `ring` | 0.17.14 | Apache-2.0 AND ISC |
| `rubato` | 4.0.0 | MIT OR Apache-2.0 |
| `rusqlite` | 0.37.0 | MIT |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT |
| `rustix` | 1.1.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `rustls` | 0.23.43 | Apache-2.0 OR ISC OR MIT |
| `rustls-pki-types` | 1.15.1 | MIT OR Apache-2.0 |
| `rustls-webpki` | 0.103.15 | ISC |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 |
| `ryu` | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| `same-file` | 1.0.6 | Unlicense OR MIT |
| `semver` | 1.0.28 | MIT OR Apache-2.0 |
| `serde` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 |
| `serde_path_to_error` | 0.1.20 | MIT OR Apache-2.0 |
| `serde_repr` | 0.1.21 | MIT OR Apache-2.0 |
| `serde_spanned` | 1.1.1 | MIT OR Apache-2.0 |
| `serde_urlencoded` | 0.7.1 | MIT OR Apache-2.0 |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 |
| `sharded-slab` | 0.1.7 | MIT |
| `shlex` | 1.3.0 | MIT OR Apache-2.0 |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 |
| `signal-hook-registry` | 1.4.8 | MIT OR Apache-2.0 |
| `slab` | 0.4.12 | MIT |
| `smallvec` | 1.16.0 | MIT OR Apache-2.0 |
| `socket2` | 0.6.5 | MIT OR Apache-2.0 |
| `stable_deref_trait` | 1.2.1 | MIT OR Apache-2.0 |
| `static_assertions` | 1.1.0 | MIT OR Apache-2.0 |
| `strsim` | 0.11.1 | MIT |
| `subtle` | 2.6.1 | BSD-3-Clause |
| `symphonia` | 0.6.1 | MPL-2.0 |
| `symphonia-bundle-flac` | 0.6.1 | MPL-2.0 |
| `symphonia-bundle-mp3` | 0.6.1 | MPL-2.0 |
| `symphonia-codec-aac` | 0.6.1 | MPL-2.0 |
| `symphonia-codec-alac` | 0.6.1 | MPL-2.0 |
| `symphonia-codec-pcm` | 0.6.1 | MPL-2.0 |
| `symphonia-codec-vorbis` | 0.6.1 | MPL-2.0 |
| `symphonia-common` | 0.6.1 | MPL-2.0 |
| `symphonia-core` | 0.6.1 | MPL-2.0 |
| `symphonia-format-caf` | 0.6.1 | MPL-2.0 |
| `symphonia-format-isomp4` | 0.6.1 | MPL-2.0 |
| `symphonia-format-ogg` | 0.6.1 | MPL-2.0 |
| `symphonia-format-riff` | 0.6.1 | MPL-2.0 |
| `symphonia-metadata` | 0.6.1 | MPL-2.0 |
| `syn` | 2.0.119 | MIT OR Apache-2.0 |
| `syn` | 3.0.4 | MIT OR Apache-2.0 |
| `sync_wrapper` | 1.0.2 | Apache-2.0 |
| `synstructure` | 0.13.2 | MIT |
| `system-deps` | 7.0.8 | MIT OR Apache-2.0 |
| `tap` | 1.0.1 | MIT |
| `target-lexicon` | 0.13.5 | Apache-2.0 WITH LLVM-exception |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 |
| `thiserror` | 2.0.20 | MIT OR Apache-2.0 |
| `thiserror-impl` | 2.0.20 | MIT OR Apache-2.0 |
| `thread_local` | 1.1.10 | MIT OR Apache-2.0 |
| `tinystr` | 0.8.4 | Unicode-3.0 |
| `tinyvec` | 1.13.0 | Zlib OR Apache-2.0 OR MIT |
| `tinyvec_macros` | 0.1.1 | MIT OR Apache-2.0 OR Zlib |
| `tokio` | 1.53.1 | MIT |
| `tokio-macros` | 2.7.2 | MIT |
| `tokio-rustls` | 0.26.4 | MIT OR Apache-2.0 |
| `toml` | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml_datetime` | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml_edit` | 0.25.13+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml_parser` | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml_writer` | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| `tower` | 0.5.3 | MIT |
| `tower-http` | 0.6.11 | MIT |
| `tower-layer` | 0.3.3 | MIT |
| `tower-service` | 0.3.3 | MIT |
| `tracing` | 0.1.44 | MIT |
| `tracing-attributes` | 0.1.31 | MIT |
| `tracing-core` | 0.1.36 | MIT |
| `tracing-log` | 0.2.0 | MIT |
| `tracing-subscriber` | 0.3.23 | MIT |
| `tree_magic_mini` | 3.2.2 | MIT |
| `try-lock` | 0.2.5 | MIT |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 |
| `uds_windows` | 1.2.1 | MIT |
| `unicode-ident` | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| `unicode-width` | 0.2.2 | MIT OR Apache-2.0 |
| `untrusted` | 0.9.0 | ISC |
| `url` | 2.5.8 | MIT OR Apache-2.0 |
| `utf8_iter` | 1.0.4 | Apache-2.0 OR MIT |
| `utf8parse` | 0.2.2 | Apache-2.0 OR MIT |
| `uuid` | 1.26.0 | Apache-2.0 OR MIT |
| `valuable` | 0.1.1 | MIT |
| `vcpkg` | 0.2.15 | MIT OR Apache-2.0 |
| `version-compare` | 0.2.1 | MIT |
| `version_check` | 0.9.5 | MIT OR Apache-2.0 |
| `visibility` | 0.1.1 | Zlib OR MIT OR Apache-2.0 |
| `walkdir` | 2.5.0 | Unlicense OR MIT |
| `want` | 0.3.1 | MIT |
| `wasi` | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `wasm-bindgen` | 0.2.127 | MIT OR Apache-2.0 |
| `wasm-bindgen-futures` | 0.4.77 | MIT OR Apache-2.0 |
| `wasm-bindgen-macro` | 0.2.127 | MIT OR Apache-2.0 |
| `wasm-bindgen-macro-support` | 0.2.127 | MIT OR Apache-2.0 |
| `wasm-bindgen-shared` | 0.2.127 | MIT OR Apache-2.0 |
| `wayland-backend` | 0.3.17 | MIT |
| `wayland-client` | 0.31.15 | MIT |
| `wayland-protocols` | 0.32.12 | MIT |
| `wayland-protocols-misc` | 0.3.12 | MIT |
| `wayland-protocols-wlr` | 0.3.12 | MIT |
| `wayland-scanner` | 0.31.11 | MIT |
| `wayland-sys` | 0.31.11 | MIT |
| `web-sys` | 0.3.104 | MIT OR Apache-2.0 |
| `web-time` | 1.1.0 | MIT OR Apache-2.0 |
| `webpki-roots` | 1.0.9 | CDLA-Permissive-2.0 |
| `whisper-rs` | 0.16.0 | Unlicense |
| `whisper-rs-sys` | 0.15.0 | Unlicense |
| `winapi-util` | 0.1.11 | Unlicense OR MIT |
| `windowfunctions` | 0.1.1 | MIT |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 |
| `windows-sys` | 0.52.0 | MIT OR Apache-2.0 |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 |
| `windows-targets` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_aarch64_gnullvm` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_aarch64_msvc` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_i686_gnu` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_i686_gnullvm` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_i686_msvc` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_x86_64_gnu` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_x86_64_gnullvm` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_x86_64_msvc` | 0.52.6 | MIT OR Apache-2.0 |
| `winnow` | 0.7.15 | MIT |
| `winnow` | 1.0.4 | MIT |
| `wl-clipboard-rs` | 0.9.3 | MIT OR Apache-2.0 |
| `writeable` | 0.6.4 | Unicode-3.0 |
| `wyz` | 0.5.1 | MIT |
| `x11-clipboard` | 0.9.3 | MIT |
| `x11rb` | 0.13.2 | MIT OR Apache-2.0 |
| `x11rb` | 0.14.0 | MIT OR Apache-2.0 |
| `x11rb-protocol` | 0.13.2 | MIT OR Apache-2.0 |
| `x11rb-protocol` | 0.14.0 | MIT OR Apache-2.0 |
| `xkbcommon` | 0.9.0 | MIT |
| `xkeysym` | 0.2.1 | MIT OR Apache-2.0 OR Zlib |
| `yoke` | 0.8.3 | Unicode-3.0 |
| `yoke-derive` | 0.8.2 | Unicode-3.0 |
| `zbus` | 5.13.2 | MIT |
| `zbus-lockstep` | 0.5.2 | MIT |
| `zbus-lockstep-macros` | 0.5.2 | MIT |
| `zbus_macros` | 5.13.2 | MIT |
| `zbus_names` | 4.3.1 | MIT |
| `zbus_xml` | 5.1.0 | MIT |
| `zerofrom` | 0.1.8 | Unicode-3.0 |
| `zerofrom-derive` | 0.1.7 | Unicode-3.0 |
| `zeroize` | 1.9.0 | Apache-2.0 OR MIT |
| `zerotrie` | 0.2.5 | Unicode-3.0 |
| `zerovec` | 0.11.8 | Unicode-3.0 |
| `zerovec-derive` | 0.11.6 | Unicode-3.0 |
| `zmij` | 1.0.23 | MIT |
| `zvariant` | 5.9.2 | MIT |
| `zvariant_derive` | 5.9.2 | MIT |
| `zvariant_utils` | 3.3.0 | MIT |
<!-- crates:end -->

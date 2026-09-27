//! Builds NeMo-Speech.cpp's diarization library from its pinned source.
//! Three archives are fetched into `OUT_DIR`, verified against the
//! checksums below and unpacked: NeMo-Speech.cpp at the commit that added
//! Nemotron 3 Diarization, the ggml commit it pins as a submodule, and the
//! SentencePiece commit its ASR library links statically (Arch ships no
//! SentencePiece package). CMake builds SentencePiece as a static
//! archive, then NeMo-Speech.cpp with only its ASR runtime library and C
//! ABI (TTS, CLI and translation off), on stock ggml
//! (`NEMO_SPEECH_GGML_PATCHED=OFF`), static, with the CPU backend always
//! and the ggml Vulkan or CUDA backend under the matching feature.
//!
//! The result is two shared libraries, `libnemo_speech_asr_c.so.1` (the
//! stable C ABI) and the `libnemo_speech_asr.so` it needs, with ggml
//! linked into them. They are copied to `OUT_DIR/lib`, linked dynamically
//! and exported as `DEP_NEMO_SPEECH_LIB_DIR`, so the engine sets its own
//! run path and this ggml copy lives in that one process.
//!
//! `NEMO_SPEECH_CPP_ARCHIVE_DIR` names a directory holding the archives
//! already downloaded. `CMAKE_*`, `GGML_*` and `Vulkan_*` variables in the
//! environment are forwarded to CMake (ADR 0067's instruction baseline
//! arrives this way).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "fetch.rs"]
mod fetch;

use fetch::Archive;

/// The NeMo-Speech.cpp commit: `main` after "feat(diar): make Nemotron 3
/// Diarization the default diarizer (#52)", 2026-09-24; v0.1.0 predates
/// the model.
const NEMO_COMMIT: &str = "97a15afa5caa9bce5baaa86c1184103877af4101";

const ARCHIVES: &[Archive] = &[
    Archive {
        name: "nemo-speech-cpp-97a15afa5caa.tar.gz",
        url: "https://github.com/NVIDIA/NeMo-Speech.cpp/archive/97a15afa5caa9bce5baaa86c1184103877af4101.tar.gz",
        sha256: "046a576e673f201381a881aad221d62ebcfc632154d1d636a6a231a4a378cba8",
        into: "nemo-speech",
    },
    // The ggml submodule commit of that tree (ggml 0.12.0).
    Archive {
        name: "ggml-c03b4e2bcece.tar.gz",
        url: "https://github.com/ggml-org/ggml/archive/c03b4e2bcece5134827881af90242086daf75be5.tar.gz",
        sha256: "7f1fe40bdd9d75bc675dab06c0436308a69556b71125bd1c9c668b26a641d94c",
        into: "nemo-speech/ggml",
    },
    // The commit NeMo-Speech.cpp's scripts/build_sentencepiece_static.sh pins.
    Archive {
        name: "sentencepiece-17d7580d6407.tar.gz",
        url: "https://github.com/google/sentencepiece/archive/17d7580d6407802f85855d2cc9190634e2c95624.tar.gz",
        sha256: "7710982d3b438e790646f8be623f3bbe6af1e2f2619129d1f946d4070d26bc34",
        into: "sentencepiece",
    },
];

/// The runtime libraries, as the loader names them.
const LIBRARIES: &[&str] = &["libnemo_speech_asr_c.so.1", "libnemo_speech_asr.so"];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=fetch.rs");
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-env-changed=NEMO_SPEECH_CPP_ARCHIVE_DIR");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let archives = env::var_os("NEMO_SPEECH_CPP_ARCHIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| out.join("archives"));
    let source = fetch::sources(&out.join("source"), &archives, ARCHIVES);
    let nemo = source.join("nemo-speech");
    let prefix = sentencepiece(&source.join("sentencepiece"), &out);
    let lib = compile(&nemo, &prefix, &out);
    println!("cargo:lib_dir={}", lib.display());
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=dylib=nemo_speech_asr_c");
    println!("cargo:rustc-link-lib=dylib=nemo_speech_asr");
    // This crate's own test binaries find the libraries where they were
    // copied, outside Cargo's library path too; the engine binary sets its
    // run path in its own build.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
    bind(&nemo, &out);
}

/// Forwards the CMake, ggml and Vulkan variables of the environment.
fn forward_env(config: &mut cmake::Config) {
    for (key, value) in env::vars() {
        if key.starts_with("GGML_") || key.starts_with("Vulkan_") || key.starts_with("CMAKE_") {
            println!("cargo:rerun-if-env-changed={key}");
            config.define(&key, &value);
        }
    }
}

/// Builds the static SentencePiece archive and lays it out as the
/// dependency prefix NeMo-Speech.cpp's CMake searches.
fn sentencepiece(source: &Path, out: &Path) -> PathBuf {
    let mut config = cmake::Config::new(source);
    config
        .out_dir(out.join("sentencepiece"))
        .profile("Release")
        .define("CMAKE_BUILD_TYPE", "Release")
        // The pinned tree predates CMake 4's minimum policy version.
        .define("CMAKE_POLICY_VERSION_MINIMUM", "3.5")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("SPM_BUILD_TEST", "OFF")
        .define("SPM_ENABLE_SHARED", "OFF")
        .define("SPM_ENABLE_TCMALLOC", "OFF")
        // Its headers use uint32_t without <cstdint>, which GCC 15 and
        // later no longer include transitively.
        .cxxflag("-include cstdint")
        .build_target("sentencepiece-static")
        .pic(true);
    forward_env(&mut config);
    let built = config.build().join("build/src/libsentencepiece.a");
    let prefix = out.join("deps/sentencepiece");
    fs::create_dir_all(prefix.join("lib")).expect("prefix lib");
    fs::create_dir_all(prefix.join("include")).expect("prefix include");
    fs::copy(&built, prefix.join("lib/libsentencepiece.a"))
        .unwrap_or_else(|e| panic!("{}: {e}", built.display()));
    fs::copy(
        source.join("src/sentencepiece_processor.h"),
        prefix.join("include/sentencepiece_processor.h"),
    )
    .expect("sentencepiece header");
    out.join("deps")
}

fn on(enabled: bool) -> &'static str {
    if enabled { "ON" } else { "OFF" }
}

/// Configures and builds the C ABI library; returns `OUT_DIR/lib` holding
/// both runtime libraries and the link name.
fn compile(source: &Path, deps: &Path, out: &Path) -> PathBuf {
    let mut config = cmake::Config::new(source);
    config
        .out_dir(out.join("nemo-speech"))
        .profile("Release")
        .define("CMAKE_BUILD_TYPE", "Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("NEMO_SPEECH_DEPENDENCY_PREFIX", deps)
        .define("NEMO_SPEECH_BUILD_ASR", "OFF")
        .define("NEMO_SPEECH_BUILD_DIAR", "ON")
        .define("NEMO_SPEECH_BUILD_TTS", "OFF")
        .define("NEMO_SPEECH_BUILD_NMT", "OFF")
        .define("NEMO_SPEECH_BUILD_CLI", "OFF")
        .define("NEMO_SPEECH_BUILD_MIC_CAPTURE", "OFF")
        .define("NEMO_SPEECH_BUILD_TESTS", "OFF")
        .define("NEMO_SPEECH_GGML_PATCHED", "OFF")
        .define("GGML_NATIVE", "OFF")
        .define("GGML_OPENMP", "OFF")
        .define("GGML_VULKAN", on(cfg!(feature = "vulkan")))
        .define("GGML_CUDA", on(cfg!(feature = "cuda")))
        // The libraries are copied out of the build tree, never installed,
        // so they carry the install run path (upstream's `$ORIGIN` first).
        // The build run path is padded with empty entries for install to
        // rewrite, and the loader reads an empty entry as the working
        // directory.
        .define("CMAKE_BUILD_WITH_INSTALL_RPATH", "ON")
        .build_target("nemo_speech_asr_c")
        .pic(true);
    forward_env(&mut config);
    let bin = config.build().join("build/bin");
    let lib = out.join("lib");
    fs::create_dir_all(&lib).expect("lib directory");
    for name in LIBRARIES {
        let from = bin.join(name);
        fs::copy(&from, lib.join(name)).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    }
    let link = lib.join("libnemo_speech_asr_c.so");
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink("libnemo_speech_asr_c.so.1", &link).expect("link name");
    lib
}

fn bind(source: &Path, out: &Path) {
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", source.join("include").display()))
        .clang_arg(format!("-I{}", source.join("ggml/include").display()))
        .allowlist_function("nemo_speech_diar_.*")
        .allowlist_function("nemo_speech_asr_last_error")
        .allowlist_function("nemo_speech_asr_version")
        .allowlist_function("ggml_backend_dev_(count|get|type|description)")
        .allowlist_type("nemo_speech_diar_.*")
        .allowlist_type("nemo_speech_asr_status")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen over nemo_speech/diar.h");
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("write bindings.rs");
    println!("cargo:rustc-env=NEMO_SPEECH_CPP_COMMIT={NEMO_COMMIT}");
}

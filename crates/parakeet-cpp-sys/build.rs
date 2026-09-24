//! Builds parakeet.cpp from its pinned source: the release archive and the
//! ggml archive it vendors as a submodule are fetched into `OUT_DIR`,
//! verified against the checksums below, unpacked, patched with the
//! upstream CPU patch, compiled through CMake (static, CPU always, the
//! ggml Vulkan backend under the `vulkan` feature) and bound with bindgen
//! over `parakeet_capi.h` plus this crate's shim.
//!
//! `PARAKEET_CPP_SOURCE_DIR` points the build at an unpacked source tree
//! (with `third_party/ggml` populated) instead of fetching;
//! `PARAKEET_CPP_ARCHIVE_DIR` names a directory holding the two archives
//! already downloaded. `CMAKE_*`, `GGML_*` and `Vulkan_*` variables in the
//! environment are forwarded to CMake.

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "patch.rs"]
mod patch;

/// The pinned parakeet.cpp release.
const PARAKEET_VERSION: &str = "0.5.0";
/// Its commit (tag `v0.5.0`).
const PARAKEET_COMMIT: &str = "1bfbebfaaf493866f49597cd3b7901959d395c60";
const PARAKEET_URL: &str = "https://github.com/mudler/parakeet.cpp/archive/refs/tags/v0.5.0.tar.gz";
const PARAKEET_SHA256: &str = "f0066d2009fbce85fb33d353c3d85aad8fe124a22ab40a3835e0f8095cae4896";
/// The ggml commit the release pins as its submodule (ggml 0.13.0).
const GGML_COMMIT: &str = "e705c5fed490514458bdd2eaddc43bd098fcce9b";
const GGML_URL: &str =
    "https://github.com/ggml-org/ggml/archive/e705c5fed490514458bdd2eaddc43bd098fcce9b.tar.gz";
const GGML_SHA256: &str = "77eacb7cc20c2e47ffbaa5a69fe6f9fd9da0bea2a6431b7fae1a8283cb2c4e43";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=shim.h");
    println!("cargo:rerun-if-changed=shim.cpp");
    println!("cargo:rerun-if-env-changed=PARAKEET_CPP_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=PARAKEET_CPP_ARCHIVE_DIR");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let (source, owned) = match env::var_os("PARAKEET_CPP_SOURCE_DIR") {
        Some(dir) => (PathBuf::from(dir), false),
        None => (fetch_sources(&out), true),
    };
    // The fetched tree is patched in place once; a supplied tree is only
    // checked, so a caller's checkout is never written to.
    match patch::prepare(&source, owned) {
        Ok(patch::Outcome::NoPatch) => {
            println!("cargo:warning=parakeet.cpp: no ggml CPU patch found; building unpatched");
        }
        Ok(patch::Outcome::NoPatchTool) => {
            println!("cargo:warning=parakeet.cpp: patch is not installed; building unpatched");
        }
        Ok(_) => {}
        Err(why) => panic!("parakeet.cpp: {why}"),
    }
    let build = compile(&source, &out);
    let shim = compile_shim(&source);
    link(&out, &build, &shim);
    bind(&source, &out);
}

/// Fetches, verifies and unpacks both archives under `OUT_DIR/source`;
/// a marker file makes it a no-op on the next build.
fn fetch_sources(out: &Path) -> PathBuf {
    let root = out.join("source");
    let marker = root.join(format!(".unpacked-{PARAKEET_COMMIT}-{GGML_COMMIT}"));
    if marker.is_file() {
        return root;
    }
    let archives = env::var_os("PARAKEET_CPP_ARCHIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| out.join("archives"));
    fs::create_dir_all(&archives).expect("archive directory");
    let parakeet = archives.join(format!("parakeet.cpp-{PARAKEET_VERSION}.tar.gz"));
    let ggml = archives.join(format!("ggml-{GGML_COMMIT}.tar.gz"));
    fetch(PARAKEET_URL, &parakeet, PARAKEET_SHA256);
    fetch(GGML_URL, &ggml, GGML_SHA256);
    if root.exists() {
        fs::remove_dir_all(&root).expect("clean source directory");
    }
    fs::create_dir_all(&root).expect("source directory");
    untar(&parakeet, &root);
    let ggml_dir = root.join("third_party/ggml");
    fs::create_dir_all(&ggml_dir).expect("ggml directory");
    untar(&ggml, &ggml_dir);
    fs::write(&marker, format!("{PARAKEET_URL}\n{GGML_URL}\n")).expect("marker");
    root
}

/// Downloads `url` to `path` unless a file with the expected checksum is
/// already there; a checksum mismatch fails the build naming both hashes.
fn fetch(url: &str, path: &Path, sha256: &str) {
    if path.is_file() && sha256_of(path) == sha256 {
        return;
    }
    let part = path.with_extension("part");
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&part)
        .arg(url)
        .status()
        .unwrap_or_else(|e| panic!("curl {url}: {e}"));
    assert!(status.success(), "curl {url} failed with {status}");
    let have = sha256_of(&part);
    assert!(
        have == sha256,
        "{url} hashed {have}, expected {sha256}; left as {}",
        part.display()
    );
    fs::rename(&part, path).expect("move archive into place");
}

fn sha256_of(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).expect("read archive");
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    format!("{:x}", hasher.finalize())
}

/// Unpacks a GitHub archive (one top-level directory) into `dir`.
fn untar(archive: &Path, dir: &Path) {
    let status = Command::new("tar")
        .args(["xzf"])
        .arg(archive)
        .args(["--strip-components=1", "-C"])
        .arg(dir)
        .status()
        .unwrap_or_else(|e| panic!("tar {}: {e}", archive.display()));
    assert!(
        status.success(),
        "tar {} failed with {status}",
        archive.display()
    );
}

/// Configures and builds the static libraries; returns the CMake build
/// directory (libparakeet.a has no install rule and stays there).
fn compile(source: &Path, out: &Path) -> PathBuf {
    let mut config = cmake::Config::new(source);
    config
        .profile("Release")
        .define("CMAKE_BUILD_TYPE", "Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("PARAKEET_SHARED", "OFF")
        .define("PARAKEET_BUILD_CLI", "OFF")
        .define("PARAKEET_BUILD_SERVER", "OFF")
        .define("PARAKEET_BUILD_TESTS", "OFF")
        .define("PARAKEET_VERSION", PARAKEET_VERSION)
        .define("GGML_NATIVE", "OFF")
        .define("GGML_OPENMP", "OFF")
        .define("GGML_BUILD_TESTS", "OFF")
        .define("GGML_BUILD_EXAMPLES", "OFF")
        .define(
            "PARAKEET_GGML_VULKAN",
            if cfg!(feature = "vulkan") {
                "ON"
            } else {
                "OFF"
            },
        )
        .pic(true);
    for (key, value) in env::vars() {
        if key.starts_with("GGML_") || key.starts_with("Vulkan_") || key.starts_with("CMAKE_") {
            println!("cargo:rerun-if-env-changed={key}");
            config.define(&key, &value);
        }
    }
    config.build();
    out.join("build")
}

/// Compiles the shim against the private headers of the pinned source.
fn compile_shim(source: &Path) -> PathBuf {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .file("shim.cpp")
        .include(".")
        .include(source.join("include"))
        .include(source.join("src"))
        .include(source.join("third_party/ggml/include"))
        .define(
            "PARAKEET_SYS_SOURCE_VERSION",
            Some(format!("\"{PARAKEET_VERSION}\"").as_str()),
        )
        .warnings(false);
    build.compile("parakeet_sys_shim");
    PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"))
}

fn link(out: &Path, build: &Path, _shim: &Path) {
    for dir in [
        build.to_path_buf(),
        build.join("third_party/ggml/src"),
        build.join("third_party/ggml/src/ggml-cpu"),
        build.join("third_party/ggml/src/ggml-vulkan"),
        out.join("lib"),
        out.join("lib64"),
    ] {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    println!("cargo:rustc-link-lib=static=parakeet");
    println!("cargo:rustc-link-lib=static=ggml");
    println!("cargo:rustc-link-lib=static=ggml-base");
    println!("cargo:rustc-link-lib=static=ggml-cpu");
    if cfg!(feature = "vulkan") {
        println!("cargo:rustc-link-lib=static=ggml-vulkan");
        println!("cargo:rustc-link-lib=dylib=vulkan");
    }
    println!("cargo:rustc-link-lib=dylib=stdc++");
}

fn bind(source: &Path, out: &Path) {
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", source.join("include").display()))
        .clang_arg("-I.")
        .allowlist_function("parakeet_capi_.*")
        .allowlist_function("parakeet_sys_.*")
        .allowlist_type("parakeet_.*")
        .allowlist_var("PARAKEET_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen over parakeet_capi.h");
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("write bindings.rs");
}

//! Builds sherpa-onnx's C API from its pinned Linux x86-64 release with
//! calibrated clustering shared by CPU and CUDA. CUDA also applies the
//! checked convolution-search patch. ONNX Runtime and provider libraries
//! stay from the verified release archives. All
//! runtime libraries are placed under `OUT_DIR/lib`. The engine links the C API
//! dynamically and sets its own run path from the `lib_dir` this script
//! exports (`DEP_SHERPA_ONNX_LIB_DIR`), so ONNX Runtime is loaded by that
//! one process and nothing else in the workspace. Bindgen runs over
//! `c-api.h`, allowlisting the offline speaker diarization surface.
//!
//! `SHERPA_ONNX_DIST_DIR` points the build at an unpacked release
//! directory (with `include/` and `lib/`) instead of fetching;
//! `SHERPA_ONNX_ARCHIVE_DIR` names a directory holding the archive already
//! downloaded.

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

mod source_build;

/// The pinned sherpa-onnx release.
const VERSION: &str = "1.13.7";
#[cfg(not(feature = "cuda"))]
const ARCHIVE: &str = "sherpa-onnx-v1.13.7-linux-x64-shared.tar.bz2";
#[cfg(not(feature = "cuda"))]
const SHA256: &str = "95b9f4358e2d5522a0bd987c3ee91128e31f8abf4340927cab32e438300f3c24";
#[cfg(feature = "cuda")]
const ARCHIVE: &str =
    "sherpa-onnx-v1.13.7-cuda-13.x-cudnn-9.x-onnxruntime1.27.1-linux-x64-gpu.tar.bz2";
#[cfg(feature = "cuda")]
const SHA256: &str = "0ce8db69d12c6e750647e2fde39f8b6281baebb42f24f53f07be5605ca8dbdd2";
/// The libraries the engine needs at run time, in the archive's `lib/`.
const LIBRARIES: &[&str] = &["libsherpa-onnx-c-api.so", "libonnxruntime.so"];
#[cfg(feature = "cuda")]
const PROVIDERS: &[&str] = &[
    "libonnxruntime_providers_shared.so",
    "libonnxruntime_providers_cuda.so",
];
#[cfg(not(feature = "cuda"))]
const PROVIDERS: &[&str] = &[];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_DIST_DIR");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_ARCHIVE_DIR");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let dist = match env::var_os("SHERPA_ONNX_DIST_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => fetch_dist(&out),
    };
    let lib = out.join("lib");
    fs::create_dir_all(&lib).expect("lib directory");
    for name in LIBRARIES.iter().chain(PROVIDERS) {
        let src = dist.join("lib").join(name);
        assert!(src.is_file(), "{} is not in the release", src.display());
        fs::copy(&src, lib.join(name)).unwrap_or_else(|e| panic!("copy {name}: {e}"));
    }
    source_build::build(&out, &lib);
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=dylib=sherpa-onnx-c-api");
    // This crate's own test binaries find the library where it was
    // unpacked; the engine binary sets its run path in its own build.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
    println!("cargo:lib_dir={}", lib.display());
    println!("cargo:version={VERSION}");
    bind(&dist, &out);
}

/// Fetches, verifies and unpacks the release under `OUT_DIR/dist`; a
/// marker file makes it a no-op on the next build.
fn fetch_dist(out: &Path) -> PathBuf {
    let root = out.join("dist");
    let marker = root.join(format!(".unpacked-{SHA256}"));
    if marker.is_file() {
        return root;
    }
    let archives = env::var_os("SHERPA_ONNX_ARCHIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| out.join("archives"));
    fs::create_dir_all(&archives).expect("archive directory");
    let archive = archives.join(ARCHIVE);
    let url =
        format!("https://github.com/k2-fsa/sherpa-onnx/releases/download/v{VERSION}/{ARCHIVE}");
    fetch(&url, &archive, SHA256);
    if root.exists() {
        fs::remove_dir_all(&root).expect("clean dist directory");
    }
    fs::create_dir_all(&root).expect("dist directory");
    let status = Command::new("tar")
        .args(["xjf"])
        .arg(&archive)
        .args(["--strip-components=1", "-C"])
        .arg(&root)
        .status()
        .unwrap_or_else(|e| panic!("tar {}: {e}", archive.display()));
    assert!(status.success(), "tar {ARCHIVE} failed with {status}");
    fs::write(&marker, format!("{url}\n")).expect("marker");
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

fn bind(dist: &Path, out: &Path) {
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", dist.join("include").display()))
        .allowlist_function("SherpaOnnx.*SpeakerDiarization.*")
        .allowlist_function("SherpaOnnxGetVersionStr")
        .allowlist_type("SherpaOnnx.*SpeakerDiarization.*")
        .allowlist_type("SherpaOnnx.*SpeakerSegmentation.*")
        .allowlist_type("SherpaOnnxSpeakerEmbeddingExtractorConfig")
        .allowlist_type("SherpaOnnxFastClusteringConfig")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen over c-api.h");
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("write bindings.rs");
}

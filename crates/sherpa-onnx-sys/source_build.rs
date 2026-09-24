//! Builds the C API with the same calibrated clustering on CPU and CUDA.
//! ONNX Runtime and optional providers stay from the verified release archive.

use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCE_ARCHIVE: &str = "sherpa-onnx-v1.13.7-source.tar.gz";
const SOURCE_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/archive/refs/tags/v1.13.7.tar.gz";
const SOURCE_SHA256: &str = "ee0c20cafb34cc1f86afb2845babd941c26e46de4a9925cbe86fd55ff3557818";
const ORT_VERSION: &str = "1.27.1";
const PATCH: &str = "patches/cuda-conv-default.patch";
const CALIBRATION_PATCH: &str = "patches/diarization-calibration.patch";
const CLUSTERING_HEADER: &str = "calibrated_clustering.h";
const HEADERS: &[(&str, &str)] = &[
    (
        "onnxruntime_c_api.h",
        "523131bab81173fe287c578f7dda344e1dc5c749fa3f70b55fbbf8f210850313",
    ),
    (
        "onnxruntime_ep_c_api.h",
        "f65ac849718cabe3ff4902698dd2de643c3b3dbe0010242b2b0407548e7ee00c",
    ),
    (
        "onnxruntime_cxx_api.h",
        "c5f9bb73d7674cf99a197b383a144eb2a5eda71cc060193a86c7d24e6fa36ed7",
    ),
    (
        "onnxruntime_cxx_inline.h",
        "114b4d008351eaa4462560438474cebd104f74259cc8af607acc0b97453200d2",
    ),
    (
        "onnxruntime_float16.h",
        "88b242845d25981633a0bbd1c148e273cf8bfb016ea3f57c4af41a06530f72b0",
    ),
];

pub fn build(out: &Path, lib: &Path) {
    println!("cargo:rerun-if-changed=source_build.rs");
    println!("cargo:rerun-if-changed={PATCH}");
    println!("cargo:rerun-if-changed={CALIBRATION_PATCH}");
    println!("cargo:rerun-if-changed={CLUSTERING_HEADER}");
    println!("cargo:rerun-if-env-changed=CMAKE_BUILD_PARALLEL_LEVEL");
    let out = fs::canonicalize(out).expect("absolute build directory");
    let source = patched_source(&out);
    let headers = headers(&out);
    let build = out.join("source-build");
    let runtime = build.join("lib");
    fs::create_dir_all(&runtime).expect("sherpa build library directory");
    for name in std::iter::once(&"libonnxruntime.so").chain(super::PROVIDERS.iter()) {
        fs::copy(lib.join(name), runtime.join(name)).expect("stage pinned ONNX Runtime");
    }
    let mut configure = Command::new("cmake");
    configure
        .arg("-S")
        .arg(&source)
        .arg("-B")
        .arg(&build)
        .args([
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DBUILD_SHARED_LIBS=ON",
            "-DCMAKE_BUILD_WITH_INSTALL_RPATH=ON",
            "-DCMAKE_BUILD_RPATH_USE_ORIGIN=ON",
            "-DSHERPA_ONNX_ENABLE_C_API=ON",
            "-DSHERPA_ONNX_ENABLE_SPEAKER_DIARIZATION=ON",
            "-DSHERPA_ONNX_ENABLE_TTS=OFF",
            "-DSHERPA_ONNX_ENABLE_BINARY=OFF",
            "-DSHERPA_ONNX_BUILD_C_API_EXAMPLES=OFF",
            "-DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF",
            "-DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF",
            "-DSHERPA_ONNX_ENABLE_TESTS=OFF",
            "-DSHERPA_ONNX_ENABLE_PYTHON=OFF",
            "-DSHERPA_ONNX_ENABLE_JNI=OFF",
            "-DSHERPA_ONNX_USE_PRE_INSTALLED_ONNXRUNTIME_IF_AVAILABLE=ON",
        ])
        .arg(if cfg!(feature = "cuda") {
            "-DSHERPA_ONNX_ENABLE_GPU=ON"
        } else {
            "-DSHERPA_ONNX_ENABLE_GPU=OFF"
        })
        .env("SHERPA_ONNXRUNTIME_INCLUDE_DIR", &headers)
        .env("SHERPA_ONNXRUNTIME_LIB_DIR", &runtime)
        .env("GIT_CEILING_DIRECTORIES", &out);
    run(&mut configure, "configure sherpa C API");
    let jobs = env::var("CMAKE_BUILD_PARALLEL_LEVEL").unwrap_or_else(|_| "8".into());
    run(
        Command::new("cmake")
            .arg("--build")
            .arg(&build)
            .args(["--target", "sherpa-onnx-c-api", "--parallel", &jobs])
            .env("SHERPA_ONNXRUNTIME_INCLUDE_DIR", &headers)
            .env("SHERPA_ONNXRUNTIME_LIB_DIR", &runtime)
            .env("GIT_CEILING_DIRECTORIES", &out),
        "build sherpa C API",
    );
    let built = runtime.join("libsherpa-onnx-c-api.so");
    fs::copy(&built, lib.join("libsherpa-onnx-c-api.so")).expect("stage calibrated C API");
}

fn patched_source(out: &Path) -> PathBuf {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let mut patches = vec![root.join(CALIBRATION_PATCH)];
    if cfg!(feature = "cuda") {
        patches.push(root.join(PATCH));
    }
    let header = root.join(CLUSTERING_HEADER);
    let hashes: Vec<_> = patches
        .iter()
        .chain(std::iter::once(&header))
        .map(|p| super::sha256_of(p))
        .collect();
    let patch_hash = format!("{:x}", Sha256::digest(hashes.concat().as_bytes()));
    let source = out.join("sherpa-source");
    let marker = source.join(format!(".patched-{SOURCE_SHA256}-{patch_hash}"));
    if marker.is_file() {
        return source;
    }
    let archives = env::var_os("SHERPA_ONNX_ARCHIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| out.join("archives"));
    fs::create_dir_all(&archives).expect("source archive directory");
    let archive = archives.join(SOURCE_ARCHIVE);
    super::fetch(SOURCE_URL, &archive, SOURCE_SHA256);
    if source.exists() {
        fs::remove_dir_all(&source).expect("replace incomplete or outdated sherpa source");
    }
    fs::create_dir_all(&source).expect("sherpa source directory");
    run(
        Command::new("tar")
            .arg("xzf")
            .arg(&archive)
            .args(["--strip-components=1", "-C"])
            .arg(&source),
        "unpack sherpa source",
    );
    for patch in patches {
        run(
            Command::new("patch")
                .args([
                    "--batch",
                    "--forward",
                    "--fuzz=0",
                    "--no-backup-if-mismatch",
                    "-p1",
                    "-d",
                ])
                .arg(&source)
                .arg("-i")
                .arg(&patch),
            "apply exact sherpa patch",
        );
    }
    fs::copy(
        header,
        source.join("sherpa-onnx/csrc/dettivo-calibrated-clustering.h"),
    )
    .expect("stage calibrated clustering");
    fs::write(&marker, patch_hash).expect("sherpa patch marker");
    source
}

fn headers(out: &Path) -> PathBuf {
    let headers = out.join("ort-headers");
    fs::create_dir_all(&headers).expect("ONNX Runtime header directory");
    for (name, hash) in HEADERS {
        let url = format!(
            "https://raw.githubusercontent.com/microsoft/onnxruntime/v{ORT_VERSION}/include/onnxruntime/core/session/{name}"
        );
        super::fetch(&url, &headers.join(name), hash);
    }
    headers
}

fn run(command: &mut Command, description: &str) {
    let status = command
        .status()
        .unwrap_or_else(|e| panic!("{description}: {e}"));
    assert!(status.success(), "{description} failed with {status}");
}

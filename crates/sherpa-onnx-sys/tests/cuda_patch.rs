//! The source-patch contract belongs to the CUDA build, not the CPU default.

#[cfg(feature = "cuda")]
#[test]
fn cuda_patch_accepts_only_the_exact_unpatched_source() {
    let test = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/packaging/test-sherpa-cuda-patch.sh");
    let status = std::process::Command::new("bash")
        .arg(test)
        .status()
        .expect("run CUDA source patch regression");
    assert!(status.success(), "CUDA source patch regression: {status}");
}

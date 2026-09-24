//! Sets the engine binary's run path: `libsherpa-onnx-c-api.so` (which
//! carries `libonnxruntime.so` beside it) is found next to the binary, in
//! `../lib/dettivo` for a packaged install, or where `sherpa-onnx-sys`
//! unpacked the release for a build tree.

fn main() {
    println!("cargo:rerun-if-env-changed=DEP_SHERPA_ONNX_LIB_DIR");
    println!("cargo:rerun-if-env-changed=DETTIVO_PACKAGE");
    let lib = std::env::var("DEP_SHERPA_ONNX_LIB_DIR").expect("sherpa-onnx-sys exports lib_dir");
    if std::env::var("DETTIVO_PACKAGE").as_deref() == Ok("1") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    } else {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN:$ORIGIN/../lib/dettivo:{lib}");
    }
}

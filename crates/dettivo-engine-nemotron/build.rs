//! Sets the engine binary's run path: `libnemo_speech_asr_c.so.1` (which
//! finds `libnemo_speech_asr.so` beside itself) is found next to the
//! binary, in `../lib/dettivo` for a packaged install, or where
//! `nemo-speech-cpp-sys` built them for a build tree.

fn main() {
    println!("cargo:rerun-if-env-changed=DEP_NEMO_SPEECH_LIB_DIR");
    println!("cargo:rerun-if-env-changed=DETTIVO_PACKAGE");
    let lib =
        std::env::var("DEP_NEMO_SPEECH_LIB_DIR").expect("nemo-speech-cpp-sys exports lib_dir");
    if std::env::var("DETTIVO_PACKAGE").as_deref() == Ok("1") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    } else {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN:$ORIGIN/../lib/dettivo:{lib}");
    }
}

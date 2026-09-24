//! CUDA is optional at runtime: resolve its libraries and device before
//! entering sherpa's C API, which can throw across the FFI boundary.

#![allow(
    unsafe_code,
    reason = "optional CUDA libraries are inspected through dlopen"
)]

use std::ffi::{CStr, CString, c_void};
use std::path::{Path, PathBuf};
use std::process::Command;

use dettivo_engine_proto::{Backend, BackendPreference, EngineError};

pub struct Selection {
    pub backend: Backend,
    pub reason: String,
    pub fallback_reason: Option<String>,
    _libraries: Vec<Library>,
}

impl Selection {
    pub fn cpu(reason: impl Into<String>) -> Self {
        Self {
            backend: Backend::Cpu,
            reason: reason.into(),
            fallback_reason: None,
            _libraries: Vec::new(),
        }
    }
}

pub fn choose(preference: BackendPreference, force_cpu: bool) -> Result<Selection, EngineError> {
    select(preference, force_cpu, cfg!(feature = "cuda"), || {
        probe(None)
    })
}

fn select(
    preference: BackendPreference,
    force_cpu: bool,
    built: bool,
    probe: impl FnOnce() -> Result<Vec<Library>, String>,
) -> Result<Selection, EngineError> {
    if force_cpu || preference == BackendPreference::Cpu {
        return Ok(Selection::cpu(if force_cpu {
            "CPU forced by --cpu or DETTIVO_FORCE_CPU=1"
        } else {
            "provider = cpu"
        }));
    }
    if preference == BackendPreference::Vulkan {
        return Err(EngineError::new(
            "backend_unavailable",
            "diarization has no Vulkan provider",
        ));
    }
    let result = if built {
        probe()
    } else {
        Err("this build has no CUDA provider; install dettivo-engines-cuda".into())
    };
    match result {
        Ok(libraries) => Ok(Selection {
            backend: Backend::Cuda,
            reason: "CUDA provider libraries resolved and a CUDA device is available".into(),
            fallback_reason: None,
            _libraries: libraries,
        }),
        Err(reason) if preference == BackendPreference::Cuda => {
            Err(EngineError::new("backend_unavailable", reason))
        }
        Err(reason) => Ok(Selection {
            fallback_reason: Some(reason.clone()),
            ..Selection::cpu(reason)
        }),
    }
}

/// Handles remain loaded until the model is destroyed.
pub struct Library(*mut c_void);

impl Library {
    fn open(name: &str) -> Result<Self, String> {
        let c_name = CString::new(name).map_err(|e| format!("{name}: {e}"))?;
        // SAFETY: the name is NUL terminated; dlopen's handle is owned here.
        let handle = unsafe { libc::dlopen(c_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        if handle.is_null() {
            // SAFETY: dlerror returns a thread-local string or null.
            let detail = unsafe {
                let error = libc::dlerror();
                if error.is_null() {
                    "unavailable".into()
                } else {
                    CStr::from_ptr(error).to_string_lossy().into_owned()
                }
            };
            Err(format!("{name}: {detail}"))
        } else {
            Ok(Self(handle))
        }
    }

    fn symbol(&self, name: &CStr) -> Result<*mut c_void, String> {
        // SAFETY: the handle is live and the symbol name is NUL terminated.
        let symbol = unsafe { libc::dlsym(self.0, name.as_ptr()) };
        if symbol.is_null() {
            Err(format!("missing symbol {}", name.to_string_lossy()))
        } else {
            Ok(symbol)
        }
    }

    fn directory(&self) -> Result<PathBuf, String> {
        let symbol = self.symbol(c"OrtGetApiBase")?;
        // SAFETY: the function symbol is in this live library; dladdr
        // fills Dl_info and its filename remains valid while loaded.
        unsafe {
            let mut info: libc::Dl_info = std::mem::zeroed();
            if libc::dladdr(symbol, &mut info) == 0 || info.dli_fname.is_null() {
                return Err("libonnxruntime.so: cannot locate the loaded library".into());
            }
            Path::new(
                CStr::from_ptr(info.dli_fname)
                    .to_str()
                    .map_err(|e| e.to_string())?,
            )
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "libonnxruntime.so has no parent directory".into())
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: this is the live handle returned by dlopen, closed once.
        unsafe { libc::dlclose(self.0) };
    }
}

fn probe(provider_directory: Option<&Path>) -> Result<Vec<Library>, String> {
    let ort = Library::open("libonnxruntime.so")?;
    let directory = match provider_directory {
        Some(directory) => directory.to_path_buf(),
        None => ort.directory()?,
    };
    provider_dependencies(&directory)?;
    ort.symbol(c"OrtSessionOptionsAppendExecutionProvider_CUDA")
        .map_err(|why| format!("libonnxruntime.so has no CUDA provider: {why}"))?;
    let cudart = Library::open("libcudart.so.13")?;
    let get_count = cudart.symbol(c"cudaGetDeviceCount")?;
    let mut count = 0;
    // SAFETY: cudaGetDeviceCount has this CUDA runtime ABI; its library
    // remains live through this call and the model's entire lifetime.
    let status = unsafe {
        let get_count: unsafe extern "C" fn(*mut i32) -> i32 = std::mem::transmute(get_count);
        get_count(&mut count)
    };
    if status != 0 || count == 0 {
        return Err(format!(
            "libcudart.so.13: cudaGetDeviceCount returned {status}, devices={count}"
        ));
    }
    Ok(vec![ort, cudart])
}

fn provider_dependencies(directory: &Path) -> Result<(), String> {
    for name in [
        "libonnxruntime_providers_shared.so",
        "libonnxruntime_providers_cuda.so",
    ] {
        let path = directory.join(name);
        if !path.is_file() {
            return Err(format!("{}: missing provider library", path.display()));
        }
        // ORT must initialize Provider_SetHost before provider constructors
        // run. glibc's dependency trace resolves DT_NEEDED without running them.
        let output = Command::new("ldd")
            .arg(&path)
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| format!("{}: ldd dependency check failed: {e}", path.display()))?;
        let dependencies = String::from_utf8_lossy(&output.stdout);
        let missing: Vec<_> = dependencies
            .lines()
            .filter(|line| line.contains("not found"))
            .map(str::trim)
            .collect();
        if !missing.is_empty() {
            return Err(format!("{}: {}", path.display(), missing.join("; ")));
        }
        if !output.status.success() {
            return Err(format!(
                "{}: ldd failed: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_provider_libraries_fall_back_only_for_auto() {
        let hidden = tempfile::tempdir().unwrap();
        for preference in [BackendPreference::Auto, BackendPreference::Cuda] {
            let result = select(preference, false, true, || probe(Some(hidden.path())));
            let reason = if preference == BackendPreference::Auto {
                let selected = result.unwrap();
                assert_eq!(selected.backend, Backend::Cpu);
                selected.fallback_reason.unwrap()
            } else {
                result.err().unwrap().message
            };
            assert!(
                reason.contains("libonnxruntime_providers_shared.so"),
                "{reason}"
            );
            assert!(reason.contains(hidden.path().to_str().unwrap()), "{reason}");
        }
    }

    #[test]
    fn cpu_pins_never_probe_and_unsupported_providers_are_refused() {
        for (preference, forced) in [
            (BackendPreference::Cpu, false),
            (BackendPreference::Cuda, true),
        ] {
            let selected = select(preference, forced, true, || {
                panic!("CPU must not probe CUDA")
            })
            .unwrap();
            assert_eq!(selected.backend, Backend::Cpu);
            assert!(selected.fallback_reason.is_none());
        }
        for preference in [BackendPreference::Vulkan, BackendPreference::Cuda] {
            assert!(select(preference, false, false, || panic!("no GPU build")).is_err());
        }
    }
}

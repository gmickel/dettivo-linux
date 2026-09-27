//! Which backend a load runs on. The build carries at most one ggml GPU
//! backend (Vulkan in the default package, CUDA in the drop-in); `auto`
//! tries it and falls back to the CPU naming why, an explicit preference
//! fails the load instead, and `cpu` or `DETTIVO_FORCE_CPU=1` never
//! touches a GPU.

use dettivo_engine_proto::{Backend, BackendPreference, EngineError};

/// The GPU backend this build carries.
pub const BUILT: Option<Backend> = if cfg!(feature = "cuda") {
    Some(Backend::Cuda)
} else if cfg!(feature = "vulkan") {
    Some(Backend::Vulkan)
} else {
    None
};

/// The backend a load ended up on, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The backend.
    pub backend: Backend,
    /// Why that one.
    pub reason: String,
    /// Why an automatic load is on the CPU instead of the GPU.
    pub fallback_reason: Option<String>,
}

fn name(backend: Backend) -> &'static str {
    match backend {
        Backend::Vulkan => "Vulkan",
        Backend::Cuda => "CUDA",
        Backend::Cpu => "CPU",
    }
}

fn cpu<M>(
    open: &mut impl FnMut(i32) -> Result<M, String>,
    reason: String,
    fallback_reason: Option<String>,
) -> Result<(M, Selection), EngineError> {
    let model = open(-1).map_err(|why| EngineError::new("load_failed", why))?;
    Ok((
        model,
        Selection {
            backend: Backend::Cpu,
            reason,
            fallback_reason,
        },
    ))
}

/// Loads through `open` (`-1` is the CPU, `0` the first GPU of the
/// build's backend `built`) under `preference`.
pub fn load<M>(
    preference: BackendPreference,
    force_cpu: bool,
    built: Option<Backend>,
    mut open: impl FnMut(i32) -> Result<M, String>,
) -> Result<(M, Selection), EngineError> {
    if force_cpu {
        return cpu(
            &mut open,
            "CPU forced by --cpu or DETTIVO_FORCE_CPU=1".into(),
            None,
        );
    }
    let wanted = match preference {
        BackendPreference::Cpu => return cpu(&mut open, "backend = cpu".into(), None),
        BackendPreference::Vulkan => Some(Backend::Vulkan),
        BackendPreference::Cuda => Some(Backend::Cuda),
        BackendPreference::Auto => None,
    };
    let Some(gpu) = built else {
        let why = "this build of dettivo-engine-nemotron has no GPU backend".to_string();
        return match wanted {
            Some(w) => Err(EngineError::new(
                "backend_unavailable",
                format!("{why}, so {} is unavailable", name(w)),
            )),
            None => cpu(&mut open, why.clone(), Some(why)),
        };
    };
    if let Some(w) = wanted.filter(|w| *w != gpu) {
        let hint = if w == Backend::Cuda {
            "; install dettivo-engines-cuda"
        } else {
            ""
        };
        return Err(EngineError::new(
            "backend_unavailable",
            format!(
                "this build of dettivo-engine-nemotron carries {}, not {}{hint}",
                name(gpu),
                name(w)
            ),
        ));
    }
    match open(0) {
        Ok(model) => Ok((
            model,
            Selection {
                backend: gpu,
                reason: format!("ggml {} device 0 loaded the model", name(gpu)),
                fallback_reason: None,
            },
        )),
        Err(why) if wanted.is_none() => {
            let why = format!("the {} load failed: {why}", name(gpu));
            tracing::warn!(reason = %why, "falling back to the CPU");
            cpu(&mut open, why.clone(), Some(why))
        }
        Err(why) => Err(EngineError::new("backend_unavailable", why)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A loader that fails on the GPU with `gpu_error` and records the
    /// devices it was asked for.
    fn loader<'a>(
        asked: &'a mut Vec<i32>,
        gpu_error: Option<&'a str>,
    ) -> impl FnMut(i32) -> Result<i32, String> + 'a {
        move |device| {
            asked.push(device);
            match (device, gpu_error) {
                (0, Some(why)) => Err(why.to_string()),
                _ => Ok(device),
            }
        }
    }

    #[test]
    fn a_failed_gpu_load_falls_back_to_the_cpu_only_for_auto() {
        let mut asked = Vec::new();
        let (device, selection) = load(
            BackendPreference::Auto,
            false,
            Some(Backend::Vulkan),
            loader(&mut asked, Some("no matching GPU device found")),
        )
        .unwrap();
        assert_eq!((device, asked), (-1, vec![0, -1]));
        assert_eq!(selection.backend, Backend::Cpu);
        let reason = selection.fallback_reason.unwrap();
        assert!(
            reason.contains("Vulkan load failed: no matching GPU"),
            "{reason}"
        );

        let mut asked = Vec::new();
        let err = load(
            BackendPreference::Vulkan,
            false,
            Some(Backend::Vulkan),
            loader(&mut asked, Some("out of device memory")),
        )
        .err()
        .unwrap();
        assert_eq!((err.code, asked), ("backend_unavailable", vec![0]));
        assert!(err.message.contains("out of device memory"));
    }

    #[test]
    fn the_gpu_loads_when_it_works_and_cpu_pins_never_touch_it() {
        let mut asked = Vec::new();
        let (_, selection) = load(
            BackendPreference::Auto,
            false,
            Some(Backend::Cuda),
            loader(&mut asked, None),
        )
        .unwrap();
        assert_eq!((selection.backend, asked), (Backend::Cuda, vec![0]));
        assert!(selection.fallback_reason.is_none());
        for (preference, forced) in [
            (BackendPreference::Cpu, false),
            (BackendPreference::Cuda, true),
        ] {
            let mut asked = Vec::new();
            let (_, selection) = load(
                preference,
                forced,
                Some(Backend::Cuda),
                loader(&mut asked, None),
            )
            .unwrap();
            assert_eq!((selection.backend, asked), (Backend::Cpu, vec![-1]));
            assert!(selection.fallback_reason.is_none());
        }
    }

    #[test]
    fn a_backend_the_build_lacks_is_refused_and_auto_says_why_it_is_on_the_cpu() {
        for (preference, built) in [
            (BackendPreference::Cuda, Some(Backend::Vulkan)),
            (BackendPreference::Vulkan, Some(Backend::Cuda)),
            (BackendPreference::Vulkan, None),
        ] {
            let mut asked = Vec::new();
            let err = load(preference, false, built, loader(&mut asked, None))
                .err()
                .unwrap();
            assert_eq!((err.code, asked.len()), ("backend_unavailable", 0));
        }
        let mut asked = Vec::new();
        let (_, selection) = load(
            BackendPreference::Auto,
            false,
            None,
            loader(&mut asked, None),
        )
        .unwrap();
        assert_eq!((selection.backend, asked), (Backend::Cpu, vec![-1]));
        assert!(
            selection
                .fallback_reason
                .unwrap()
                .contains("no GPU backend")
        );
    }
}

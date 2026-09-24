//! Backend selection (FR-E9), shared by the ggml engines: Vulkan when the
//! build carries the Vulkan backend, a device is installed and CPU was not
//! forced; CPU otherwise, with the reason reported in `loaded` and by
//! `dettivo doctor`. `auto` falls back to the CPU with the reason;
//! `vulkan` is Vulkan or a refusal, never a quiet CPU load.

use std::path::Path;

use crate::messages::{Backend, BackendPreference};

/// True when a Vulkan installable client driver is present.
pub fn vulkan_device_present() -> bool {
    ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"]
        .iter()
        .any(|d| {
            std::fs::read_dir(Path::new(d))
                .map(|mut e| e.next().is_some())
                .unwrap_or(false)
        })
}

/// Decides the backend to try first and why; `vulkan_built_in` says
/// whether the calling binary was compiled with ggml's Vulkan backend.
/// `auto` answers the CPU with the reason when Vulkan is out; `vulkan`
/// answers `Err` with that reason instead, so the load fails rather
/// than running on the CPU under a preference that asked for the GPU.
/// `DETTIVO_FORCE_CPU=1` (the QA switch) wins over both.
pub fn choose(
    preference: BackendPreference,
    force_cpu: bool,
    vulkan_built_in: bool,
) -> Result<(Backend, String), String> {
    if force_cpu {
        return Ok((Backend::Cpu, "DETTIVO_FORCE_CPU=1".into()));
    }
    if preference == BackendPreference::Cuda {
        return Err("this ggml engine does not support the CUDA provider".into());
    }
    let vulkan_out = if preference == BackendPreference::Cpu {
        return Ok((Backend::Cpu, "backend_preference = cpu".into()));
    } else if !vulkan_built_in {
        Some("this build has no Vulkan backend".to_string())
    } else if !vulkan_device_present() {
        Some("no Vulkan device (no ICD under /usr/share/vulkan/icd.d or /etc/vulkan/icd.d)".into())
    } else {
        None
    };
    match (vulkan_out, preference) {
        (None, _) => Ok((
            Backend::Vulkan,
            "Vulkan backend built in and a device is installed".into(),
        )),
        (Some(why), BackendPreference::Vulkan) => Err(why),
        (Some(why), _) => Ok((Backend::Cpu, why)),
    }
}

/// The refusal an engine answers when `backend_preference = vulkan` and
/// the load did not land on a Vulkan device: the reason, never a CPU
/// result.
pub fn strict_vulkan_refused(why: &str) -> String {
    format!("backend_preference = vulkan but the load did not run on Vulkan: {why}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_cpu_and_cpu_preference_win() {
        assert_eq!(
            choose(BackendPreference::Auto, true, true).unwrap().0,
            Backend::Cpu
        );
        assert_eq!(
            choose(BackendPreference::Vulkan, true, true).unwrap().0,
            Backend::Cpu
        );
        let (b, why) = choose(BackendPreference::Cpu, false, true).unwrap();
        assert_eq!(b, Backend::Cpu);
        assert!(why.contains("cpu"));
    }

    #[test]
    fn auto_reports_why_when_vulkan_is_out() {
        let (b, why) = choose(BackendPreference::Auto, false, false).unwrap();
        assert_eq!(b, Backend::Cpu);
        assert!(why.contains("no Vulkan backend"));
        let (b, why) = choose(BackendPreference::Auto, false, true).unwrap();
        if !vulkan_device_present() {
            assert_eq!(b, Backend::Cpu);
            assert!(why.contains("no Vulkan device"));
        } else {
            assert_eq!(b, Backend::Vulkan);
        }
    }

    #[test]
    fn a_vulkan_preference_is_vulkan_or_a_refusal_never_the_cpu() {
        let why = choose(BackendPreference::Vulkan, false, false).unwrap_err();
        assert!(why.contains("no Vulkan backend"), "{why}");
        match choose(BackendPreference::Vulkan, false, true) {
            Ok((b, _)) => {
                assert_eq!(b, Backend::Vulkan);
                assert!(vulkan_device_present());
            }
            Err(why) => {
                assert!(why.contains("no Vulkan device"), "{why}");
                assert!(!vulkan_device_present());
            }
        }
        assert!(strict_vulkan_refused("x").contains("backend_preference = vulkan"));
    }
}

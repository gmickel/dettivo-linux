//! The hardware tier (S-16): the answer `system.capabilities.platform.tier`,
//! `dettivo doctor` and the benchmark report give, and the tier the NFR
//! targets are read by. `gpu` when a Vulkan device is installed and no
//! engine that has loaded fell back to the CPU; `cpu` otherwise, with the
//! reason. `DETTIVO_FORCE_CPU=1` (the supervisor's `force_cpu`) forces
//! `cpu` through the whole path: the engines it spawns skip Vulkan, and
//! the tier says so.

use dettivo_engine_proto::Backend;
use dettivo_engine_proto::backend::vulkan_device_present;
pub use dettivo_proto::capabilities::Tier;

/// The tier with the reason it was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierReport {
    /// `gpu` or `cpu`.
    pub tier: Tier,
    /// Why: the override, the device, or the engine that fell back.
    pub reason: String,
}

/// One engine that has loaded a model: its binary name, the backend it
/// loaded on, and the reason the engine gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedEngine {
    /// The binary name.
    pub binary: String,
    /// The backend of the last load.
    pub backend: Backend,
    /// The engine's reason for that backend.
    pub reason: String,
}

/// The engines with no GPU execution path: the diarization engine runs
/// sherpa-onnx on the CPU by design, so its CPU backend says nothing
/// about the machine and never counts as a fallback.
pub const CPU_ONLY: &[&str] = &["dettivo-engine-diarize"];

/// Decides the tier from the override, the installed device and the
/// engines that have loaded.
pub fn detect(force_cpu: bool, loaded: &[LoadedEngine]) -> TierReport {
    detect_with(force_cpu, vulkan_device_present(), loaded)
}

/// `detect` with the device presence given (tests).
pub fn detect_with(force_cpu: bool, device_present: bool, loaded: &[LoadedEngine]) -> TierReport {
    if force_cpu {
        return TierReport {
            tier: Tier::Cpu,
            reason: "DETTIVO_FORCE_CPU=1 forces the CPU tier".into(),
        };
    }
    if !device_present {
        return TierReport {
            tier: Tier::Cpu,
            reason: "no Vulkan device (no ICD under /usr/share/vulkan/icd.d or /etc/vulkan/icd.d)"
                .into(),
        };
    }
    let with_gpu_path = || {
        loaded
            .iter()
            .filter(|e| !CPU_ONLY.contains(&e.binary.as_str()))
    };
    if let Some(cpu) = with_gpu_path().find(|e| e.backend == Backend::Cpu) {
        return TierReport {
            tier: Tier::Cpu,
            reason: format!("{} loaded on the CPU: {}", cpu.binary, cpu.reason),
        };
    }
    let on_vulkan: Vec<&str> = with_gpu_path().map(|e| e.binary.as_str()).collect();
    TierReport {
        tier: Tier::Gpu,
        reason: if on_vulkan.is_empty() {
            "a Vulkan device is installed; no engine has loaded yet".into()
        } else {
            format!(
                "a Vulkan device is installed and {} run{} on it",
                on_vulkan.join(", "),
                if on_vulkan.len() == 1 { "s" } else { "" }
            )
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(binary: &str, backend: Backend, reason: &str) -> LoadedEngine {
        LoadedEngine {
            binary: binary.into(),
            backend,
            reason: reason.into(),
        }
    }

    #[test]
    fn the_tier_follows_the_override_the_device_and_the_engines() {
        let vulkan = engine("dettivo-engine-whisper", Backend::Vulkan, "Vulkan0");
        let cpu = engine(
            "dettivo-engine-llm",
            Backend::Cpu,
            "the model exceeds free memory",
        );
        let cases: &[(bool, bool, &[LoadedEngine], Tier, &str)] = &[
            (
                true,
                true,
                std::slice::from_ref(&vulkan),
                Tier::Cpu,
                "DETTIVO_FORCE_CPU=1",
            ),
            (
                false,
                false,
                std::slice::from_ref(&vulkan),
                Tier::Cpu,
                "no Vulkan device",
            ),
            (false, true, &[], Tier::Gpu, "no engine has loaded yet"),
            (
                false,
                true,
                std::slice::from_ref(&vulkan),
                Tier::Gpu,
                "dettivo-engine-whisper runs on it",
            ),
            (
                false,
                true,
                &[vulkan.clone(), cpu.clone()],
                Tier::Cpu,
                "dettivo-engine-llm loaded on the CPU: the model exceeds free memory",
            ),
        ];
        for (force, device, loaded, tier, reason) in cases {
            let report = detect_with(*force, *device, loaded);
            assert_eq!(report.tier, *tier, "{reason}");
            assert!(report.reason.contains(reason), "{}", report.reason);
        }
        assert_eq!(Tier::Gpu.as_str(), "gpu");
    }

    #[test]
    fn cpu_only_diarization_never_moves_a_gpu_machine_off_its_tier() {
        let vulkan = engine("dettivo-engine-whisper", Backend::Vulkan, "Vulkan0");
        let diarize = engine(
            "dettivo-engine-diarize",
            Backend::Cpu,
            "sherpa-onnx runs on the CPU",
        );
        // Before, during and after the speaker pass the tier is the same.
        let before = detect_with(false, true, std::slice::from_ref(&vulkan));
        let during = detect_with(false, true, &[vulkan.clone(), diarize.clone()]);
        let after = detect_with(false, true, std::slice::from_ref(&vulkan));
        assert_eq!(before.tier, Tier::Gpu);
        assert_eq!(during.tier, Tier::Gpu, "{}", during.reason);
        assert_eq!(
            during.reason, before.reason,
            "the report names the engines with a GPU path"
        );
        assert_eq!(after, before);
        // Alone, it says nothing about the machine either.
        let alone = detect_with(false, true, std::slice::from_ref(&diarize));
        assert_eq!(alone.tier, Tier::Gpu);
        assert!(
            alone.reason.contains("no engine has loaded yet"),
            "{}",
            alone.reason
        );
    }
}

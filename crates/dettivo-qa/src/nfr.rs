//! The NFR targets in one place (S-16, ADR 0029): the initial figures the
//! masterplan stated and the calibrated ones the first benchmark reports
//! set. The dictation pack's NFR lines and `dettivo-qa bench` read the
//! calibrated targets from here, so a target moves in one edit and the
//! reports say which figure they were held against.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// The hardware tier a target is stated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// A Vulkan device runs the engines.
    Gpu,
    /// The CPU runs them.
    Cpu,
}

impl Tier {
    /// The lowercase word the reports use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::Cpu => "cpu",
        }
    }

    /// `gpu` when the engine reported Vulkan, `cpu` otherwise.
    pub fn from_backend(backend: Option<&str>) -> Self {
        if backend == Some("vulkan") {
            Self::Gpu
        } else {
            Self::Cpu
        }
    }
}

/// How a measurement is held against its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// The figure must stay at or under the target (a latency, a size).
    AtMost,
    /// The figure must reach the target (a throughput).
    AtLeast,
}

/// One target: the NFR it belongs to, what it bounds, the initial figure
/// and the calibrated one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
    /// `NFR-1`, `NFR-2`, ...
    pub nfr: Cow<'static, str>,
    /// What is measured.
    pub metric: Cow<'static, str>,
    /// The unit of the figure.
    pub unit: Cow<'static, str>,
    /// `at_most` or `at_least`.
    pub direction: Direction,
    /// The masterplan's initial figure.
    pub initial: f64,
    /// The figure in force after the calibration (ADR 0029).
    pub calibrated: f64,
}

impl Target {
    /// True when `value` meets the calibrated target.
    pub fn met(&self, value: f64) -> bool {
        match self.direction {
            Direction::AtMost => value <= self.calibrated,
            Direction::AtLeast => value >= self.calibrated,
        }
    }
}

/// NFR-1: time to first insert, warm engine, GPU tier, p50.
pub const NFR1_FIRST_INSERT_GPU: Target = Target {
    nfr: Cow::Borrowed("NFR-1"),
    metric: Cow::Borrowed("first_insert_p50"),
    unit: Cow::Borrowed("ms"),
    direction: Direction::AtMost,
    initial: 1000.0,
    calibrated: 1000.0,
};

/// NFR-2: time to first insert, warm engine, CPU tier, p50; stated for
/// Parakeet, the CPU tier's engine (ADR 0029).
pub const NFR2_FIRST_INSERT_CPU: Target = Target {
    nfr: Cow::Borrowed("NFR-2"),
    metric: Cow::Borrowed("first_insert_p50"),
    unit: Cow::Borrowed("ms"),
    direction: Direction::AtMost,
    initial: 2500.0,
    calibrated: 1500.0,
};

/// NFR-4: meeting transcription throughput on the GPU tier, as a
/// multiple of realtime.
pub const NFR4_STT_REALTIME_GPU: Target = Target {
    nfr: Cow::Borrowed("NFR-4"),
    metric: Cow::Borrowed("stt_realtime_factor"),
    unit: Cow::Borrowed("x realtime"),
    direction: Direction::AtLeast,
    initial: 10.0,
    calibrated: 20.0,
};

/// NFR-4, second half: diarization throughput on the GPU tier.
pub const NFR4_DIARIZATION_REALTIME_GPU: Target = Target {
    nfr: Cow::Borrowed("NFR-4"),
    metric: Cow::Borrowed("diarization_realtime_factor"),
    unit: Cow::Borrowed("x realtime"),
    direction: Direction::AtLeast,
    initial: 4.0,
    calibrated: 4.0,
};

/// NFR-5: meeting transcription throughput on the CPU tier; stated for
/// Parakeet, the CPU tier's engine (ADR 0029).
pub const NFR5_STT_REALTIME_CPU: Target = Target {
    nfr: Cow::Borrowed("NFR-5"),
    metric: Cow::Borrowed("stt_realtime_factor"),
    unit: Cow::Borrowed("x realtime"),
    direction: Direction::AtLeast,
    initial: 2.0,
    calibrated: 5.0,
};

/// NFR-6: the daemon's resident set after the idle timeout, engines
/// unloaded.
pub const NFR6_IDLE_RSS: Target = Target {
    nfr: Cow::Borrowed("NFR-6"),
    metric: Cow::Borrowed("daemon_idle_rss"),
    unit: Cow::Borrowed("bytes"),
    direction: Direction::AtMost,
    initial: 60.0 * 1024.0 * 1024.0,
    calibrated: 60.0 * 1024.0 * 1024.0,
};

/// NFR-8: the socket answers after activation (docs/daemon.md's budget).
pub const NFR8_SOCKET_READY: Target = Target {
    nfr: Cow::Borrowed("NFR-8"),
    metric: Cow::Borrowed("socket_ready_p50"),
    unit: Cow::Borrowed("ms"),
    direction: Direction::AtMost,
    initial: 300.0,
    calibrated: 300.0,
};

/// NFR-8: the app's first frame (docs/app.md's budget).
pub const NFR8_APP_FIRST_FRAME: Target = Target {
    nfr: Cow::Borrowed("NFR-8"),
    metric: Cow::Borrowed("app_first_frame"),
    unit: Cow::Borrowed("ms"),
    direction: Direction::AtMost,
    initial: 300.0,
    calibrated: 300.0,
};

/// The engine a tier's first-insert and throughput targets are stated for
/// (ADR 0029): Whisper large-v3-turbo on the GPU tier, Parakeet on the
/// CPU tier, where Whisper large-v3-turbo runs at a fraction of the
/// speed. The other engine's row is recorded with its own verdict.
pub fn headline_provider(tier: Tier) -> &'static str {
    match tier {
        Tier::Gpu => "whisper",
        Tier::Cpu => "parakeet",
    }
}

/// The first-insert target for a tier (NFR-1 on the GPU, NFR-2 on the
/// CPU).
pub fn first_insert(tier: Tier) -> Target {
    match tier {
        Tier::Gpu => NFR1_FIRST_INSERT_GPU,
        Tier::Cpu => NFR2_FIRST_INSERT_CPU,
    }
}

/// The transcription throughput target for a tier (NFR-4 on the GPU,
/// NFR-5 on the CPU).
pub fn stt_realtime(tier: Tier) -> Target {
    match tier {
        Tier::Gpu => NFR4_STT_REALTIME_GPU,
        Tier::Cpu => NFR5_STT_REALTIME_CPU,
    }
}

/// Every target, for the report's table.
pub fn all() -> Vec<Target> {
    vec![
        NFR1_FIRST_INSERT_GPU,
        NFR2_FIRST_INSERT_CPU,
        NFR4_STT_REALTIME_GPU,
        NFR4_DIARIZATION_REALTIME_GPU,
        NFR5_STT_REALTIME_CPU,
        NFR6_IDLE_RSS,
        NFR8_SOCKET_READY,
        NFR8_APP_FIRST_FRAME,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_read_by_tier_and_judged_by_direction() {
        assert_eq!(first_insert(Tier::Gpu).nfr, "NFR-1");
        assert_eq!(first_insert(Tier::Cpu).nfr, "NFR-2");
        assert_eq!(stt_realtime(Tier::Gpu).nfr, "NFR-4");
        assert_eq!(stt_realtime(Tier::Cpu).nfr, "NFR-5");
        let latency = first_insert(Tier::Gpu);
        assert!(latency.met(latency.calibrated));
        assert!(!latency.met(latency.calibrated + 1.0));
        let throughput = stt_realtime(Tier::Gpu);
        assert!(throughput.met(throughput.calibrated));
        assert!(!throughput.met(throughput.calibrated - 0.1));
        assert_eq!(Tier::from_backend(Some("vulkan")), Tier::Gpu);
        assert_eq!(Tier::from_backend(Some("cpu")), Tier::Cpu);
        assert_eq!(Tier::from_backend(None), Tier::Cpu);
        assert_eq!(all().len(), 8);
        assert_eq!(headline_provider(Tier::Gpu), "whisper");
        assert_eq!(headline_provider(Tier::Cpu), "parakeet");
    }
}

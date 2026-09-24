//! The hardware tier a machine benchmarks as (S-16, ADR 0029): carried in
//! `system.capabilities.platform.tier` and re-exported from
//! [`crate::capabilities`].

use serde::{Deserialize, Serialize};

/// Linux addition: the hardware tier a machine benchmarks as (S-16). The
/// NFR targets are stated per tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// A Vulkan device is installed and the engines run on it.
    Gpu,
    /// No Vulkan device, an engine that fell back to the CPU, or the
    /// CPU forced.
    Cpu,
}

impl Tier {
    /// The lowercase word the reports and the doctor use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::Cpu => "cpu",
        }
    }
}

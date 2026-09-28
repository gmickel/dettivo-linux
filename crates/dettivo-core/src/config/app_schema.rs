//! The `[app]` section: how the Dettivo Qt windows draw (ADR 0077). The
//! Qt hosts read it from the file before their first window exists, so
//! it takes effect at the next start of each window.

use serde::{Deserialize, Serialize};

/// `[app]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct App {
    /// `auto` draws on the GPU and restarts a window once on the software
    /// renderer when the GPU cannot draw it; `software` draws every
    /// window on the CPU from the start.
    pub renderer: AppRenderer,
}

/// `[app] renderer`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppRenderer {
    /// The GPU, with the one software restart as the fallback.
    #[default]
    Auto,
    /// Qt's software renderer from the start.
    Software,
}

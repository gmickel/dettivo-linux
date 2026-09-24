//! The `[omarchy]` section: what the Omarchy plugin shows in the bar and
//! the panel (docs/omarchy.md, ADR 0030). Split out of `schema.rs` so the
//! schema file stays readable.

use serde::{Deserialize, Serialize};

/// `[omarchy]`: what the Omarchy plugin shows in the bar and the panel.
/// The plugin reads the section through `dettivo config get omarchy.*`
/// and its settings sheet writes it through `dettivo config set`, so the
/// file stays the one source of truth (ADR 0009) and the plugin folder
/// carries no state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Omarchy {
    /// The bar glyph: `waveform` (the six-bar mark) or `dot`.
    pub glyph: OmarchyGlyph,
    /// Whether the glyph's bars follow the live level while recording.
    pub level_meter: bool,
    /// Where the pill shows on Omarchy: `panel` (the plugin hosts it and
    /// claims `dev.dettivo.OmarchyPanel`), `service` (`dettivo-osd` keeps
    /// the pill and the plugin leaves the name unclaimed) or `off`.
    pub osd: OmarchyOsd,
    /// How many history items the panel lists.
    pub history_items: u32,
    /// The shortcut label beside Open Dettivo, in Hyprland's chord notation.
    pub open_shortcut: String,
}

impl Default for Omarchy {
    fn default() -> Self {
        Self {
            glyph: OmarchyGlyph::Waveform,
            level_meter: true,
            osd: OmarchyOsd::Panel,
            history_items: 3,
            open_shortcut: "SUPER SHIFT, D".into(),
        }
    }
}

/// `[omarchy] glyph`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmarchyGlyph {
    /// The six-bar mark.
    Waveform,
    /// A single dot.
    Dot,
}

/// `[omarchy] osd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmarchyOsd {
    /// The plugin's panel hosts the pill; `dettivo-osd` steps aside.
    Panel,
    /// `dettivo-osd.service` keeps the pill.
    Service,
    /// No pill on Omarchy.
    Off,
}

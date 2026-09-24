//! `qa/visual/manifest.toml` (fn-20 R1): every surface the visual job
//! renders, with the binary that draws it, the QA variables that show
//! each state, the artboard its Black Gold crops are cut from, the crop
//! rule for the script, its threshold and its theme set.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// Where the manifest lives, relative to the repository root.
pub const MANIFEST_PATH: &str = "qa/visual/manifest.toml";

/// The theme the artboards were approved in; its crops serve as the
/// baseline until a render is approved for the same theme and scale.
pub const ARTBOARD_THEME: &str = "black-gold";

/// The built-in palettes, rendered without theme files.
pub const BUILTIN_THEMES: &[&str] = &["builtin-dark", "builtin-light"];

/// The structure-correlation threshold when a surface names none.
pub const DEFAULT_THRESHOLD: f64 = 0.55;

/// The whole manifest.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Every theme the job renders in, by fixture name or built-in name.
    pub themes: Vec<String>,
    /// The scale factors (`QT_SCALE_FACTOR`).
    pub scales: Vec<u32>,
    /// The surfaces.
    #[serde(rename = "surface")]
    pub surfaces: Vec<Surface>,
}

/// One surface: a binary and its states.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    /// The name; also the crop directory under the baselines.
    pub name: String,
    /// The Qt binary that renders it with `--render`.
    pub binary: String,
    /// The approved artboard under `docs/design/studio/baselines/`.
    pub artboard: Option<String>,
    /// How the crop script cuts the states out of the artboard: `boxes`
    /// finds bordered boxes top to bottom; absent means each state names
    /// its own rectangle, or the whole artboard is the crop.
    pub crop: Option<String>,
    /// Whether a state's rectangle is also cut out of the render before
    /// the diff (a region of a full-window surface); `false` for a host
    /// that draws the region alone, whose render is compared whole.
    #[serde(default = "default_crop_render")]
    pub crop_render: bool,
    /// The structure-correlation threshold a render must reach.
    #[serde(default = "default_threshold")]
    pub threshold: f64,
    /// Against an approved render of the same theme and scale, the largest
    /// mean-colour distance a cell may drift (0 identical, 1 black against
    /// white); the size must match as well.
    #[serde(default = "default_colour_tolerance")]
    pub colour_tolerance: f64,
    /// A surface-specific theme set; the manifest's set otherwise.
    pub themes: Option<Vec<String>>,
    /// The states.
    #[serde(rename = "state")]
    pub states: Vec<State>,
}

/// One state of a surface: the QA variables that show it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    /// The name; also the crop file name.
    pub name: String,
    /// Variables set for the render (`DETTIVO_E2E_*`).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Arguments the binary takes after `--render <png>` (`--icon` for the
    /// sheet's icon page).
    #[serde(default)]
    pub args: Vec<String>,
    /// The rectangle the crop script cuts for this state (`x, y, w, h`).
    pub crop: Option<[u32; 4]>,
    /// The rectangle of the render held against that crop when the window
    /// draws the region elsewhere (a dialog the artboard shows beside
    /// another); the render is cut here instead of at `crop`.
    pub render_crop: Option<[u32; 4]>,
    /// An artboard of this state's own under `docs/design/studio/baselines/`
    /// (a surface whose steps have one artboard each); the surface's
    /// artboard otherwise.
    pub artboard: Option<String>,
}

/// The colour tolerance when a surface names none.
pub const DEFAULT_COLOUR_TOLERANCE: f64 = 0.06;

fn default_colour_tolerance() -> f64 {
    DEFAULT_COLOUR_TOLERANCE
}

fn default_threshold() -> f64 {
    DEFAULT_THRESHOLD
}

fn default_crop_render() -> bool {
    true
}

impl Manifest {
    /// Parses and validates the manifest text.
    pub fn parse(text: &str) -> Result<Self, String> {
        let manifest: Self = toml::from_str(text).map_err(|e| format!("manifest: {e}"))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Reads `qa/visual/manifest.toml` under `repo`.
    pub fn load(repo: &Path) -> Result<Self, String> {
        let path = repo.join(MANIFEST_PATH);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("manifest: {}: {e}", path.display()))?;
        Self::parse(&text)
    }

    fn validate(&self) -> Result<(), String> {
        if self.themes.is_empty() {
            return Err("manifest: `themes` is empty".into());
        }
        if self.scales.is_empty() {
            return Err("manifest: `scales` is empty".into());
        }
        for scale in &self.scales {
            if !(1..=4).contains(scale) {
                return Err(format!("manifest: scale {scale} is outside 1..=4"));
            }
        }
        let mut names = std::collections::BTreeSet::new();
        for surface in &self.surfaces {
            if !names.insert(surface.name.as_str()) {
                return Err(format!(
                    "manifest: surface {:?} is listed twice",
                    surface.name
                ));
            }
            if surface.states.is_empty() {
                return Err(format!(
                    "manifest: surface {:?} has no states",
                    surface.name
                ));
            }
            if !(surface.threshold > 0.0 && surface.threshold <= 1.0) {
                return Err(format!(
                    "manifest: surface {:?} threshold {} is outside (0, 1]",
                    surface.name, surface.threshold
                ));
            }
            if let Some(crop) = &surface.crop {
                if crop != "boxes" {
                    return Err(format!(
                        "manifest: surface {:?} crop {crop:?}; expected `boxes` or per-state rectangles",
                        surface.name
                    ));
                }
                if surface.artboard.is_none() {
                    return Err(format!(
                        "manifest: surface {:?} has a crop rule but no artboard",
                        surface.name
                    ));
                }
            }
            let mut states = std::collections::BTreeSet::new();
            for state in &surface.states {
                if !states.insert(state.name.as_str()) {
                    return Err(format!(
                        "manifest: surface {:?} state {:?} is listed twice",
                        surface.name, state.name
                    ));
                }
                if state.artboard.is_some() && state.crop.is_none() {
                    return Err(format!(
                        "manifest: surface {:?} state {:?} names an artboard but no crop",
                        surface.name, state.name
                    ));
                }
                if state.render_crop.is_some() && state.crop.is_none() {
                    return Err(format!(
                        "manifest: surface {:?} state {:?} has a render_crop but no crop",
                        surface.name, state.name
                    ));
                }
                if state.crop.is_some() && surface.artboard.is_none() && state.artboard.is_none() {
                    return Err(format!(
                        "manifest: surface {:?} state {:?} has a crop but the surface has no artboard",
                        surface.name, state.name
                    ));
                }
            }
            for theme in surface.themes.iter().flatten() {
                if !self.themes.contains(theme) {
                    return Err(format!(
                        "manifest: surface {:?} names theme {theme:?}, which is not in `themes`",
                        surface.name
                    ));
                }
            }
        }
        Ok(())
    }

    /// A surface by name.
    pub fn surface(&self, name: &str) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.name == name)
    }
}

impl Surface {
    /// The themes this surface renders in.
    pub fn themes<'a>(&'a self, manifest: &'a Manifest) -> &'a [String] {
        self.themes.as_deref().unwrap_or(&manifest.themes)
    }

    /// Whether the crop script cuts a per-state file for `state`
    /// (`boxes`, or the state's own rectangle); otherwise the artboard
    /// itself is the crop.
    pub fn has_state_crop(&self, state: &State) -> bool {
        self.crop.as_deref() == Some("boxes") || state.crop.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
themes = ["black-gold", "builtin-light"]
scales = [1, 2]

[[surface]]
name = "osd"
binary = "dettivo-osd"
artboard = "osd.png"
crop = "boxes"
[[surface.state]]
name = "listening"
env = { DETTIVO_E2E_OSD_STATE = "listening" }

[[surface]]
name = "sheet"
binary = "dettivo-sheet"
artboard = "design-system.png"
threshold = 0.6
themes = ["black-gold"]
crop_render = false
[[surface.state]]
name = "sheet"

[[surface]]
name = "target"
binary = "dettivo-insert-target"
[[surface.state]]
name = "empty"
"#;

    #[test]
    fn the_manifest_parses_surfaces_states_and_defaults() {
        let m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(m.surfaces.len(), 3);
        let osd = m.surface("osd").unwrap();
        assert_eq!(osd.threshold, DEFAULT_THRESHOLD);
        assert_eq!(
            osd.states[0]
                .env
                .get("DETTIVO_E2E_OSD_STATE")
                .map(String::as_str),
            Some("listening")
        );
        assert!(osd.has_state_crop(&osd.states[0]));
        assert!(osd.crop_render);
        assert_eq!(osd.themes(&m).len(), 2);
        let sheet = m.surface("sheet").unwrap();
        assert_eq!(sheet.threshold, 0.6);
        assert_eq!(sheet.themes(&m), &["black-gold".to_string()]);
        assert!(!sheet.crop_render);
        assert!(!sheet.has_state_crop(&sheet.states[0]));
        assert!(m.surface("target").unwrap().artboard.is_none());
    }

    #[test]
    fn a_bad_manifest_is_refused_by_name() {
        let cases = [
            (
                SAMPLE.replace("scales = [1, 2]", "scales = [1, 9]"),
                "scale 9",
            ),
            (
                SAMPLE.replace("name = \"target\"", "name = \"osd\""),
                "listed twice",
            ),
            (
                SAMPLE.replace("crop = \"boxes\"", "crop = \"grid\""),
                "crop \"grid\"",
            ),
            (
                SAMPLE.replace("threshold = 0.6", "threshold = 1.5"),
                "threshold 1.5",
            ),
            (
                SAMPLE.replace("themes = [\"black-gold\"]", "themes = [\"nord\"]"),
                "theme \"nord\"",
            ),
            (
                SAMPLE.replace("binary = \"dettivo-insert-target\"", "bin = \"x\""),
                "unknown field",
            ),
        ];
        for (text, needle) in cases {
            let err = Manifest::parse(&text).unwrap_err();
            assert!(err.contains(needle), "{needle}: {err}");
        }
    }
}

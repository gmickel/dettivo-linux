//! The render matrix (fn-20 R1): every manifest state times every theme
//! and scale, each with the environment that renders it and the baseline
//! it is held to. The baseline for a theme and scale is the approved
//! render `baselines/<surface>/<state>.<theme>.<scale>x.png`; without one,
//! the Black Gold artboard crop stands in for the artboard theme; without
//! either, the entry needs a first approval and never passes silently.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::manifest::{ARTBOARD_THEME, BUILTIN_THEMES, Manifest, State, Surface};

/// Where the approved baselines live, relative to the repository root.
pub const BASELINES_DIR: &str = "docs/design/studio/baselines";

/// Where the theme fixtures live, relative to the repository root.
pub const FIXTURES_DIR: &str = "qt/fixtures/themes";

/// What an entry is compared with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Baseline {
    /// A render approved for this theme and scale.
    Approved(PathBuf),
    /// The artboard crop, standing in for the artboard theme.
    Artboard(PathBuf),
    /// Nothing yet: `approve` writes the render to this path.
    Missing(PathBuf),
}

impl Baseline {
    /// The file to diff against, when there is one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Approved(p) | Self::Artboard(p) => Some(p),
            Self::Missing(_) => None,
        }
    }

    /// `approved`, `artboard` or `missing`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Approved(_) => "approved",
            Self::Artboard(_) => "artboard",
            Self::Missing(_) => "missing",
        }
    }
}

/// One render of the matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The surface name.
    pub surface: String,
    /// The state name.
    pub state: String,
    /// The theme name.
    pub theme: String,
    /// The scale factor.
    pub scale: u32,
    /// The binary that renders it.
    pub binary: String,
    /// The state's own variables.
    pub env: BTreeMap<String, String>,
    /// The state's own arguments after `--render <png>`.
    pub args: Vec<String>,
    /// The threshold, in thousandths so the entry stays comparable.
    pub threshold_milli: u32,
    /// The colour tolerance against an approved render, in thousandths.
    pub colour_tolerance_milli: u32,
    /// The baseline.
    pub baseline: Baseline,
    /// The artboard crop for this state, when the surface has one, for
    /// the advisory artboard score on every theme.
    pub artboard_crop: Option<PathBuf>,
    /// The state's rectangle of the window (`x, y, w, h` at 1x); the
    /// render is cut to it, scaled, before any comparison.
    pub crop: Option<[u32; 4]>,
}

impl Entry {
    /// The threshold as the diff tool takes it.
    pub fn threshold(&self) -> f64 {
        f64::from(self.threshold_milli) / 1000.0
    }

    /// The colour tolerance as the diff tool takes it.
    pub fn colour_tolerance(&self) -> f64 {
        f64::from(self.colour_tolerance_milli) / 1000.0
    }

    /// Whether the baseline is an approved render of this theme and scale,
    /// which the comparison holds to size and colour as well as structure.
    pub fn strict(&self) -> bool {
        matches!(self.baseline, Baseline::Approved(_))
    }

    /// `<surface>/<state>.<theme>.<scale>x`, the file stem of every
    /// artefact of this entry.
    pub fn stem(&self) -> String {
        format!("{}.{}.{}x", self.state, self.theme, self.scale)
    }
}

/// Which entries a run covers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// One surface, or every surface.
    pub surface: Option<String>,
    /// One state of that surface, or every state.
    pub state: Option<String>,
    /// One theme, or every theme.
    pub theme: Option<String>,
    /// One scale, or every scale.
    pub scale: Option<u32>,
}

/// The approved-render path for a theme and scale.
pub fn approved_path(repo: &Path, surface: &str, state: &str, theme: &str, scale: u32) -> PathBuf {
    repo.join(BASELINES_DIR)
        .join(surface)
        .join(format!("{state}.{theme}.{scale}x.png"))
}

/// The artboard crop for a state: the per-state file the crop script
/// writes, or the artboard itself when the whole sheet is the crop.
pub fn artboard_crop(repo: &Path, surface: &Surface, state: &State) -> Option<PathBuf> {
    let artboard = state.artboard.as_deref().or(surface.artboard.as_deref())?;
    let dir = repo.join(BASELINES_DIR);
    Some(if surface.has_state_crop(state) {
        dir.join(&surface.name).join(format!("{}.png", state.name))
    } else {
        dir.join(artboard)
    })
}

/// The variables that select a theme: a fixture directory, or the
/// built-in palette with its colour scheme. A fixture that is missing
/// either file fails by name.
pub fn theme_env(repo: &Path, theme: &str) -> Result<BTreeMap<String, String>, String> {
    let mut env = BTreeMap::new();
    if BUILTIN_THEMES.contains(&theme) {
        env.insert("DETTIVO_OMARCHY_THEME_DIR".into(), "/nonexistent".into());
        env.insert(
            "DETTIVO_COLOR_SCHEME".into(),
            theme.trim_start_matches("builtin-").to_string(),
        );
        return Ok(env);
    }
    let dir = repo.join(FIXTURES_DIR).join(theme);
    for file in ["colors.toml", "shell.toml"] {
        if !dir.join(file).is_file() {
            return Err(format!(
                "theme {theme:?}: fixture missing {}",
                dir.join(file).display()
            ));
        }
    }
    env.insert(
        "DETTIVO_OMARCHY_THEME_DIR".into(),
        dir.to_string_lossy().into_owned(),
    );
    Ok(env)
}

/// Every entry the filter keeps, in manifest order: surface, state,
/// theme, scale.
pub fn expand(manifest: &Manifest, repo: &Path, filter: &Filter) -> Vec<Entry> {
    let mut entries = Vec::new();
    for surface in &manifest.surfaces {
        if filter.surface.as_deref().is_some_and(|s| s != surface.name) {
            continue;
        }
        for state in &surface.states {
            if filter.state.as_deref().is_some_and(|s| s != state.name) {
                continue;
            }
            let crop = artboard_crop(repo, surface, state);
            for theme in surface.themes(manifest) {
                if filter.theme.as_deref().is_some_and(|t| t != theme) {
                    continue;
                }
                for &scale in &manifest.scales {
                    if filter.scale.is_some_and(|s| s != scale) {
                        continue;
                    }
                    let approved = approved_path(repo, &surface.name, &state.name, theme, scale);
                    let baseline = if approved.is_file() {
                        Baseline::Approved(approved)
                    } else if theme == ARTBOARD_THEME && crop.as_ref().is_some_and(|c| c.is_file())
                    {
                        Baseline::Artboard(crop.clone().unwrap_or_default())
                    } else {
                        Baseline::Missing(approved)
                    };
                    entries.push(Entry {
                        surface: surface.name.clone(),
                        state: state.name.clone(),
                        theme: theme.clone(),
                        scale,
                        binary: surface.binary.clone(),
                        env: state.env.clone(),
                        args: state.args.clone(),
                        threshold_milli: (surface.threshold * 1000.0).round() as u32,
                        colour_tolerance_milli: (surface.colour_tolerance * 1000.0).round() as u32,
                        baseline,
                        artboard_crop: crop.clone(),
                        crop: state
                            .render_crop
                            .or(state.crop)
                            .filter(|_| surface.crop_render),
                    });
                }
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
themes = ["black-gold", "tokyo-night", "builtin-light"]
scales = [1, 2]
[[surface]]
name = "osd"
binary = "dettivo-osd"
artboard = "osd.png"
crop = "boxes"
[[surface.state]]
name = "listening"
env = { DETTIVO_E2E_OSD_STATE = "listening" }
[[surface.state]]
name = "error"
env = { DETTIVO_E2E_OSD_STATE = "error" }
[[surface]]
name = "target"
binary = "dettivo-insert-target"
themes = ["black-gold"]
[[surface.state]]
name = "empty"
"#;

    fn repo_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"png").unwrap();
        }
        dir
    }

    #[test]
    fn the_matrix_is_states_times_themes_times_scales_with_the_right_baseline() {
        let repo = repo_with(&[
            "docs/design/studio/baselines/osd/listening.png",
            "docs/design/studio/baselines/osd/listening.tokyo-night.2x.png",
            "docs/design/studio/baselines/osd/error.png",
        ]);
        let manifest = Manifest::parse(SAMPLE).unwrap();
        let entries = expand(&manifest, repo.path(), &Filter::default());
        // osd: 2 states x 3 themes x 2 scales; target: 1 x 1 x 2.
        assert_eq!(entries.len(), 14);
        let find = |state: &str, theme: &str, scale: u32| {
            entries
                .iter()
                .find(|e| e.state == state && e.theme == theme && e.scale == scale)
                .unwrap()
        };
        assert_eq!(
            find("listening", "black-gold", 1).baseline.kind(),
            "artboard"
        );
        assert_eq!(
            find("listening", "tokyo-night", 2).baseline.kind(),
            "approved"
        );
        let missing = find("listening", "tokyo-night", 1);
        assert_eq!(missing.baseline.kind(), "missing");
        assert!(
            matches!(&missing.baseline, Baseline::Missing(p) if p.ends_with("osd/listening.tokyo-night.1x.png"))
        );
        assert_eq!(find("empty", "black-gold", 2).baseline.kind(), "missing");
        assert!(find("empty", "black-gold", 2).artboard_crop.is_none());
        assert_eq!(find("error", "black-gold", 2).stem(), "error.black-gold.2x");
        assert_eq!(find("error", "black-gold", 2).threshold(), 0.55);
        assert_eq!(entries[0].surface, "osd");
        assert_eq!(
            (entries[0].theme.as_str(), entries[0].scale),
            ("black-gold", 1)
        );
        assert_eq!(
            (entries[1].theme.as_str(), entries[1].scale),
            ("black-gold", 2)
        );
    }

    #[test]
    fn filters_narrow_the_matrix() {
        let repo = repo_with(&[]);
        let manifest = Manifest::parse(SAMPLE).unwrap();
        let filter = Filter {
            surface: Some("osd".into()),
            state: None,
            theme: Some("builtin-light".into()),
            scale: Some(2),
        };
        let entries = expand(&manifest, repo.path(), &filter);
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .all(|e| e.theme == "builtin-light" && e.scale == 2)
        );
        let one = Filter {
            surface: Some("osd".into()),
            state: Some("error".into()),
            theme: None,
            scale: None,
        };
        assert_eq!(expand(&manifest, repo.path(), &one).len(), 6);
    }

    #[test]
    fn theme_env_names_a_missing_fixture_and_pins_the_builtin_scheme() {
        let repo = repo_with(&["qt/fixtures/themes/tokyo-night/colors.toml"]);
        let err = theme_env(repo.path(), "tokyo-night").unwrap_err();
        assert!(
            err.contains("tokyo-night") && err.contains("shell.toml"),
            "{err}"
        );
        let light = theme_env(repo.path(), "builtin-light").unwrap();
        assert_eq!(light["DETTIVO_COLOR_SCHEME"], "light");
        assert_eq!(light["DETTIVO_OMARCHY_THEME_DIR"], "/nonexistent");
        std::fs::write(
            repo.path()
                .join("qt/fixtures/themes/tokyo-night/shell.toml"),
            b"",
        )
        .unwrap();
        let fixture = theme_env(repo.path(), "tokyo-night").unwrap();
        assert!(fixture["DETTIVO_OMARCHY_THEME_DIR"].ends_with("themes/tokyo-night"));
    }
}

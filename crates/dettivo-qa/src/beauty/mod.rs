//! `dettivo-qa beauty` (fn-38, ADR 0042): every manifest surface and
//! state rendered on the five palettes through the visual verb's
//! renderer, tiled into one contact sheet per surface (states down,
//! themes across, the artboard score under every cell), every `check:`
//! command of `docs/design/checklist.md` run once, and `beauty-report.md`
//! with a section per surface: the checklist items with their machine
//! verdict or `[ ]` for a person, the artboard scores, and the strings
//! the surface shows. `--themes all` adds a second sheet across every
//! theme installed under Omarchy's themes directory, read at run time.

pub mod checklist;
pub mod report;
pub mod sheet;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::visual::manifest::Manifest;
use crate::visual::matrix::{Entry, Filter, expand};
use crate::visual::render::{RenderError, compare, diff_tool, render};

/// The default output directory, relative to the working directory.
pub const DEFAULT_OUT: &str = "qa-evidence/beauty";

/// Where Omarchy installs its themes, under the user's data directory.
pub const OMARCHY_THEMES: &str = "omarchy/themes";

/// Options for a run.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// One surface, or every surface.
    pub surface: Option<String>,
    /// Where the sheets and the report go.
    pub out: PathBuf,
    /// Also the all-themes sheet from the desktop's Omarchy themes.
    pub all_themes: bool,
}

/// One rendered cell of a sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// The state.
    pub state: String,
    /// The theme.
    pub theme: String,
    /// The render, when the binary drew one.
    pub render: Option<PathBuf>,
    /// The advisory score against the artboard crop.
    pub artboard_score: Option<f64>,
    /// Why there is no render.
    pub error: Option<String>,
    /// The strings the render shows, from `<png>.strings.txt`.
    pub strings: Vec<String>,
}

/// One surface's sheets and cells.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceResult {
    /// The surface.
    pub name: String,
    /// The contact sheet on the fixture palettes.
    pub sheet: PathBuf,
    /// The sheet across the desktop's themes, with `--themes all`.
    pub all_themes_sheet: Option<PathBuf>,
    /// Every cell, in manifest order.
    pub cells: Vec<Cell>,
}

/// The whole run.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Every surface.
    pub surfaces: Vec<SurfaceResult>,
    /// Every checklist item with its verdict.
    pub checks: Vec<checklist::Verdict>,
    /// The report.
    pub report: PathBuf,
}

/// Renders, tiles, checks and reports.
pub fn run(repo: &Path, opts: &RunOptions) -> Result<Outcome, String> {
    let manifest = Manifest::load(repo)?;
    let filter = Filter {
        surface: opts.surface.clone(),
        state: None,
        theme: None,
        scale: Some(1),
    };
    let entries = expand(&manifest, repo, &filter);
    if entries.is_empty() {
        return Err(format!("no manifest surface matches {:?}", opts.surface));
    }
    let tool = diff_tool(repo)?;
    std::fs::create_dir_all(&opts.out).map_err(|e| format!("{}: {e}", opts.out.display()))?;
    let items = checklist::load(repo)?;
    let mut surfaces = Vec::new();
    for surface in &manifest.surfaces {
        if opts.surface.as_deref().is_some_and(|s| s != surface.name) {
            continue;
        }
        let own: Vec<&Entry> = entries
            .iter()
            .filter(|e| e.surface == surface.name)
            .collect();
        let cells = render_cells(repo, &tool, &own, &opts.out);
        let themes = surface.themes(&manifest).to_vec();
        let sheet = opts.out.join(format!("{}.png", surface.name));
        sheet::tile(&tool, &surface.name, &themes, &cells, &sheet)
            .map_err(|e| format!("surface {}: {e}", surface.name))?;
        if !sheet.is_file() {
            return Err(format!(
                "surface {}: no contact sheet written",
                surface.name
            ));
        }
        let all_themes_sheet = if opts.all_themes {
            let themes = desktop_themes()?;
            let cells = render_desktop(repo, &tool, &own, &themes, &opts.out);
            let path = opts.out.join(format!("{}.all-themes.png", surface.name));
            let names: Vec<String> = themes.iter().map(|(n, _)| n.clone()).collect();
            sheet::tile(&tool, &surface.name, &names, &cells, &path)
                .map_err(|e| format!("surface {} (all themes): {e}", surface.name))?;
            Some(path)
        } else {
            None
        };
        surfaces.push(SurfaceResult {
            name: surface.name.clone(),
            sheet,
            all_themes_sheet,
            cells,
        });
    }
    let checks = checklist::run_all(repo, &items)?;
    let report = opts.out.join("beauty-report.md");
    std::fs::write(&report, report::markdown(&surfaces, &checks, &opts.out))
        .map_err(|e| format!("{}: {e}", report.display()))?;
    Ok(Outcome {
        surfaces,
        checks,
        report,
    })
}

/// Renders every entry of one surface at 1x into `out/<surface>/`.
fn render_cells(repo: &Path, tool: &Path, entries: &[&Entry], out: &Path) -> Vec<Cell> {
    let mut cells = Vec::with_capacity(entries.len());
    for entry in entries {
        let dir = out.join(&entry.surface);
        let png = dir.join(format!("{}.png", entry.stem()));
        let log = dir.join(format!("{}.log", entry.stem()));
        cells.push(render_cell(repo, tool, entry, &png, &log));
    }
    cells
}

fn render_cell(repo: &Path, tool: &Path, entry: &Entry, png: &Path, log: &Path) -> Cell {
    let mut cell = Cell {
        state: entry.state.clone(),
        theme: entry.theme.clone(),
        render: None,
        artboard_score: None,
        error: None,
        strings: Vec::new(),
    };
    match render(repo, entry, png, log, None) {
        Ok(()) => cell.render = Some(png.to_path_buf()),
        Err(RenderError::Style(findings)) => {
            cell.render = png.is_file().then(|| png.to_path_buf());
            cell.error = Some(format!("style check: {}", findings.join("; ")));
        }
        Err(RenderError::Process(why)) => cell.error = Some(why),
    }
    // The strings land beside the whole window; a cut render keeps them
    // under the `.window.png` name.
    for candidate in [
        png.with_extension("png.strings.txt"),
        png.with_extension("window.png.strings.txt"),
    ] {
        if let Ok(text) = std::fs::read_to_string(&candidate) {
            cell.strings = text.lines().map(str::to_string).collect();
            break;
        }
    }
    if let (Some(render), Some(crop)) = (&cell.render, entry.artboard_crop.as_deref()) {
        if crop.is_file() {
            if let Ok(score) = compare(tool, crop, render, entry.threshold(), None) {
                cell.artboard_score = Some(score.score);
            }
        }
    }
    cell
}

/// Every theme directory under Omarchy's themes directory on this
/// desktop, by name, colours only: nothing is copied into the fixtures.
pub fn desktop_themes() -> Result<Vec<(String, PathBuf)>, String> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or("neither XDG_DATA_HOME nor HOME is set")?;
    let dir = data.join(OMARCHY_THEMES);
    let mut themes: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| {
            format!(
                "{}: {e} (--themes all needs Omarchy's themes)",
                dir.display()
            )
        })?
        .filter_map(Result::ok)
        .map(|d| d.path())
        .filter(|p| p.join("colors.toml").is_file())
        .filter_map(|p| {
            p.file_name()
                .map(|n| (n.to_string_lossy().into_owned(), p.clone()))
        })
        .collect();
    themes.sort();
    if themes.is_empty() {
        return Err(format!(
            "no theme with a colors.toml under {}",
            dir.display()
        ));
    }
    Ok(themes)
}

/// Renders the surface's states on every desktop theme by pointing the
/// theme directory at each in turn.
fn render_desktop(
    repo: &Path,
    tool: &Path,
    entries: &[&Entry],
    themes: &[(String, PathBuf)],
    out: &Path,
) -> Vec<Cell> {
    let mut cells = Vec::new();
    let mut states: Vec<&Entry> = Vec::new();
    for entry in entries {
        if !states.iter().any(|e| e.state == entry.state) {
            states.push(entry);
        }
    }
    for state in states {
        for (name, dir) in themes {
            let mut entry = state.clone();
            entry.theme = name.clone();
            let mut env: BTreeMap<String, String> = entry.env.clone();
            env.insert(
                "DETTIVO_OMARCHY_THEME_DIR".into(),
                dir.to_string_lossy().into_owned(),
            );
            entry.env = env;
            let dir = out.join(&entry.surface).join("all-themes");
            let png = dir.join(format!("{}.png", entry.stem()));
            let log = dir.join(format!("{}.log", entry.stem()));
            cells.push(render_cell(repo, tool, &entry, &png, &log));
        }
    }
    cells
}

/// The one-line-per-surface human output.
pub fn human(outcome: &Outcome) -> String {
    let mut text = String::new();
    for s in &outcome.surfaces {
        let rendered = s.cells.iter().filter(|c| c.render.is_some()).count();
        text.push_str(&format!(
            "{:<18} {:>3}/{:<3} renders -> {}{}\n",
            s.name,
            rendered,
            s.cells.len(),
            s.sheet.display(),
            s.all_themes_sheet
                .as_ref()
                .map(|p| format!(" and {}", p.display()))
                .unwrap_or_default()
        ));
    }
    let (pass, fail, human) = checklist::tally(&outcome.checks);
    text.push_str(&format!(
        "beauty: {} surfaces, checklist {pass} pass, {fail} fail, {human} for a person -> {}\n",
        outcome.surfaces.len(),
        outcome.report.display()
    ));
    text
}

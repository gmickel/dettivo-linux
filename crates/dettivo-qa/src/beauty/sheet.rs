//! The contact sheet: one surface's cells laid out states down and themes
//! across, every cell captioned with its artboard score or its error,
//! drawn by `dettivo-visual-diff tile` from a JSON spec this module
//! writes beside the sheet.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::Cell;

/// The width every cell is scaled to.
pub const CELL_WIDTH: u32 = 320;

/// The tile spec for one surface: a row per state in first-seen order,
/// a column per theme in the order given, a cell where a render exists.
pub fn spec(surface: &str, themes: &[String], cells: &[Cell]) -> Value {
    let mut states: Vec<&str> = Vec::new();
    for cell in cells {
        if !states.contains(&cell.state.as_str()) {
            states.push(&cell.state);
        }
    }
    let rows: Vec<Value> = states
        .iter()
        .map(|state| {
            let row: Vec<Value> = themes
                .iter()
                .map(|theme| {
                    let cell = cells
                        .iter()
                        .find(|c| c.state == *state && c.theme == *theme);
                    match cell {
                        Some(c) => json!({
                            "path": c.render.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
                            "caption": caption(c),
                        }),
                        None => Value::Null,
                    }
                })
                .collect();
            json!({"label": state, "cells": row})
        })
        .collect();
    json!({
        "title": format!("{surface} · {} states × {} themes · artboard score under each cell", states.len(), themes.len()),
        "columns": themes,
        "rows": rows,
        "cell_width": CELL_WIDTH,
    })
}

/// The caption under a cell.
pub fn caption(cell: &Cell) -> String {
    match (&cell.error, cell.artboard_score) {
        (Some(e), _) => e.lines().next().unwrap_or("error").to_string(),
        (None, Some(s)) => format!("artboard {s:.3}"),
        (None, None) => "no artboard".into(),
    }
}

/// Writes the spec beside `out` and tiles it.
pub fn tile(
    tool: &Path,
    surface: &str,
    themes: &[String],
    cells: &[Cell],
    out: &Path,
) -> Result<(), String> {
    if cells.iter().all(|c| c.render.is_none()) {
        return Err(format!(
            "no render for any state of {surface}: {}",
            cells
                .iter()
                .filter_map(|c| c.error.as_deref())
                .next()
                .unwrap_or("nothing rendered")
        ));
    }
    let spec_path = out.with_extension("json");
    let spec = spec(surface, themes, cells);
    std::fs::write(
        &spec_path,
        serde_json::to_string_pretty(&spec).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("{}: {e}", spec_path.display()))?;
    let output = Command::new(tool)
        .arg("tile")
        .arg(&spec_path)
        .arg(out)
        .env("QT_QPA_PLATFORM", "offscreen")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("run {}: {e}", tool.display()))?;
    if !output.status.success() {
        return Err(format!(
            "dettivo-visual-diff tile exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cell(state: &str, theme: &str, score: Option<f64>, error: Option<&str>) -> Cell {
        Cell {
            state: state.into(),
            theme: theme.into(),
            render: error
                .is_none()
                .then(|| PathBuf::from(format!("/r/{state}.{theme}.1x.png"))),
            artboard_score: score,
            error: error.map(str::to_string),
            strings: Vec::new(),
        }
    }

    #[test]
    fn the_spec_lays_states_down_and_themes_across_with_captions() {
        let themes = vec!["black-gold".to_string(), "builtin-light".to_string()];
        let cells = vec![
            cell("listening", "black-gold", Some(0.6012), None),
            cell(
                "listening",
                "builtin-light",
                None,
                Some("dettivo-osd exited 2"),
            ),
            cell("error", "black-gold", None, None),
            cell("error", "builtin-light", Some(0.7), None),
        ];
        let spec = spec("osd", &themes, &cells);
        assert_eq!(spec["columns"], json!(["black-gold", "builtin-light"]));
        let rows = spec["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["label"], "listening");
        assert_eq!(rows[0]["cells"][0]["caption"], "artboard 0.601");
        assert_eq!(
            rows[0]["cells"][0]["path"],
            "/r/listening.black-gold.1x.png"
        );
        assert_eq!(rows[0]["cells"][1]["caption"], "dettivo-osd exited 2");
        assert_eq!(rows[0]["cells"][1]["path"], "");
        assert_eq!(rows[1]["cells"][0]["caption"], "no artboard");
        assert_eq!(spec["cell_width"], CELL_WIDTH);
        assert!(
            spec["title"]
                .as_str()
                .unwrap()
                .starts_with("osd · 2 states × 2 themes")
        );
    }

    #[test]
    fn a_surface_without_a_single_render_fails_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let cells = vec![cell(
            "sheet",
            "black-gold",
            None,
            Some("dettivo-sheet exited 1"),
        )];
        let err = tile(
            Path::new("/nonexistent/dettivo-visual-diff"),
            "design-system",
            &["black-gold".to_string()],
            &cells,
            &dir.path().join("design-system.png"),
        )
        .unwrap_err();
        assert!(
            err.contains("design-system") && err.contains("exited 1"),
            "{err}"
        );
    }
}

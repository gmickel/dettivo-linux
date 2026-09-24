//! `beauty-report.md`: the checklist's verdicts once at the top, then a
//! section per surface with its contact sheet, the items again with
//! `pass`, `fail` (the command's last line) or `[ ]` for a person, the
//! artboard scores per state and theme, and the strings the surface
//! shows on the artboard theme, so a walkthrough reads the copy beside
//! the picture.

use std::collections::BTreeSet;
use std::path::Path;

use super::checklist::{Verdict, tally};
use super::{Cell, SurfaceResult};

/// The whole report.
pub fn markdown(surfaces: &[SurfaceResult], checks: &[Verdict], out: &Path) -> String {
    let (pass, fail, human) = tally(checks);
    let mut md = String::new();
    md.push_str("# Beauty pass\n\n");
    md.push_str(&format!(
        "{} surfaces on the fixture palettes. Checklist: {pass} pass, {fail} fail, {human} for a person. A machine verdict is a lint's exit code; an open box is Gordon's to tick on the walkthrough (docs/design/checklist.md, ADR 0042).\n\n",
        surfaces.len()
    ));
    md.push_str("## Checklist\n\n| Item | Rule | Check | Verdict |\n|---|---|---|---|\n");
    for v in checks {
        md.push_str(&format!(
            "| {} | {} | `{}` | {} |\n",
            v.item.id,
            v.item.text.replace('|', "\\|"),
            v.item.check.replace('|', "\\|"),
            verdict_cell(v)
        ));
    }
    md.push('\n');
    for s in surfaces {
        md.push_str(&surface_section(s, checks, out));
    }
    md
}

fn verdict_cell(v: &Verdict) -> String {
    match v.outcome.as_str() {
        "pass" => "pass".into(),
        "fail" => format!(
            "**fail** {}",
            v.detail.lines().last().unwrap_or("").replace('|', "\\|")
        ),
        _ => "[ ]".into(),
    }
}

fn relative(out: &Path, path: &Path) -> String {
    path.strip_prefix(out)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// One surface's section.
pub fn surface_section(s: &SurfaceResult, checks: &[Verdict], out: &Path) -> String {
    let mut md = format!("## {}\n\n", s.name);
    md.push_str(&format!(
        "Contact sheet: [{0}]({0})",
        relative(out, &s.sheet)
    ));
    if let Some(all) = &s.all_themes_sheet {
        md.push_str(&format!(
            ", every desktop theme: [{0}]({0})",
            relative(out, all)
        ));
    }
    md.push_str(".\n\n");
    let failed = s.cells.iter().filter(|c| c.error.is_some()).count();
    if failed > 0 {
        md.push_str(&format!("{failed} render(s) failed: "));
        let list: Vec<String> = s
            .cells
            .iter()
            .filter_map(|c| {
                c.error
                    .as_deref()
                    .map(|e| format!("{}/{} ({e})", c.state, c.theme))
            })
            .collect();
        md.push_str(&list.join("; "));
        md.push_str(".\n\n");
    }
    md.push_str("| Item | Verdict |\n|---|---|\n");
    for v in checks {
        md.push_str(&format!("| {} | {} |\n", v.item.id, verdict_cell(v)));
    }
    md.push('\n');
    md.push_str(&scores_table(&s.cells));
    md.push_str(&strings_block(&s.cells));
    md
}

/// Artboard scores, states down and themes across.
pub fn scores_table(cells: &[Cell]) -> String {
    let mut themes: Vec<&str> = Vec::new();
    let mut states: Vec<&str> = Vec::new();
    for c in cells {
        if !themes.contains(&c.theme.as_str()) {
            themes.push(&c.theme);
        }
        if !states.contains(&c.state.as_str()) {
            states.push(&c.state);
        }
    }
    let mut md = String::from("| State |");
    for t in &themes {
        md.push_str(&format!(" {t} |"));
    }
    md.push_str("\n|---|");
    for _ in &themes {
        md.push_str("---|");
    }
    md.push('\n');
    for st in &states {
        md.push_str(&format!("| {st} |"));
        for t in &themes {
            let cell = cells.iter().find(|c| c.state == *st && c.theme == *t);
            let text = match cell {
                Some(c) if c.error.is_some() => "error".to_string(),
                Some(c) => c
                    .artboard_score
                    .map(|s| format!("{s:.3}"))
                    .unwrap_or_else(|| "-".into()),
                None => String::new(),
            };
            md.push_str(&format!(" {text} |"));
        }
        md.push('\n');
    }
    md.push('\n');
    md
}

/// The strings the surface shows, from the first theme's renders, once
/// each in first-seen order.
pub fn strings_block(cells: &[Cell]) -> String {
    let Some(first_theme) = cells.first().map(|c| c.theme.clone()) else {
        return String::new();
    };
    let mut seen = BTreeSet::new();
    let mut lines = Vec::new();
    for c in cells.iter().filter(|c| c.theme == first_theme) {
        for s in &c.strings {
            if seen.insert(s.clone()) {
                lines.push(s.clone());
            }
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut md = format!("Strings shown ({first_theme}):\n\n");
    for l in lines {
        md.push_str(&format!("- {}\n", l.replace('\n', " ")));
    }
    md.push('\n');
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beauty::checklist::Item;
    use std::path::PathBuf;

    fn verdict(id: &str, outcome: &str, detail: &str) -> Verdict {
        Verdict {
            item: Item {
                id: id.into(),
                section: "S".into(),
                text: format!("rule {id}"),
                check: if outcome == "human" {
                    "human".into()
                } else {
                    "scripts/x.sh".into()
                },
            },
            outcome: outcome.into(),
            detail: detail.into(),
        }
    }

    #[test]
    fn the_report_fills_machine_verdicts_and_leaves_human_items_open() {
        let out = PathBuf::from("/out");
        let surfaces = vec![SurfaceResult {
            name: "osd".into(),
            sheet: out.join("osd.png"),
            all_themes_sheet: None,
            cells: vec![
                Cell {
                    state: "listening".into(),
                    theme: "black-gold".into(),
                    render: Some(out.join("osd/listening.black-gold.1x.png")),
                    artboard_score: Some(0.6),
                    error: None,
                    strings: vec!["Listening".into(), "release to insert".into()],
                },
                Cell {
                    state: "listening".into(),
                    theme: "builtin-light".into(),
                    render: None,
                    artboard_score: None,
                    error: Some("dettivo-osd exited 2".into()),
                    strings: Vec::new(),
                },
            ],
        }];
        let checks = vec![
            verdict("C-01", "pass", ""),
            verdict("C-02", "fail", "lint: a.qml:3: rule\nlint: FAILED"),
            verdict("C-03", "human", ""),
        ];
        let md = markdown(&surfaces, &checks, &out);
        assert!(md.contains("Checklist: 1 pass, 1 fail, 1 for a person"));
        assert!(md.contains("| C-01 | rule C-01 | `scripts/x.sh` | pass |"));
        assert!(md.contains("| C-02 | rule C-02 | `scripts/x.sh` | **fail** lint: FAILED |"));
        assert!(md.contains("| C-03 | rule C-03 | `human` | [ ] |"));
        assert!(md.contains("## osd\n\nContact sheet: [osd.png](osd.png)."));
        assert!(md.contains("1 render(s) failed: listening/builtin-light (dettivo-osd exited 2)."));
        assert!(md.contains("| State | black-gold | builtin-light |"));
        assert!(md.contains("| listening | 0.600 | error |"));
        assert!(md.contains("- Listening\n- release to insert\n"));
    }
}

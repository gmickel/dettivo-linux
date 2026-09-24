//! The command-line face of `dettivo-qa visual`: output and exit codes,
//! kept out of main.rs so the binary stays a thin dispatcher.

use std::path::Path;

use crate::visual;

/// `dettivo-qa visual`: the exit code, human or JSON output.
pub fn run(json: bool, repo: &Path, opts: &visual::RunOptions) -> u8 {
    match visual::run(repo, opts) {
        Ok(report) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                print!("{}", visual::human(&report, &opts.out));
            }
            u8::from(!report.ok())
        }
        Err(e) => {
            eprintln!("visual: {e}");
            2
        }
    }
}

/// `dettivo-qa visual approve`: the exit code, human or JSON output.
pub fn approve(
    json: bool,
    repo: &Path,
    filter: &visual::matrix::Filter,
    work: &Path,
    by: Option<&str>,
) -> u8 {
    let manifest = match visual::manifest::Manifest::load(repo) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("visual approve: {e}");
            return 2;
        }
    };
    match visual::approve::approve(repo, &manifest, filter, work, by) {
        Ok(approved) => {
            if json {
                let rows: Vec<serde_json::Value> = approved
                    .iter()
                    .map(|a| {
                        serde_json::json!({
                            "surface": a.entry.surface, "state": a.entry.state,
                            "theme": a.entry.theme, "scale": a.entry.scale,
                            "path": a.path, "artboard_score": a.artboard_score,
                            "replaced": a.replaced,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rows).unwrap_or_default()
                );
            } else {
                print!("{}", visual::human_approved(&approved));
            }
            0
        }
        Err(e) => {
            eprintln!("visual approve: {e}");
            1
        }
    }
}

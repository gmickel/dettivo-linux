//! The command-line face of `dettivo-qa beauty` (ADR 0042): the options,
//! the human or JSON output and the exit code, kept out of main.rs so the
//! binary stays a thin dispatcher.

use std::path::{Path, PathBuf};

use clap::Args;
use dettivo_qa::beauty;

/// `dettivo-qa beauty`.
#[derive(Debug, Args)]
pub struct BeautyArgs {
    /// One surface, or every surface.
    #[arg(long)]
    pub surface: Option<String>,
    /// `fixtures` for the five palettes; `all` adds a sheet across every
    /// theme installed under Omarchy's themes directory.
    #[arg(long, default_value = "fixtures")]
    pub themes: String,
    /// Where the sheets and the report go.
    #[arg(long, default_value = beauty::DEFAULT_OUT, value_name = "DIR")]
    pub out: PathBuf,
}

/// Runs the verb: exit 0 when every machine check passed, 1 when one
/// failed, 2 when a surface could not be rendered or tiled, 4 for a bad
/// option.
pub fn run(json: bool, repo: &Path, args: &BeautyArgs) -> u8 {
    let all_themes = match args.themes.as_str() {
        "fixtures" => false,
        "all" => true,
        other => {
            eprintln!("beauty: --themes {other:?}; expected fixtures or all");
            return 4;
        }
    };
    let opts = beauty::RunOptions {
        surface: args.surface.clone(),
        out: args.out.clone(),
        all_themes,
    };
    match beauty::run(repo, &opts) {
        Ok(outcome) => {
            if json {
                let rows: Vec<serde_json::Value> = outcome
                    .surfaces
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "surface": s.name, "sheet": s.sheet,
                            "all_themes_sheet": s.all_themes_sheet,
                            "renders": s.cells.iter().filter(|c| c.render.is_some()).count(),
                            "cells": s.cells.len(),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "surfaces": rows, "checks": outcome.checks, "report": outcome.report,
                    }))
                    .unwrap_or_default()
                );
            } else {
                print!("{}", beauty::human(&outcome));
            }
            let (_, fail, _) = beauty::checklist::tally(&outcome.checks);
            u8::from(fail > 0)
        }
        Err(e) => {
            eprintln!("beauty: {e}");
            2
        }
    }
}

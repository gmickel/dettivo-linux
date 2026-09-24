//! Repository lints that run through `cargo run -p xtask -- <command>`.
//!
//! Exit codes: 0 pass, 1 violations found (each printed on its own line,
//! path first), 2 usage or tool error.

mod deltas;
mod edges;
mod file_length;
mod notice;
mod notice_markers;

use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo run -p xtask -- <command>

commands:
  lint-edges        check crate dependency edges against tools/xtask/src/edges.rs
  lint-file-length  check source file line counts (.file-length-allow exempts files)
  lint-deltas       every contract item has a disposition in docs/api/linux-deltas.md
  lint-notice       every external crate in Cargo.lock has a row in NOTICE.md and every reuse marker has its row (--write regenerates the crate rows)
  --help            print this help";

/// Outcome of one lint command.
pub enum Outcome {
    /// No violations.
    Pass,
    /// Violations were printed; the caller exits 1.
    Violations,
}

/// A tool failure (cargo or git could not be run, unparsable output, ...).
#[derive(Debug)]
pub struct ToolError(pub String);

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<std::io::Error> for ToolError {
    fn from(err: std::io::Error) -> Self {
        Self(err.to_string())
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["lint-edges"] => edges::run(),
        ["lint-file-length"] => file_length::run(),
        ["lint-deltas"] => deltas::run(),
        ["lint-notice"] => notice::run(false),
        ["lint-notice", "--write"] => notice::run(true),
        ["--help" | "-h" | "help"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(Outcome::Pass) => ExitCode::SUCCESS,
        Ok(Outcome::Violations) => ExitCode::from(1),
        Err(err) => {
            eprintln!("xtask: {err}");
            ExitCode::from(2)
        }
    }
}

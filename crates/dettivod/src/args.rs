//! Command line of the daemon: two overrides and the usual `--version`
//! and `--help`. Everything else is configuration (ADR 0009).

use std::path::PathBuf;

/// Usage text, printed by `--help` and after a bad argument.
pub const USAGE: &str = "Usage: dettivod [--socket PATH] [--config PATH] [--version] [--help]

  --socket PATH   Listen on PATH instead of $XDG_RUNTIME_DIR/dettivo/dettivo.sock
                  (DETTIVO_IPC_SOCKET does the same; a systemd socket unit wins).
  --config PATH   Read PATH instead of $XDG_CONFIG_HOME/dettivo/config.toml
                  (DETTIVO_CONFIG does the same).
";

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Run the daemon with these overrides.
    Run(RunArgs),
    /// Print the version and exit.
    Version,
    /// Print usage and exit.
    Help,
}

/// Overrides for a daemon run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RunArgs {
    /// `--socket`.
    pub socket: Option<PathBuf>,
    /// `--config`.
    pub config: Option<PathBuf>,
}

/// Parses the arguments after the program name.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut run = RunArgs::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--socket" => {
                run.socket = Some(PathBuf::from(iter.next().ok_or("--socket needs a path")?));
            }
            "--config" => {
                run.config = Some(PathBuf::from(iter.next().ok_or("--config needs a path")?));
            }
            other => {
                if let Some(v) = other.strip_prefix("--socket=") {
                    run.socket = Some(PathBuf::from(v));
                } else if let Some(v) = other.strip_prefix("--config=") {
                    run.config = Some(PathBuf::from(v));
                } else {
                    return Err(format!("unknown argument {other:?}"));
                }
            }
        }
    }
    Ok(Command::Run(run))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_both_flag_forms_and_version() {
        assert_eq!(parse(args(&["--version"])).unwrap(), Command::Version);
        let cmd = parse(args(&["--socket", "/tmp/a.sock", "--config=/tmp/c.toml"])).unwrap();
        assert_eq!(
            cmd,
            Command::Run(RunArgs {
                socket: Some(PathBuf::from("/tmp/a.sock")),
                config: Some(PathBuf::from("/tmp/c.toml")),
            })
        );
    }

    #[test]
    fn rejects_unknown_and_incomplete_arguments() {
        assert!(parse(args(&["--bogus"])).is_err());
        assert!(parse(args(&["--socket"])).is_err());
    }
}

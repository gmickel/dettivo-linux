//! `dettivo`: the command line client. Every command maps 1:1 to a
//! contract method (`call` reaches any of them), output is human by
//! default and `--json` for machines, and the exit codes are the
//! contract's: 0 success, 1 runtime failure, 2 daemon unavailable,
//! 3 permission denied, 4 invalid arguments, 5 timeout.

mod app;
mod client;
mod commands;
mod docs;
mod doctor;
mod doctor_report;
mod exit;
mod experiment;
mod history;
mod insert;
mod mcp;
mod mcp_probe;
mod meetings;
mod meetings_notes;
mod meetings_segments;
mod omarchy;
mod omarchy_check;
mod osd;
mod output;
mod paths;
mod polish;
mod rest;
mod setup;
mod speech;
mod transfer;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};

use crate::exit::Exit;

/// The package version and the git commit it was built from
/// (`DETTIVO_GIT_SHA` at build time; see `build.rs`).
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("DETTIVO_GIT_SHA"),
    ")"
);

/// Global options every command honours.
#[derive(Debug, Parser)]
#[command(
    name = "dettivo",
    version = VERSION,
    about = "Talk to the Dettivo daemon",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Socket path (default: DETTIVO_IPC_SOCKET, then $XDG_RUNTIME_DIR/dettivo/dettivo.sock).
    #[arg(long, global = true, value_name = "PATH")]
    pub socket: Option<PathBuf>,
    /// Shared token for peer_token mode (default: DETTIVO_IPC_TOKEN).
    #[arg(long, global = true, value_name = "TOKEN")]
    pub token: Option<String>,
    /// Read the shared token from this file.
    #[arg(long, global = true, value_name = "PATH")]
    pub token_file: Option<PathBuf>,
    /// Give up on a request after this long.
    #[arg(long, global = true, value_name = "MS", default_value_t = 5000)]
    pub timeout_ms: u64,
    /// Print the raw JSON result (or error) instead of human output.
    #[arg(long, global = true)]
    pub json: bool,
    /// Print nothing on success; the exit code is the answer.
    #[arg(long, global = true)]
    pub quiet: bool,
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// system.* methods.
    Status {
        /// Which system method.
        #[command(subcommand)]
        what: commands::StatusCmd,
    },
    /// Read and write config.toml through the daemon.
    Config {
        /// Which config operation.
        #[command(subcommand)]
        what: commands::ConfigCmd,
    },
    /// dictation.* methods: start, stop, cancel, status, toggle, reinsert-last.
    Dictation {
        /// Which dictation verb.
        #[command(subcommand)]
        what: commands::DictationCmd,
    },
    /// Follow the event stream (events.subscribe + events.notify), one JSON line per event.
    Events {
        /// Keep printing until interrupted (the only mode today).
        #[arg(long)]
        follow: bool,
        /// Topics to subscribe (default: every topic).
        #[arg(long = "topic")]
        topics: Vec<String>,
        /// Stop after this many events (0 = unlimited).
        #[arg(long, default_value_t = 0)]
        count: u64,
    },
    /// speech.* methods.
    Speech {
        /// Which speech method.
        #[command(subcommand)]
        what: speech::SpeechCmd,
    },
    /// Insert text into the focused window (insert.perform); `undo` and `target` are Linux additions.
    #[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
    Insert {
        /// `undo` or `target`; without one, perform an insertion.
        #[command(subcommand)]
        what: Option<insert::InsertCmd>,
        /// The insertion flags.
        #[command(flatten)]
        args: insert::InsertArgs,
    },
    /// transcripts.* methods: list, get, latest, search, delete, rerun, export, import.
    History {
        /// Which history verb.
        #[command(subcommand)]
        what: history::HistoryCmd,
    },
    /// meetings.* methods: start, stop, cancel, status, list, get, segments, search, notes, analyze, analysis, export, delete, recover, discard, disclosure.
    Meetings {
        /// Which meeting verb.
        #[command(subcommand)]
        what: meetings::MeetingsCmd,
    },
    /// audio.devices: the capturable nodes, the default source and the default sink.
    Audio {
        /// Which audio verb.
        #[command(subcommand)]
        what: meetings::AudioCmd,
    },
    /// polish.* methods: try the layers, and edit the rules, presets and app profiles.
    Polish {
        /// Which polish verb.
        #[command(subcommand)]
        what: polish::PolishCmd,
    },
    /// llm.* methods (Linux additions): the providers behind Enhanced and the endpoint trust gate.
    Llm {
        /// Which llm verb.
        #[command(subcommand)]
        what: polish::LlmCmd,
    },
    /// Raise the desktop app, open a route in it, or read its status, without the daemon.
    #[command(args_conflicts_with_subcommands = true)]
    App {
        /// `open <route>` or `status`; without one, launch or raise the window.
        #[command(subcommand)]
        what: Option<app::AppCmd>,
    },
    /// Show, hide or inspect the recording pill (dettivo-osd), without the daemon.
    Osd {
        /// Which pill verb.
        #[command(subcommand)]
        what: osd::OsdCmd,
    },
    /// Report service, socket, config, engine, platform, insertion, hotkey, history and pill facts.
    Doctor,
    /// Write the compositor binding snippet (hyprland, sway, niri) with the `[hotkeys]` chords; `omarchy` does the whole install.
    Setup {
        /// omarchy (socket, snippet, plugin, reloads), hyprland (hyprland-lua or hyprland-conf to force a flavour), sway or niri.
        compositor: String,
        /// Print the snippet (and what omarchy would run) instead of writing it.
        #[arg(long)]
        stdout: bool,
        /// Report whether the main configuration loads the snippet (every step for omarchy).
        #[arg(long)]
        check: bool,
        /// omarchy only: enable the socket and write the snippet, leave the plugin alone.
        #[arg(long)]
        no_plugin: bool,
        /// omarchy only: add the plugin from this git URL (the mirror repository) instead of the installed folder.
        #[arg(long, value_name = "URL")]
        git: Option<String>,
    },
    /// hotkeys.status: the daemon-side backend, the portal and evdev availability, the bound actions.
    Hotkeys {
        /// Which hotkeys verb.
        #[command(subcommand)]
        what: commands::HotkeysCmd,
    },
    /// The MCP server over stdio, the host configuration and a check (docs/mcp.md).
    Mcp {
        /// serve, config or check.
        #[command(subcommand)]
        what: mcp::McpCmd,
    },
    /// The loopback REST shim: host it in this process, report the listener, name the token source (docs/rest.md).
    Rest {
        /// serve, status or token.
        #[command(subcommand)]
        what: rest::RestCmd,
    },
    /// Call any contract method with JSON params.
    Call {
        /// Method name, e.g. system.health.
        method: String,
        /// Params as a JSON object (default: {}).
        params: Option<String>,
    },
    /// Generate documentation from this binary: `cli-tree` prints the command tree with every Linux addition marked (docs/guides/agents.md).
    Docs {
        /// cli-tree.
        #[command(subcommand)]
        what: docs::DocsCmd,
    },
    /// Print the shell completion script for bash, zsh or fish (the package installs all three).
    Completions {
        /// The shell.
        shell: clap_complete::Shell,
    },
}

/// A closed pipe on standard output (`dettivo meetings segments <id> | head`)
/// ends the process quietly, as it does for other Unix tools, instead of
/// `println!` panicking. The REST shim keeps Rust's ignored `SIGPIPE`, so a
/// client that hangs up mid-response costs one error, not the server.
#[allow(unsafe_code)]
fn restore_default_sigpipe() {
    // SAFETY: runs once at startup, before any thread exists; `SIG_DFL` is a
    // valid disposition for `SIGPIPE`.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            use clap::error::ErrorKind;
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                let _ = e.print();
                return ExitCode::SUCCESS;
            }
            let _ = e.print();
            let flags: Vec<_> = std::env::args_os()
                .skip(1)
                .take_while(|arg| arg != "--")
                .collect();
            output::failure_json(
                flags.iter().any(|arg| arg == "--json"),
                flags.iter().any(|arg| arg == "--quiet"),
                &exit::Failure::new(Exit::InvalidArgs, e.to_string()),
            );
            return Exit::InvalidArgs.code();
        }
    };
    if !matches!(
        &cli.command,
        Command::Rest {
            what: rest::RestCmd::Serve { .. }
        }
    ) {
        restore_default_sigpipe();
    }
    let outcome = run(&cli);
    match outcome {
        Ok(()) => Exit::Success.code(),
        Err(failure) => {
            output::failure(&cli, &failure);
            failure.exit.code()
        }
    }
}

fn run(cli: &Cli) -> Result<(), exit::Failure> {
    if let Command::Completions { shell } = &cli.command {
        let mut command = Cli::command();
        clap_complete::generate(*shell, &mut command, "dettivo", &mut std::io::stdout());
        return Ok(());
    }
    if let Command::Docs { what } = &cli.command {
        return docs::run(cli, what);
    }
    let client = client::Client::from_cli(cli)?;
    match &cli.command {
        Command::Status { what } => commands::status(cli, &client, what),
        Command::Config { what } => commands::config(cli, &client, what),
        Command::Speech { what } => speech::run(cli, &client, what),
        Command::Dictation { what } => commands::dictation(cli, &client, what),
        Command::History { what } => history::run(cli, &client, what),
        Command::Meetings { what } => meetings::run(cli, &client, what),
        Command::Audio { what } => meetings::audio(cli, &client, what),
        Command::Events {
            follow: _,
            topics,
            count,
        } => commands::events(cli, &client, topics, *count),
        Command::Insert { what, args } => insert::run(cli, &client, what.as_ref(), args),
        Command::Polish { what } => polish::polish(cli, &client, what),
        Command::Llm { what } => polish::llm(cli, &client, what),
        Command::App { what } => app::run(cli, &client.socket, what.as_ref()),
        Command::Osd { what } => osd::run(cli, &client, what),
        Command::Doctor => doctor::run(cli, &client),
        Command::Setup {
            compositor,
            stdout,
            check,
            no_plugin,
            git,
        } => {
            if compositor.eq_ignore_ascii_case("omarchy") {
                omarchy::run(cli, &client, *stdout, *check, *no_plugin, git.as_deref())
            } else {
                setup::run(cli, &client, compositor, *stdout, *check)
            }
        }
        Command::Hotkeys { what } => commands::hotkeys(cli, &client, what),
        Command::Mcp { what } => mcp::run(cli, &client, what),
        Command::Rest { what } => rest::run(cli, &client, what),
        Command::Call { method, params } => commands::call(cli, &client, method, params.as_deref()),
        Command::Docs { .. } | Command::Completions { .. } => Ok(()),
    }
}

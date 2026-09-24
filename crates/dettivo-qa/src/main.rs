//! `dettivo-qa`: the QA runner (ADR 0011). `contract` replays the fixture
//! suite against a fresh daemon, `mcp` drives the MCP server in both
//! framings against one (ADR 0019), `drive` runs scenarios through a
//! driver, `pack` runs a named pack of scenarios into one report (ADR
//! 0017), `audio-check` proves the virtual audio rig, `visual` holds every
//! surface to its approved baseline (ADR 0021), `lint-scenarios` keeps the
//! pack driver-neutral, `doctor` names what a desktop is missing,
//! `pipeline` runs the engine pipelines (`parakeet-alignment`, ADR 0018;
//! `import-merge`, ADR 0022), `polish-eval` runs a held-out set through
//! the Enhanced pipeline against a candidate model and scores it with the
//! macOS metrics and gates (ADR 0032), `bench` measures the NFRs (ADR 0029),
//! `evidence-map` maps every R-ID to a route, `pack release` is the gate
//! (ADR 0041) and `beauty` fills the design checklist's verdicts (ADR 0042).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use dettivo_qa::{bench, doctor, lint, pacing, pack, pipeline, runner, scenarios, visual};

use daemon_verbs::{audio_check, contract, drive, mcp, rest};

mod beauty_verb;
mod cli_paths;
use cli_paths::{models_dir, repo_root};
mod daemon_verbs;
mod polish_verb;

#[derive(Debug, Parser)]
#[command(
    name = "dettivo-qa",
    version,
    about = "Drive the product end to end",
    disable_help_subcommand = true
)]
struct Cli {
    /// Print JSON instead of human output.
    #[arg(long, global = true)]
    json: bool,
    /// Repository root (default: found from this binary or the current directory).
    #[arg(long, global = true, value_name = "DIR")]
    repo: Option<PathBuf>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Replay every contract fixture against a live daemon.
    Contract {
        /// An existing socket; without it a daemon is started in a fresh profile.
        #[arg(long, value_name = "PATH")]
        socket: Option<PathBuf>,
        /// A method the daemon has not implemented yet counts as a failure.
        #[arg(long)]
        strict: bool,
    },
    /// Drive the MCP server in both framings against a seeded daemon.
    Mcp {
        /// An existing seeded socket; without it a daemon is started in a fresh profile.
        #[arg(long, value_name = "PATH")]
        socket: Option<PathBuf>,
        /// The server binary (default: the workspace build of dettivo-mcp).
        #[arg(long, value_name = "PATH")]
        server: Option<PathBuf>,
    },
    /// Replay the REST fixtures against the shim a seeded daemon hosts.
    Rest {
        /// An existing seeded socket whose daemon hosts the shim; without it a daemon is started in a fresh profile.
        #[arg(long, value_name = "PATH")]
        socket: Option<PathBuf>,
    },
    /// Run one scenario (or `all`) on one driver.
    Drive {
        /// Scenario id, or `all`.
        scenario: String,
        /// `cua` or `atspi`.
        #[arg(long, default_value = "atspi")]
        driver: String,
        /// Evidence base directory.
        #[arg(long, default_value = "qa-evidence", value_name = "DIR")]
        evidence: PathBuf,
        /// Keep the profile tree for inspection.
        #[arg(long)]
        keep_profile: bool,
        /// Window and label wait in milliseconds.
        #[arg(long, default_value_t = 15000)]
        timeout_ms: u64,
    },
    /// List the scenarios in the pack.
    List,
    /// Prove the virtual audio rig round trip.
    AudioCheck {
        /// Minimum normalised correlation between fixture and recording.
        #[arg(long, default_value_t = 0.8)]
        tolerance: f64,
    },
    /// Every scenario uses the driver interface only.
    LintScenarios,
    /// The pill's listening animation on the real display: frame pacing evidence.
    OsdPacing {
        /// How long the animation runs.
        #[arg(long, default_value_t = 10)]
        seconds: u32,
        /// Evidence base directory.
        #[arg(long, default_value = "qa-evidence", value_name = "DIR")]
        evidence: PathBuf,
    },
    /// Name what this desktop is missing for a drive.
    Doctor,
    /// Run an engine pipeline: `parakeet-alignment` measures both engines'
    /// timestamps against the golden alignment; `import-merge` runs the
    /// chunker and merger over the engine CLI mode against the golden;
    /// `diarization` scores the diarization engine against the two-speaker
    /// fixture.
    Pipeline {
        /// `parakeet-alignment`, `import-merge` or `diarization`.
        name: String,
        /// Pin both engines to the CPU backend.
        #[arg(long)]
        cpu: bool,
        /// Record the realtime factor and hold it to the NFR-4 floor (`diarization`).
        #[arg(long)]
        bench: bool,
        /// Evidence base directory.
        #[arg(long, default_value = "qa-evidence", value_name = "DIR")]
        evidence: PathBuf,
        /// Also write the report to this path (the checked-in copy).
        #[arg(long, value_name = "FILE")]
        report: Option<PathBuf>,
    },
    /// Run a held-out polish set through the Enhanced pipeline against a
    /// candidate model and score it with the macOS metrics and hard gates.
    PolishEval(polish_verb::PolishEvalArgs),
    /// Measure the NFRs (first insert, transcription throughput, the
    /// language model, the idle footprint, startup) into one report per
    /// host and tier (docs/qa.md, ADR 0029).
    Bench {
        /// Force the CPU tier (`DETTIVO_FORCE_CPU=1` for every engine).
        #[arg(long)]
        cpu: bool,
        /// Three iterations and shorter idles, for CI.
        #[arg(long)]
        quick: bool,
        /// One step by name (`first_insert`, `stt_throughput`,
        /// `llm_rewrite`, `idle_footprint`, `startup`, `diarization`).
        #[arg(long, value_name = "NAME")]
        step: Option<String>,
        /// Also file the report as `<date>-<host>-<tier>.json` in this
        /// directory and render its README table (docs/reports/benchmarks).
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
        /// The engine binaries to measure (a Vulkan build); the workspace
        /// build otherwise.
        #[arg(long, value_name = "DIR")]
        engines: Option<PathBuf>,
        /// Evidence base directory; the report lands under `<dir>/<run>/bench/`.
        #[arg(long, default_value = "qa-evidence", value_name = "DIR")]
        evidence: PathBuf,
    },
    /// Run a named pack of scenarios into one report (`pack list` names them).
    Pack {
        /// Pack name, or `list`.
        name: String,
        /// `cua` or `atspi` for the steps that do not pin a driver.
        #[arg(long, default_value = "atspi")]
        driver: String,
        /// Keep running after a hard failure instead of marking the rest `not_run`.
        #[arg(long = "continue")]
        continue_after_failure: bool,
        /// Evidence base directory; the report lands under `<dir>/<run>/pack-<name>/`.
        #[arg(long, default_value = "qa-evidence", value_name = "DIR")]
        out: PathBuf,
        /// Window and label wait in milliseconds.
        #[arg(long, default_value_t = 15000)]
        timeout_ms: u64,
        /// One surface of a GUI pack: onboarding, settings, history or omarchy.
        #[arg(long, value_name = "SURFACE")]
        surface: Option<String>,
        /// The meetings pack on the CPU tier: `DETTIVO_FORCE_CPU=1` for
        /// every daemon and engine, `base.en` for the throughput steps.
        #[arg(long)]
        cpu: bool,
        /// The Whisper model the meetings pack's throughput steps use
        /// (a catalogue id; the default is the tier's headline model).
        #[arg(long, value_name = "ID")]
        model: Option<String>,
        /// The engine binaries the meetings pack's throughput daemon
        /// runs (a Vulkan build); the workspace build otherwise.
        #[arg(long, value_name = "DIR")]
        engines: Option<PathBuf>,
        /// File the meetings pack's figures into docs/reports/benchmarks
        /// and re-render its README; for the release pack, file the run
        /// under docs/reports/release-gate/<version>.json.
        #[arg(long)]
        record: bool,
        /// A release gate step to leave out, recorded as skipped (repeatable).
        #[arg(long, value_name = "STEP")]
        skip: Vec<String>,
    },
    /// Every R-ID of every spec under .flow/specs/ mapped to an evidence
    /// route that exists (qa/evidence-map.toml, docs/guides/qa.md).
    EvidenceMap {
        /// Also write docs/reports/evidence-map.json and .md.
        #[arg(long)]
        write: bool,
    },
    /// Render every manifest surface at every scale and theme and diff
    /// each against its approved baseline (qa/visual/manifest.toml).
    Visual {
        #[command(subcommand)]
        action: Option<VisualAction>,
        /// One surface, or every surface.
        #[arg(long)]
        surface: Option<String>,
        /// One theme, or every theme.
        #[arg(long)]
        theme: Option<String>,
        /// One scale, or every scale.
        #[arg(long)]
        scale: Option<u32>,
        /// Where renders, diffs and the report go.
        #[arg(long, default_value = visual::DEFAULT_OUT, value_name = "DIR")]
        out: PathBuf,
        /// Render with a deliberate token regression; passes only when
        /// every diff fails.
        #[arg(long)]
        canary: bool,
        /// Which regression the canary renders: all (accent, type and
        /// spacing), colour (the accent alone, seen only against approved
        /// renders), type or spacing.
        #[arg(long, default_value = "all", value_name = "AXIS", requires = "canary")]
        canary_axis: String,
    },
    /// Render every manifest surface on the five palettes into one
    /// contact sheet per surface and write beauty-report.md with the
    /// design checklist's machine verdicts (docs/design/checklist.md).
    Beauty(beauty_verb::BeautyArgs),
}

#[derive(Debug, Subcommand)]
enum VisualAction {
    /// Render a surface fresh and make the render its baseline for the
    /// themes and scales given, recorded in docs/design/baselines.md.
    Approve {
        /// The surface.
        surface: String,
        /// One state, or every state.
        #[arg(long)]
        state: Option<String>,
        /// One theme, or every theme.
        #[arg(long)]
        theme: Option<String>,
        /// One scale, or every scale.
        #[arg(long)]
        scale: Option<u32>,
        /// Where the fresh renders go.
        #[arg(long, default_value = visual::DEFAULT_OUT, value_name = "DIR")]
        out: PathBuf,
        /// Who approves, in docs/design/baselines.md (default: the git
        /// user name); `awaiting review` leaves the column for the reviewer.
        #[arg(long, value_name = "NAME")]
        by: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() {
                ExitCode::from(4)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    let repo = repo_root(cli.repo.clone());
    let code = match &cli.command {
        Cmd::Contract { socket, strict } => contract(&cli, &repo, socket.as_deref(), *strict),
        Cmd::Mcp { socket, server } => mcp(&cli, &repo, socket.as_deref(), server.as_deref()),
        Cmd::Rest { socket } => rest(&cli, &repo, socket.as_deref()),
        Cmd::Drive {
            scenario,
            driver,
            evidence,
            keep_profile,
            timeout_ms,
        } => drive(
            &cli,
            &repo,
            scenario,
            runner::Options {
                driver: driver.clone(),
                evidence_base: evidence.clone(),
                models_dir: models_dir(),
                keep_profile: *keep_profile,
                timeout: Duration::from_millis(*timeout_ms),
                run_dir: None,
                env: Default::default(),
            },
        ),
        Cmd::List => {
            for s in scenarios::all() {
                println!("{:<24} {}", s.id(), s.summary());
            }
            0
        }
        Cmd::AudioCheck { tolerance } => audio_check(&cli, &repo, *tolerance),
        Cmd::LintScenarios => match lint::run(&repo.join("crates/dettivo-qa/src/scenarios")) {
            Ok(n) => {
                println!("lint-scenarios: OK ({n} scenarios use the driver interface only)");
                0
            }
            Err(findings) => {
                for f in findings {
                    eprintln!("lint-scenarios: {f}");
                }
                1
            }
        },
        Cmd::OsdPacing { seconds, evidence } => match scenarios::binary(&repo, "dettivo-osd")
            .map_err(|e| e.to_string())
            .and_then(|osd| pacing::run(&osd, evidence, *seconds))
        {
            Ok(report) => {
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report.summary).unwrap_or_default()
                    );
                } else {
                    print!("{}", pacing::human(&report));
                }
                u8::from(!report.ok)
            }
            Err(e) => {
                eprintln!("osd-pacing: {e}");
                2
            }
        },
        Cmd::Pipeline {
            name,
            cpu,
            bench,
            evidence,
            report,
        } => pipeline::run_cli(
            cli.json,
            &repo,
            name,
            *cpu,
            *bench,
            evidence,
            report.as_deref(),
        ),
        Cmd::Bench {
            cpu,
            quick,
            step,
            out,
            engines,
            evidence,
        } => bench::command(
            &bench::Options {
                repo_root: repo.clone(),
                cpu: *cpu,
                quick: *quick,
                step: step.clone(),
                out: out.clone(),
                evidence_base: evidence.clone(),
                engines_dir: engines.clone(),
                models_dir: models_dir(),
            },
            cli.json,
        ),
        Cmd::Pack {
            name,
            driver,
            continue_after_failure,
            out,
            timeout_ms,
            surface,
            cpu,
            model,
            engines,
            record,
            skip,
        } => pack::command(
            name,
            surface.as_deref(),
            &pack::RunOptions {
                repo_root: repo.clone(),
                driver: driver.clone(),
                continue_after_failure: *continue_after_failure,
                evidence_base: out.clone(),
                models_dir: models_dir(),
                timeout: Duration::from_millis(*timeout_ms),
                meetings: pack::meetings::Options {
                    cpu: *cpu,
                    model: model.clone(),
                    engines_dir: engines.clone(),
                    record: *record,
                },
                release: pack::release::Options {
                    skip: skip.clone(),
                    record: *record,
                },
            },
            cli.json,
        ),
        Cmd::EvidenceMap { write } => dettivo_qa::evidence_map::command(&repo, cli.json, *write),
        Cmd::PolishEval(args) => polish_verb::run(cli.json, &repo, args, models_dir()),
        Cmd::Doctor => {
            let checks = doctor::checks();
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&checks).unwrap_or_default()
                );
            } else {
                print!("{}", doctor::human(&checks));
            }
            u8::from(doctor::drive_ready(&checks).is_err())
        }
        Cmd::Beauty(args) => beauty_verb::run(cli.json, &repo, args),
        Cmd::Visual {
            action,
            surface,
            theme,
            scale,
            out,
            canary,
            canary_axis,
        } => match action {
            Some(VisualAction::Approve {
                surface,
                state,
                theme,
                scale,
                out,
                by,
            }) => {
                let filter = visual::matrix::Filter {
                    surface: Some(surface.clone()),
                    state: state.clone(),
                    theme: theme.clone(),
                    scale: *scale,
                };
                visual::cli::approve(
                    cli.json,
                    &repo,
                    &filter,
                    &out.join("approve"),
                    by.as_deref(),
                )
            }
            None => visual::cli::run(
                cli.json,
                &repo,
                &visual::RunOptions {
                    filter: visual::matrix::Filter {
                        surface: surface.clone(),
                        state: None,
                        theme: theme.clone(),
                        scale: *scale,
                    },
                    out: if *canary {
                        out.join(format!("canary-{canary_axis}"))
                    } else {
                        out.clone()
                    },
                    canary: *canary,
                    canary_axis: canary_axis.clone(),
                },
            ),
        },
    };
    ExitCode::from(code)
}

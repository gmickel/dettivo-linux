//! `dettivod`: the resident daemon. It owns the Unix socket, authenticates
//! peers, routes JSON-RPC to the `system.*`, `config.*` and `insert.*`
//! handlers, answers every other contract method `NOT_IMPLEMENTED` until
//! its spec lands, hosts the loopback REST shim when `[rest] enabled`
//! (ADR 0028), watches `config.toml`, and shuts down cleanly on SIGTERM
//! (ADR 0002, ADR 0009).
//!
//! Exit status: 0 after a clean shutdown, 1 when another instance owns the
//! socket or the socket cannot be acquired, 2 for bad arguments.

mod actions;
mod analysis;
mod args;
mod auth;
mod daemon;
mod diarization;
mod diarization_run;
mod dictation;
mod engines;
mod events;
mod feedback;
mod handlers;
mod history;
mod hotkeys;
mod imports;
mod inserter;
mod jobs;
mod jobs_run;
mod listener;
mod llm_engine;
mod logging;
mod meeting_jobs;
mod meetings;
mod meetings_archive;
mod models;
mod platform;
mod rest;
mod router;
mod self_target;
mod server;
mod shutdown;
mod transfers;
mod verify;
mod watcher;

use std::process::ExitCode;
use std::sync::Arc;

use dettivo_core::paths::Paths;

fn main() -> ExitCode {
    let args = match args::parse(std::env::args().skip(1)) {
        Ok(args::Command::Run(args)) => args,
        Ok(args::Command::Version) => {
            println!("dettivod {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Ok(args::Command::Help) => {
            print!("{}", args::USAGE);
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("dettivod: {message}");
            eprint!("{}", args::USAGE);
            return ExitCode::from(2);
        }
    };

    // QA mode (ADR 0011): a bad or unknown QA variable is a refusal, not a
    // silent real run, decided before anything else happens.
    let qa = match dettivo_core::qa::QaEnv::from_env(dettivo_core::qa::BuildKind::current()) {
        Ok(qa) => qa,
        Err(e) => {
            eprintln!("dettivod: {e}");
            return ExitCode::from(2);
        }
    };

    let mut paths = Paths::resolve();
    if let Some(config) = args.config {
        paths.config_file = config;
        paths.config_file_source = dettivo_proto::methods::config::Source::Environment;
    }
    if let Some(socket) = args.socket {
        paths.socket = socket;
        paths.socket_source = dettivo_proto::methods::config::Source::Environment;
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("dettivod: cannot start the async runtime: {e}");
            return ExitCode::from(1);
        }
    };

    runtime.block_on(async move {
        // The history store opens before anything listens: a migration
        // that fails names itself and its backup, and the daemon does not
        // start on a database it cannot read.
        let daemon = match daemon::Daemon::new(paths, &qa) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("dettivod: {e}");
                return ExitCode::from(1);
            }
        };
        logging::init(daemon.log_level());
        let daemon = Arc::new(daemon.with_audio());
        daemon.report_config_state();
        // A meeting a previous daemon left recording is promoted to
        // partial before any client can ask about it (FR-G7).
        let promoted = daemon.meetings().recover_stale(daemon.history());
        if promoted > 0 {
            tracing::warn!(promoted, "meetings promoted to partial after a restart");
        }
        if qa.enabled || daemon.config().config.qa.mode {
            tracing::info!(
                mock_mode = qa.mock_mode,
                force_cpu = qa.force_cpu,
                "QA mode on"
            );
        }
        let acquired = match listener::acquire(&daemon).await {
            Ok(a) => a,
            Err(e) => {
                tracing::error!(error = %e, "cannot acquire the socket");
                eprintln!("dettivod: {e}");
                return ExitCode::from(1);
            }
        };
        daemon.record_start();
        if let Some(disclosure) = qa.e2e_disclosure {
            let acknowledged = disclosure == dettivo_core::qa::Disclosure::Acknowledged;
            match daemon.meetings().seed_disclosure(acknowledged) {
                Ok(()) => tracing::info!(acknowledged, "QA meeting disclosure seeded"),
                Err(e) => tracing::warn!(error = %e, "QA meeting disclosure seed failed"),
            }
        }
        if qa.e2e_seed {
            match daemon.history().seed() {
                Ok(added) => tracing::info!(added, "QA history seed applied"),
                Err(e) => tracing::warn!(error = %e, "QA history seed failed"),
            }
            match handlers::polish::seed(&daemon) {
                Ok(added) => tracing::info!(added, "QA polish rule seed applied"),
                Err(e) => tracing::warn!(error = %e.message, "QA polish rule seed failed"),
            }
        }
        // The preload waits for the start-up verification, so no engine
        // opens a model the verifier is about to quarantine; without the
        // verification each load hashes what it needs first.
        if daemon.config().config.models.verify_on_start {
            let d = daemon.clone();
            daemon
                .models()
                .verify_on_start(move || d.engines().preload_startup());
        } else {
            daemon.engines().preload_startup();
        }
        // A finalisation that lands completed starts the speaker pass when
        // `[meetings.diarization] auto` applies (ADR 0035).
        let weak = Arc::downgrade(&daemon);
        daemon
            .meeting_archive()
            .set_on_completed(Arc::new(move |row| {
                if let Some(d) = weak.upgrade() {
                    d.diarization().auto_start(&d, row.clone());
                }
            }));
        daemon.hotkeys().start(&daemon);
        let rest = rest::start(&daemon).await;
        let reaper = tokio::spawn(reap_idle_engines(daemon.clone()));
        let maintenance = tokio::spawn(maintain_history(daemon.clone()));
        let downloads = tokio::spawn(publish_downloads(daemon.clone()));
        // One deadline from the signal: the drain, the listener release
        // and every stop below share `[daemon] shutdown_timeout_ms`.
        let deadline = server::run(daemon.clone(), acquired).await;
        if tokio::time::timeout_at(deadline.into(), rest::stop(&daemon, rest))
            .await
            .is_err()
        {
            tracing::warn!("shutdown budget exhausted; REST shim left behind");
        }
        reaper.abort();
        maintenance.abort();
        downloads.abort();
        let d = daemon.clone();
        shutdown::within(deadline, "hotkeys", move || d.hotkeys().shutdown());
        let d = daemon.clone();
        shutdown::within(deadline, "models", move || d.models().shutdown());
        // The engines' shutdown waits for each slot's lock; an inference
        // in flight holds it, so the budget rather than the lock decides
        // when the daemon exits. The children end on their closed pipes.
        let d = daemon.clone();
        shutdown::within(deadline, "engines", move || d.engines().shutdown());
        ExitCode::SUCCESS
    })
}

/// The hourly retention sweep (first run at start) and the minute-wise
/// expiry of idle transfers, off the runtime's worker threads.
async fn maintain_history(daemon: Arc<daemon::Daemon>) {
    let mut ticks = tokio::time::interval(std::time::Duration::from_secs(60));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut minute: u64 = 0;
    loop {
        ticks.tick().await;
        let expired = daemon.transfers().expire();
        if expired > 0 {
            tracing::info!(expired, "idle transfers dropped");
        }
        if minute % 60 == 0 {
            let d = daemon.clone();
            match tokio::task::spawn_blocking(move || d.history().sweep()).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => tracing::warn!(error = %e, "history sweep failed"),
                Err(e) => tracing::error!(error = %e, "history sweep task failed"),
            }
        }
        minute += 1;
    }
}

/// Publishes the `model.download` payloads the download threads queued
/// onto the event stream four times a second, and logs each download
/// that ended, so a client following a download sees its progress and
/// the journal keeps the outcome (docs/models.md).
async fn publish_downloads(daemon: Arc<daemon::Daemon>) {
    let mut ticks = tokio::time::interval(std::time::Duration::from_millis(250));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        for p in daemon.models().drain_published() {
            if p.state != "running" {
                tracing::info!(
                    provider = %p.provider,
                    model = %p.model,
                    state = %p.state,
                    bytes = p.bytes_done,
                    "model download ended"
                );
            }
            daemon.bus().publish(
                dettivo_proto::events::Topic::ModelDownload,
                serde_json::json!(p),
            );
        }
    }
}

/// Unloads engines idle past `[engines].stt_idle_seconds`, checking every
/// `REAP_INTERVAL` off the runtime's worker threads.
async fn reap_idle_engines(daemon: Arc<daemon::Daemon>) {
    let mut ticks = tokio::time::interval(engines::REAP_INTERVAL);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        let d = daemon.clone();
        let stopped = match tokio::task::spawn_blocking(move || d.engines().reap_idle()).await {
            Ok(n) => n,
            Err(e) => {
                tracing::error!(error = %e, "idle engine reaper failed this tick");
                0
            }
        };
        if stopped > 0 {
            tracing::info!(stopped, "idle engines unloaded");
        }
    }
}

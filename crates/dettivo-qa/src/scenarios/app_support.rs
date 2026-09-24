//! What the app scenarios share (fn-17): the route table with the titles
//! the drives assert, the app's QA journal, its instance socket, the
//! evidence a route leaves, the negative text scan over a tree, and the
//! timing gate that turns recorded numbers into assertions on this
//! machine.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::Context;
use crate::driver::{App, Driver, Element};
use crate::negative_text;

/// One route the drive opens: the `DETTIVO_E2E_OPEN` value, the
/// `DETTIVO_E2E_ROUTE` value for a settings section, and the title the
/// page carries (`docs/qa/a11y-names.md`).
pub struct RouteCase {
    /// The evidence name and the `--open` spelling.
    pub name: &'static str,
    /// `DETTIVO_E2E_OPEN`.
    pub open: &'static str,
    /// `DETTIVO_E2E_ROUTE`, for a settings section.
    pub route: Option<&'static str>,
    /// The accessible title.
    pub title: &'static str,
}

/// Every FR-U2 route, the settings sections through `DETTIVO_E2E_ROUTE`.
pub const ROUTES: &[RouteCase] = &[
    RouteCase {
        name: "home",
        open: "home",
        route: None,
        title: "Home",
    },
    RouteCase {
        name: "history",
        open: "history",
        route: None,
        title: "History",
    },
    RouteCase {
        name: "history.detail",
        open: "history.detail",
        route: None,
        title: "Dictation",
    },
    RouteCase {
        name: "meetings",
        open: "meetings",
        route: None,
        title: "Meetings",
    },
    RouteCase {
        name: "meetings.live",
        open: "meetings.live",
        route: None,
        title: "Meeting live",
    },
    RouteCase {
        name: "meetings.detail",
        open: "meetings.detail",
        route: None,
        title: "Meeting",
    },
    RouteCase {
        name: "settings.general",
        open: "settings",
        route: Some("general"),
        title: "Settings / General",
    },
    RouteCase {
        name: "settings.hotkeys",
        open: "settings",
        route: Some("hotkeys"),
        title: "Settings / Hotkeys",
    },
    RouteCase {
        name: "settings.models",
        open: "settings",
        route: Some("models"),
        title: "Settings / Models",
    },
    RouteCase {
        name: "settings.polish",
        open: "settings",
        route: Some("polish"),
        title: "Settings / Polish",
    },
    RouteCase {
        name: "settings.insertion",
        open: "settings",
        route: Some("insertion"),
        title: "Settings / Insertion",
    },
    RouteCase {
        name: "settings.meetings",
        open: "settings",
        route: Some("meetings"),
        title: "Settings / Meetings",
    },
    RouteCase {
        name: "settings.agents",
        open: "settings",
        route: Some("agents"),
        title: "Settings / Agents",
    },
    RouteCase {
        name: "settings.diagnostics",
        open: "settings",
        route: Some("diagnostics"),
        title: "Settings / Diagnostics",
    },
    RouteCase {
        name: "onboarding",
        open: "onboarding",
        route: None,
        title: "First run",
    },
];

/// The first-frame budget (FR-U7).
pub const FIRST_FRAME_BUDGET_MS: u64 = 300;
/// Thor desktop-app idle RSS allowance (ADR 0059), in KiB.
pub const RSS_BUDGET_KB: u64 = 320 * 1024;
/// Thor desktop-app anonymous allocation-growth guard, in KiB.
pub const ANONYMOUS_BUDGET_KB: u64 = 128 * 1024;
/// The theme apply budget (NFR-14).
pub const THEME_APPLY_BUDGET_MS: u64 = 100;

/// `DETTIVO_TIMING_GATE=1` makes the budgets assertions; without it the
/// numbers are recorded, since CI's runners are slower than any desktop.
pub fn timing_gate() -> bool {
    std::env::var("DETTIVO_TIMING_GATE").is_ok_and(|v| v == "1")
}

/// The app's QA journal under the profile's state directory.
pub fn journal_path(ctx: &Context<'_>) -> PathBuf {
    ctx.profile.root.join("state/dettivo/qa/app.jsonl")
}

/// Reads the journal's lines, oldest first.
pub fn journal(ctx: &Context<'_>) -> Vec<Value> {
    std::fs::read_to_string(journal_path(ctx))
        .map(|text| {
            text.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Waits for a journal line with `event`, returning it.
pub fn wait_journal(ctx: &Context<'_>, event: &str, timeout: Duration) -> Result<Value, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(line) = journal(ctx).into_iter().rev().find(|l| l["event"] == event) {
            return Ok(line);
        }
        if Instant::now() > deadline {
            return Err(format!(
                "the app never journaled {event:?} within {timeout:?} ({})",
                journal_path(ctx).display()
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Clears the journal between launches.
pub fn reset_journal(ctx: &Context<'_>) {
    let _ = std::fs::remove_file(journal_path(ctx));
}

/// One line to the app's instance socket beside the daemon socket.
pub fn app_command(ctx: &Context<'_>, command: &Value) -> Result<Value, String> {
    let socket = ctx.profile.socket().with_file_name("app.sock");
    let mut stream = UnixStream::connect(&socket).map_err(|e| format!("app.sock: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(format!("{command}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(line.trim_end()).map_err(|e| e.to_string())
}

/// The app's own status until `predicate` holds.
pub fn wait_status(
    ctx: &Context<'_>,
    timeout: Duration,
    what: &str,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    let deadline = Instant::now() + timeout;
    let mut last = Value::Null;
    loop {
        if let Ok(status) = app_command(ctx, &serde_json::json!({"cmd": "status"})) {
            if predicate(&status) {
                return Ok(status);
            }
            last = status;
        }
        if Instant::now() > deadline {
            return Err(format!(
                "the app never reported {what} within {timeout:?}; last status {last}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A process's memory in KiB from /proc. RSS includes shared mappings;
/// PSS apportions them. Anonymous includes Qt and driver allocations,
/// not only allocations made directly by Dettivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    /// `VmRSS`.
    pub rss_kb: u64,
    /// `Pss` from smaps_rollup: shared pages divided by their sharers.
    pub pss_kb: u64,
    /// `Anonymous` from smaps_rollup: heap and stacks, no file behind them.
    pub anonymous_kb: u64,
}

impl Memory {
    /// Both inclusive desktop limits; PSS remains informational.
    pub fn within_budget(self) -> bool {
        self.rss_kb <= RSS_BUDGET_KB && self.anonymous_kb <= ANONYMOUS_BUDGET_KB
    }

    /// Additive footprint fields; the historical `_kb_after_5s` keys retain KiB.
    pub fn evidence(self) -> Value {
        serde_json::json!({
            "rss_kb_after_5s": self.rss_kb,
            "pss_kb_after_5s": self.pss_kb,
            "anonymous_kb_after_5s": self.anonymous_kb,
            "rss_mib_after_5s": self.rss_kb as f64 / 1024.0,
            "pss_mib_after_5s": self.pss_kb as f64 / 1024.0,
            "anonymous_mib_after_5s": self.anonymous_kb as f64 / 1024.0,
            "rss_budget_kb": RSS_BUDGET_KB,
            "anonymous_budget_kb": ANONYMOUS_BUDGET_KB,
            "rss_budget_mib": RSS_BUDGET_KB / 1024,
            "anonymous_budget_mib": ANONYMOUS_BUDGET_KB / 1024,
            "rss_within_budget": self.rss_kb <= RSS_BUDGET_KB,
            "anonymous_within_budget": self.anonymous_kb <= ANONYMOUS_BUDGET_KB,
            "pss_budget_kb": null,
            "memory_within_budget": self.within_budget(),
            "memory_units": "KiB (Linux procfs kB); MiB = KiB / 1024",
            "memory_source": {"rss": "/proc/<pid>/status:VmRSS", "pss": "/proc/<pid>/smaps_rollup:Pss", "anonymous": "/proc/<pid>/smaps_rollup:Anonymous"},
            "memory_scope": "dettivo-app process only; daemon and engine processes excluded",
        })
    }
}

/// The resident set of a pid in kilobytes, from /proc.
pub fn resident_kb(pid: u32) -> Option<u64> {
    memory_kb(pid).map(|m| m.rss_kb)
}

/// The memory breakdown of a pid.
pub fn memory_kb(pid: u32) -> Option<Memory> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let rollup = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
    parse_memory(&status, &rollup)
}

pub(super) fn parse_memory(status: &str, rollup: &str) -> Option<Memory> {
    let field = |text: &str, key: &str| -> Option<u64> {
        let mut matches = text.lines().filter(|l| l.starts_with(key));
        let mut parts = matches.next()?.split_whitespace();
        if parts.next()? != key {
            return None;
        }
        let value = parts.next()?.parse().ok()?;
        if parts.next()? != "kB" || parts.next().is_some() || matches.next().is_some() {
            return None;
        }
        Some(value)
    };
    let rss_kb = field(status, "VmRSS:")?;
    Some(Memory {
        rss_kb,
        pss_kb: field(rollup, "Pss:")?,
        anonymous_kb: field(rollup, "Anonymous:")?,
    })
}

/// Snapshot, screenshot and the negative text scan for one named look;
/// the findings are the error. A path into the drive's own profile root
/// (the socket the Agents route names, a data directory) is the rig's
/// location shown back and is set aside; every other class fails.
pub fn capture(
    driver: &mut dyn Driver,
    app: &App,
    ctx: &mut Context<'_>,
    name: &str,
) -> Result<Vec<Element>, String> {
    let tree = driver
        .snapshot(app)
        .map_err(|e| format!("snapshot {name}: {e}"))?;
    let file = format!("tree-{name}.json");
    std::fs::write(
        ctx.evidence_dir.join(&file),
        serde_json::to_string_pretty(&tree).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    ctx.evidence.push(file);
    let shot = format!("screenshot-{name}.png");
    driver
        .screenshot(app, &ctx.evidence_dir.join(&shot))
        .map_err(|e| format!("screenshot {name}: {e}"))?;
    ctx.evidence.push(shot);
    let findings = negative_text::findings_outside_profile(&tree);
    if !findings.is_empty() {
        let list: Vec<String> = findings.iter().map(ToString::to_string).collect();
        return Err(format!(
            "developer text on the {name} route: {}",
            list.join("; ")
        ));
    }
    Ok(tree)
}

/// Waits for the process to end after SIGTERM, so its instance socket is
/// gone before the leak check.
pub fn wait_exit(pid: u32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Path::new(&format!("/proc/{pid}")).exists() && Instant::now() < deadline {
        let zombie = std::fs::read_to_string(format!("/proc/{pid}/status"))
            .map(|s| {
                s.lines()
                    .any(|l| l.starts_with("State:") && l.contains('Z'))
            })
            .unwrap_or(true);
        if zombie {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Ends the app through the driver and waits for it to go.
pub fn close(driver: &mut dyn Driver, app: &App) -> Result<(), String> {
    driver.close(app).map_err(|e| format!("close: {e}"))?;
    wait_exit(app.pid, Duration::from_secs(5));
    Ok(())
}

/// The profile's environment plus the app's QA variables.
pub fn app_env(ctx: &Context<'_>, open: &str, route: Option<&str>) -> BTreeMap<String, String> {
    let mut env = ctx.profile.env();
    env.insert("DETTIVO_E2E_OPEN".into(), open.into());
    if let Some(r) = route {
        env.insert("DETTIVO_E2E_ROUTE".into(), r.into());
    }
    // The theme is the desktop's own, as it is for the pill drives: a
    // drive on Omarchy shows the active theme, CI shows the built-in
    // palette, and the theme drives point the variable where they need it.
    env
}

/// The seed daemon's configuration: the mock switches of the profile, the
/// engines beside the build so a dictation can run.
pub fn daemon_config(repo_root: &Path) -> String {
    let bin_dir = super::binary(repo_root, "dettivod")
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n",
        bin_dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_memory_contract_is_320_and_128_mib_inclusive() {
        assert_eq!(RSS_BUDGET_KB, 327_680);
        assert_eq!(ANONYMOUS_BUDGET_KB, 131_072);
        for (rss_kb, anonymous_kb, within) in [
            (327_680, 131_072, true),
            (327_681, 131_072, false),
            (327_680, 131_073, false),
        ] {
            let memory = Memory {
                rss_kb,
                pss_kb: u64::MAX,
                anonymous_kb,
            };
            assert_eq!(memory.within_budget(), within);
            assert_eq!(memory.evidence()["memory_within_budget"], within);
        }
        let memory =
            parse_memory("VmRSS: 327680 kB", "Pss: 1537 kB\nAnonymous: 131072 kB").unwrap();
        let report = memory.evidence();
        assert_eq!(report["rss_budget_mib"], 320);
        assert_eq!(report["anonymous_budget_mib"], 128);
        assert_eq!(report["rss_mib_after_5s"], 320.0);
        assert_eq!(report["anonymous_mib_after_5s"], 128.0);
        assert_eq!(report["pss_mib_after_5s"], 1.5009765625);
        assert_eq!(report["pss_kb_after_5s"], 1537);
        assert!(report["pss_budget_kb"].is_null());
    }

    #[test]
    fn partial_or_malformed_memory_is_unavailable() {
        for (status, rollup) in [
            ("VmRSS: 1024 kB", "Pss: 512 kB"),
            ("VmRSS: 1024 kB", "Anonymous: 256 kB"),
            ("VmRSS: 1024 kB", ""),
            ("VmRSS: 1024 MB", "Pss: 512 kB\nAnonymous: 256 kB"),
            ("VmRSS: 1024 kB", "Pss: bad kB\nAnonymous: 256 kB"),
            ("VmRSS: 1024 kB", "Pss: 512 kB\nAnonymous: 256 MB"),
            ("VmRSS: 1024 kB", "Pss: 512 kB\nAnonymous: -1 kB"),
            ("VmRSS: 1024 kB", "Pss: 512 kB extra\nAnonymous: 256 kB"),
            ("", "Pss: 512 kB\nAnonymous: 256 kB"),
        ] {
            assert_eq!(parse_memory(status, rollup), None, "{status}; {rollup}");
        }
        assert_eq!(memory_kb(u32::MAX), None);
    }

    #[test]
    fn every_route_has_a_distinct_title_and_name() {
        let mut names: Vec<&str> = ROUTES.iter().map(|r| r.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ROUTES.len());
        let mut titles: Vec<&str> = ROUTES.iter().map(|r| r.title).collect();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), ROUTES.len());
        assert_eq!(ROUTES.iter().filter(|r| r.route.is_some()).count(), 8);
        assert!(resident_kb(std::process::id()).unwrap() > 0);
    }
}

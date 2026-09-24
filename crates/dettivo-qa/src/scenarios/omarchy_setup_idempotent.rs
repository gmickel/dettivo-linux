//! `dettivo setup omarchy` is idempotent (fn-35 R5, ADR 0030): run twice
//! against a profile daemon on an Omarchy desktop, the second run reports
//! no failed step and changes nothing, proven by hash over the snippet,
//! the plugin folder it installed and the socket unit's text, and
//! `--check` afterwards reports the socket, the snippet and the plugin
//! `ok`. The command enables the session's socket unit, reloads Hyprland
//! and rescans the live shell's plugins, so like `omarchy_bar` it runs
//! only where `DETTIVO_QA_OMARCHY_LIVE=1` allows a drive against the live
//! desktop; anywhere else it reports why it skipped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::daemon::DaemonHandle;
use super::omarchy_bar::LIVE_VAR;
use super::support::installed;
use super::{Context, Scenario, binary};
use crate::driver::Driver;

/// The scenario.
pub struct OmarchySetupIdempotent;

/// The hashes of everything a setup run touches under the profile.
pub type Hashes = BTreeMap<String, String>;

fn hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// One hash per file under `roots` (recursively), keyed by path, plus one
/// for `unit` (the socket unit's text) under `unit:dettivod.socket`.
pub fn hashes(roots: &[PathBuf], unit: &str) -> Hashes {
    let mut out = Hashes::new();
    for root in roots {
        let mut files = Vec::new();
        if root.is_file() {
            files.push(root.clone());
        } else {
            walk(root, &mut files);
        }
        for file in files {
            let bytes = std::fs::read(&file).unwrap_or_default();
            out.insert(file.to_string_lossy().into_owned(), hex(&bytes));
        }
    }
    out.insert("unit:dettivod.socket".into(), hex(unit.as_bytes()));
    out
}

/// The paths whose hash differs between two runs, or are only in one.
pub fn changed(before: &Hashes, after: &Hashes) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .filter(|k| before.get(*k) != after.get(*k))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The steps of a `setup omarchy` report whose state is `failed`.
pub fn failed_steps(report: &Value) -> Vec<String> {
    report
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, s)| s["state"] == "failed")
        .map(|(k, s)| format!("{k}: {}", s["detail"].as_str().unwrap_or("failed")))
        .collect()
}

/// The `--check` steps that are not `ok`.
pub fn not_ok(check: &Value) -> Vec<String> {
    ["socket", "snippet", "plugin"]
        .iter()
        .filter(|k| check[k]["state"] != "ok")
        .map(|k| format!("{k}: {}", check[k]["state"].as_str().unwrap_or("missing")))
        .collect()
}

impl OmarchySetupIdempotent {
    fn cli_json(
        ctx: &Context<'_>,
        env: &BTreeMap<String, String>,
        args: &[&str],
    ) -> Result<Value, String> {
        let cli = binary(ctx.repo_root, "dettivo")?;
        let output = crate::profile::command(cli, env)
            .arg("--json")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("run dettivo {}: {e}", args.join(" ")))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .find(|l| l.starts_with('{'))
            .and_then(|l| serde_json::from_str(l).ok())
            .ok_or_else(|| {
                format!(
                    "dettivo {} answered no JSON (exit {:?}): {}",
                    args.join(" "),
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr).trim()
                )
            })
    }

    fn unit_text() -> String {
        Command::new("systemctl")
            .args(["--user", "cat", "dettivod.socket"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    }
}

impl Scenario for OmarchySetupIdempotent {
    fn id(&self) -> &'static str {
        "omarchy_setup_idempotent"
    }

    fn summary(&self) -> &'static str {
        "dettivo setup omarchy run twice changes nothing by hash and --check reports every step ok"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn preflight(&self) -> Result<(), String> {
        for program in ["omarchy", "omarchy-shell", "hyprctl", "systemctl"] {
            if !installed(program) {
                return Err(format!("{program} is not on PATH (an Omarchy desktop)"));
            }
        }
        if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
            return Err("not inside a Hyprland session".into());
        }
        if std::env::var(LIVE_VAR).ok().as_deref() != Some("1") {
            return Err(format!(
                "enables the session's socket unit and rescans the live shell; set {LIVE_VAR}=1 to allow it"
            ));
        }
        Ok(())
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let mut extra = BTreeMap::new();
        extra.insert("XDG_CURRENT_DESKTOP".to_string(), "Hyprland".to_string());
        extra.insert(
            "DETTIVO_OMARCHY_PLUGIN_DIR".to_string(),
            ctx.repo_root.join("omarchy").to_string_lossy().into_owned(),
        );
        let _daemon = DaemonHandle::spawn(&dettivod, ctx.profile, "", &extra, ctx.timeout)?;
        ctx.timings.mark("daemon");
        // The profile's Hyprland configuration sources the snippet the way
        // a user's does, so the snippet step can report it as loaded.
        let hypr = ctx.profile.root.join("cfg/hypr");
        std::fs::create_dir_all(&hypr).map_err(|e| e.to_string())?;
        std::fs::write(
            hypr.join("hyprland.conf"),
            "# the user's own configuration\nsource = ~/.config/hypr/dettivo.conf\n",
        )
        .map_err(|e| e.to_string())?;
        let mut env = ctx.profile.env();
        env.extend(extra.clone());
        let roots = vec![
            hypr.join("dettivo.conf"),
            ctx.profile.root.join("cfg/omarchy/plugins"),
        ];

        let first = Self::cli_json(ctx, &env, &["setup", "omarchy"])?;
        let failed = failed_steps(&first);
        if !failed.is_empty() {
            return Err(format!("the first run failed: {}", failed.join("; ")));
        }
        let before = hashes(&roots, &Self::unit_text());
        ctx.timings.mark("first");

        let second = Self::cli_json(ctx, &env, &["setup", "omarchy"])?;
        let failed = failed_steps(&second);
        if !failed.is_empty() {
            return Err(format!("the second run failed: {}", failed.join("; ")));
        }
        let after = hashes(&roots, &Self::unit_text());
        let diff = changed(&before, &after);
        ctx.timings.mark("second");

        let check = Self::cli_json(ctx, &env, &["setup", "omarchy", "--check"])?;
        let missing = not_ok(&check);
        let evidence = json!({
            "first": first,
            "second": second,
            "check": check,
            "hashes": after,
            "changed": diff,
        });
        std::fs::write(
            ctx.evidence_dir.join("setup-idempotent.json"),
            serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("setup-idempotent.json".into());
        if !diff.is_empty() {
            return Err(format!("the second setup run changed: {}", diff.join(", ")));
        }
        if !missing.is_empty() {
            return Err(format!(
                "--check after two runs reports {}",
                missing.join("; ")
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_file_is_named_and_the_reports_are_judged() {
        let dir = tempfile::tempdir().unwrap();
        let snippet = dir.path().join("dettivo.conf");
        let plugin = dir.path().join("plugins/gmickel.dettivo");
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(&snippet, "bind = a\n").unwrap();
        std::fs::write(plugin.join("manifest.json"), "{}").unwrap();
        let roots = vec![snippet.clone(), dir.path().join("plugins")];
        let before = hashes(&roots, "[Socket]\n");
        assert_eq!(before.len(), 3);
        assert!(changed(&before, &hashes(&roots, "[Socket]\n")).is_empty());
        std::fs::write(plugin.join("manifest.json"), "{\"v\": 2}").unwrap();
        let diff = changed(&before, &hashes(&roots, "[Socket]\nListenStream=x\n"));
        assert_eq!(diff.len(), 2);
        assert!(diff[0].ends_with("manifest.json"), "{diff:?}");
        assert_eq!(diff[1], "unit:dettivod.socket");

        let report = json!({
            "socket": {"state": "ok", "detail": "enabled"},
            "plugin": {"state": "failed", "detail": "enabling the plugin: `omarchy plugin enable` exited 1"},
            "shell": {"state": "skipped", "detail": "omarchy-shell is not on PATH"},
        });
        let failed = failed_steps(&report);
        assert_eq!(failed.len(), 1);
        assert!(failed[0].starts_with("plugin: enabling the plugin"));
        let check =
            json!({"socket": {"state": "ok"}, "snippet": {"state": "failed"}, "plugin": {}});
        assert_eq!(not_ok(&check), ["snippet: failed", "plugin: missing"]);
    }
}

//! A poisoned caller must not redirect any profile-backed launch into its files.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use dettivo_qa::driver::{Driver, Launch, atspi::AtspiDriver, cua::CuaDriver};
use dettivo_qa::evidence::Timings;
use dettivo_qa::profile::Profile;
use dettivo_qa::scenarios::{
    Context, daemon::DaemonHandle, first_run_support, support::InsertDaemon,
};

#[test]
fn scenario_overrides_and_removals_are_deliberate() {
    let mut profile = Profile::create("env-override", None).unwrap();
    profile.extend_env(&BTreeMap::from([("DETTIVO_FORCE_CPU".into(), "1".into())]));
    let mut env = profile.env();
    env.remove("DETTIVO_MOCK_MODE");
    env.insert(
        "DETTIVO_CONFIG".into(),
        profile.root.join("scenario.toml").display().to_string(),
    );
    env.insert("DISPLAY".into(), ":986".into());
    let output = dettivo_qa::profile::command("/usr/bin/env", &env)
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let child: BTreeMap<_, _> = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();
    assert!(!child.contains_key("DETTIVO_MOCK_MODE"));
    for key in ["DETTIVO_CONFIG", "DETTIVO_FORCE_CPU", "DISPLAY"] {
        assert_eq!(child[key], env[key]);
    }
    for key in ["XDG_CACHE_HOME", "TMPDIR"] {
        assert!(Path::new(child[key]).is_dir());
        assert!(Path::new(child[key]).starts_with(&profile.root));
    }
}

#[test]
fn profile_launchers_reject_parent_overrides() {
    let mut failures = Vec::new();
    for boundary in [
        "daemon",
        "insert",
        "atspi",
        "cua",
        "cua-tools",
        "cli",
        "render",
        "pacing",
        "contract",
        "polish",
    ] {
        let fixture = tempfile::tempdir().unwrap();
        for dir in ["home", "cfg", "data", "state", "cache", "bin"] {
            std::fs::create_dir(fixture.path().join(dir)).unwrap();
        }
        for file in ["config.toml", "data/sentinel", "cache/sentinel"] {
            std::fs::write(fixture.path().join(file), "untouched").unwrap();
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "launch_boundary_probe", "--nocapture"])
            .env("QA_BOUNDARY", boundary)
            .env("QA_FIXTURE", fixture.path())
            .env("HOME", fixture.path().join("home"))
            .env("XDG_CONFIG_HOME", fixture.path().join("cfg"))
            .env("XDG_DATA_HOME", fixture.path().join("data"))
            .env("XDG_STATE_HOME", fixture.path().join("state"))
            .env("XDG_CACHE_HOME", fixture.path().join("cache"))
            .env("DETTIVO_CONFIG", fixture.path().join("config.toml"))
            .env("DETTIVO_DATA_DIR", fixture.path().join("data"))
            .env("DETTIVO_IPC_TOKEN", "outside-token")
            .env("DETTIVO_MOCK_LLM", "outside-mock")
            .env("UNRELATED_SECRET", "must-not-cross")
            .env("PATH", fixture.path().join("bin"))
            .env("DISPLAY", ":987")
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/qa-bus")
            .env("AT_SPI_BUS_ADDRESS", "unix:path=/nonexistent/qa-a11y")
            .env("PIPEWIRE_REMOTE", "qa-pipewire")
            .env("PULSE_SERVER", "unix:/nonexistent/qa-pulse")
            .output()
            .unwrap();
        if !output.status.success() {
            failures.push(format!(
                "{boundary}: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn launch_boundary_probe() {
    let Ok(boundary) = std::env::var("QA_BOUNDARY") else {
        return;
    };
    let fixture = std::env::var_os("QA_FIXTURE").unwrap();
    let root = Path::new(&fixture);
    let bin = root.join("target/debug");
    std::fs::create_dir_all(&bin).unwrap();
    let script = probe_script(root, "", "printf '{}\\n'");
    for name in ["dettivod", "dettivo", "dettivo-osd", "dettivo-mcp"] {
        let file = bin.join(name);
        std::fs::write(&file, &script).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut profile = Profile::create("env-test", None).unwrap();
    let timeout = Duration::from_millis(50);
    let mut timings = Timings::start();
    let mut evidence = Vec::new();
    let mut ctx = Context {
        profile: &mut profile,
        evidence_dir: root,
        repo_root: root,
        timings: &mut timings,
        evidence: &mut evidence,
        timeout,
    };
    let mut driver: Option<Box<dyn Driver>> = None;
    match boundary.as_str() {
        "daemon" => {
            let _ = DaemonHandle::spawn(
                &bin.join("dettivod"),
                ctx.profile,
                "",
                &BTreeMap::new(),
                timeout,
            );
        }
        "insert" => {
            let _ = InsertDaemon::start(&mut ctx);
        }
        "atspi" | "cua" => {
            let mut d: Box<dyn Driver> = if boundary == "atspi" {
                Box::new(AtspiDriver::new())
            } else {
                Box::new(CuaDriver::new())
            };
            let _ = d.launch(
                &Launch {
                    program: bin.join("dettivo"),
                    args: vec![],
                    env: ctx.profile.env(),
                },
                timeout,
            );
            driver = Some(d);
        }
        "cua-tools" => {
            let file = root.join("bin/cua-driver");
            std::fs::write(
                &file,
                probe_script(
                    root,
                    "-$1",
                    r#"
case "$1" in
  --version) printf 'cua-driver 0.20.0\n';;
  serve) for socket; do :; done; : > "$socket";;
  call) printf '{"atspi":true,"x11":true}\n';;
esac
"#,
                ),
            )
            .unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
            CuaDriver::new().preflight().unwrap();
            for action in ["--version", "serve", "call", "stop"] {
                assert_isolated(root, &boundary, &format!("-{action}"));
            }
            return;
        }
        "cli" => {
            let _ = first_run_support::golden_snippet(&ctx, &BTreeMap::new());
        }
        "render" => {
            use dettivo_qa::visual::{
                matrix::{Baseline, Entry},
                render,
            };
            let entry = Entry {
                args: vec![],
                surface: "osd".into(),
                state: "idle".into(),
                theme: "builtin-dark".into(),
                scale: 1,
                binary: "dettivo-osd".into(),
                env: BTreeMap::new(),
                threshold_milli: 990,
                colour_tolerance_milli: 1,
                baseline: Baseline::Missing(root.join("baseline.png")),
                artboard_crop: None,
                crop: None,
            };
            let _ = render::render(
                root,
                &entry,
                &root.join("out.png"),
                &root.join("render.log"),
                None,
            );
        }
        "pacing" => {
            let _ = dettivo_qa::pacing::run(&bin.join("dettivo-osd"), root, 1);
        }
        "contract" => {
            let _ = Command::new(env!("CARGO_BIN_EXE_dettivo-qa"))
                .arg("--repo")
                .arg(root)
                .arg("contract")
                .output()
                .unwrap();
        }
        "polish" => {
            use dettivo_qa::polish_eval::runner::{Candidate, Options, Runner};
            let _ = Runner::start(
                root,
                &Options {
                    candidate: Candidate::Echo,
                    cpu: true,
                    experiments_dir: None,
                    models_dir: None,
                    timeout_ms: 50,
                    engines_dir: None,
                },
            );
        }
        _ => panic!("unknown boundary"),
    }
    assert_isolated(root, &boundary, "");
    drop(driver);
}

fn probe_script(root: &Path, suffix: &str, answer: &str) -> String {
    format!(
        "#!/bin/sh\n/usr/bin/env > \"{root}/capture{suffix}\"\n\
         if [ -f \"${{DETTIVO_CONFIG:-/nonexistent}}\" ]; then printf changed > \"$DETTIVO_CONFIG\"; fi\n\
         if [ -n \"${{DETTIVO_DATA_DIR:-}}\" ]; then printf changed > \"$DETTIVO_DATA_DIR/sentinel\"; fi\n\
         if [ -f \"${{XDG_CACHE_HOME:-/nonexistent}}/sentinel\" ]; then printf changed > \"$XDG_CACHE_HOME/sentinel\"; fi\n\
         {answer}\n\
         printf done > \"{root}/done{suffix}\"\n",
        root = root.display(),
    )
}

fn assert_isolated(root: &Path, boundary: &str, suffix: &str) {
    let capture = root.join(format!("capture{suffix}"));
    let deadline = Instant::now() + Duration::from_secs(2);
    while !root.join(format!("done{suffix}")).exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let text = std::fs::read_to_string(capture).expect("launcher actually ran the probe");
    for file in ["config.toml", "data/sentinel", "cache/sentinel"] {
        assert_eq!(
            std::fs::read_to_string(root.join(file)).unwrap(),
            "untouched",
            "{boundary}: outside {file}"
        );
    }
    let env: BTreeMap<_, _> = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();
    for key in [
        "DETTIVO_DATA_DIR",
        "UNRELATED_SECRET",
        "QA_FIXTURE",
        "QA_BOUNDARY",
    ] {
        assert!(!env.contains_key(key), "{boundary}: inherited {key}");
    }
    assert_ne!(env.get("DETTIVO_IPC_TOKEN"), Some(&"outside-token"));
    assert_ne!(env.get("DETTIVO_MOCK_LLM"), Some(&"outside-mock"));
    for key in [
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
    ] {
        assert!(
            !Path::new(env[key]).starts_with(root),
            "{boundary}: inherited {key}"
        );
        assert!(
            env[key].starts_with("/tmp/dq"),
            "{boundary}: {key} must be private"
        );
    }
    for (key, value) in [
        ("DISPLAY", ":987"),
        ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/qa-bus"),
        ("AT_SPI_BUS_ADDRESS", "unix:path=/nonexistent/qa-a11y"),
        ("PIPEWIRE_REMOTE", "qa-pipewire"),
        ("PULSE_SERVER", "unix:/nonexistent/qa-pulse"),
    ] {
        assert_eq!(
            env.get(key),
            Some(&value),
            "{boundary}: preserve session {key}"
        );
    }
}

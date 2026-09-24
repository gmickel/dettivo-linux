//! R1 and R6 end to end: `dettivo setup` renders the checked-in goldens
//! for the default and for custom keys, refuses an unknown compositor with
//! exit 4 naming the supported ones and an unparseable chord naming the
//! key and the notation, writes the snippet without touching the main
//! configuration, `--check` reports the include line, and `hotkeys
//! status` plus `doctor` name the backend, the availability reasons and
//! the snippet state.

mod common;

use common::*;
use serde_json::Value;

fn golden(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/goldens")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("golden {name}: {e}"))
}

const CUSTOM_KEYS: &str = "[hotkeys]\nhold = \"SUPER ALT, space\"\ntoggle = \"SUPER ALT, D\"\ncancel = \"SUPER ALT, Escape\"\nreinsert = \"SUPER ALT SHIFT, D\"\n";

#[test]
fn snippets_match_the_goldens_for_default_and_custom_keys() {
    let d = Daemon::spawn(Tree::new(), &[]);
    for (name, file) in [
        ("hyprland-conf", "hyprland.conf"),
        ("hyprland-lua", "hyprland.lua"),
        ("hyprland", "hyprland.conf"),
        ("sway", "sway"),
        ("niri", "niri.kdl"),
    ] {
        let out = d.cli(&["setup", name, "--stdout"]);
        assert_eq!(out.status.code(), Some(0), "{name}: {}", stderr(&out));
        assert_eq!(stdout(&out), golden(file), "{name}");
    }
    // A Lua Hyprland configuration (0.56, Omarchy) selects the Lua flavour.
    let hypr = d.tree.root().join("cfg/hypr");
    std::fs::create_dir_all(&hypr).unwrap();
    std::fs::write(hypr.join("hyprland.lua"), "-- mine\n").unwrap();
    let out = d.cli(&["setup", "hyprland", "--stdout"]);
    assert_eq!(stdout(&out), golden("hyprland.lua"));
    assert!(
        stderr(&out).contains("note: Omarchy binds F9"),
        "{}",
        stderr(&out)
    );

    let tree = Tree::new();
    tree.write_config(CUSTOM_KEYS);
    let d = Daemon::spawn(tree, &[]);
    for (name, file) in [
        ("hyprland-conf", "custom-hyprland.conf"),
        ("hyprland-lua", "custom-hyprland.lua"),
        ("sway", "custom-sway"),
        ("niri", "custom-niri.kdl"),
    ] {
        let out = d.cli(&["setup", name, "--stdout"]);
        assert_eq!(out.status.code(), Some(0), "{name}: {}", stderr(&out));
        assert_eq!(stdout(&out), golden(file), "{name}");
    }
    let niri = golden("custom-niri.kdl");
    assert!(niri.contains("Mod+Alt+D { spawn"));
    assert!(!niri.contains("dictation\" \"start"));
}

#[test]
fn an_unknown_compositor_and_a_bad_chord_exit_4_naming_the_problem() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let out = d.cli(&["setup", "gnome", "--stdout"]);
    assert_eq!(out.status.code(), Some(4), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("unknown compositor \"gnome\"; supported: hyprland, sway, niri"),
        "{}",
        stderr(&out)
    );
    let tree = Tree::new();
    tree.write_config("[hotkeys]\nhold = \"HYPER, X\"\n");
    let d = Daemon::spawn(tree, &[]);
    let out = d.cli(&["setup", "sway", "--stdout"]);
    assert_eq!(out.status.code(), Some(4), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("hotkeys.hold"), "{err}");
    assert!(err.contains("unknown modifier `HYPER`"), "{err}");
    assert!(err.contains("`SUPER CTRL, X`"), "{err}");
    assert_eq!(stdout(&out), "");
}

#[test]
fn setup_writes_the_snippet_and_check_reports_the_include_line_without_editing_the_main_config() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let snippet = d.tree.root().join("cfg/hypr/dettivo.conf");
    let main = d.tree.root().join("cfg/hypr/hyprland.conf");
    let check = d.cli(&["setup", "hyprland", "--check"]);
    assert_eq!(check.status.code(), Some(1), "{}", stderr(&check));
    assert!(
        stdout(&check).contains("snippet   hyprland: not written (run `dettivo setup hyprland`)"),
        "{}",
        stdout(&check)
    );

    let out = d.cli(&["setup", "hyprland"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains(&format!("wrote {}", snippet.display())),
        "{text}"
    );
    assert!(
        text.contains("add to") && text.contains("source = ~/.config/hypr/dettivo.conf"),
        "{text}"
    );
    assert_eq!(
        std::fs::read_to_string(&snippet).unwrap(),
        golden("hyprland.conf")
    );
    assert!(!main.exists(), "the main configuration is never created");

    let check = d.cli(&["setup", "hyprland", "--check"]);
    assert_eq!(check.status.code(), Some(1));
    assert!(
        stdout(&check).contains("written, not sourced (add to"),
        "{}",
        stdout(&check)
    );
    std::fs::write(&main, "# mine\nsource = ~/.config/hypr/dettivo.conf\n").unwrap();
    let check = d.cli(&["setup", "hyprland", "--check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
    assert!(
        stdout(&check).contains("snippet   hyprland: sourced ("),
        "{}",
        stdout(&check)
    );
    let json: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "setup", "hyprland", "--check"]))).unwrap();
    assert_eq!(json["main_config"]["sourced"], true);
    assert_eq!(json["snippet"]["written"], true);
    let again = d.cli(&["setup", "hyprland"]);
    assert_eq!(again.status.code(), Some(0));
    assert!(
        stdout(&again).contains("already loads it"),
        "{}",
        stdout(&again)
    );
    assert_eq!(
        std::fs::read_to_string(&main).unwrap(),
        "# mine\nsource = ~/.config/hypr/dettivo.conf\n",
        "the main configuration is never edited"
    );
}

#[test]
fn hotkeys_status_and_doctor_name_the_backend_the_reasons_and_the_snippet() {
    let d = Daemon::spawn(Tree::new(), &[("XDG_CURRENT_DESKTOP", "sway")]);
    let status = d.cli(&["hotkeys", "status"]);
    assert_eq!(status.status.code(), Some(0), "{}", stderr(&status));
    let text = stdout(&status);
    assert!(
        text.starts_with("backend   none (requested auto)\n"),
        "{text}"
    );
    assert!(text.contains("portal    unavailable: "), "{text}");
    assert!(text.contains("evdev     "), "{text}");
    assert!(text.contains("bound     nothing"), "{text}");
    let json: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "hotkeys", "status"]))).unwrap();
    assert_eq!(json["backend"], "none");
    assert_eq!(json["portal"]["available"], false);

    let doctor = cli_in(&d.tree, &["doctor"], &[("XDG_CURRENT_DESKTOP", "sway")]);
    let text = stdout(&doctor);
    assert!(
        text.contains("hotkeys   backend   none (requested auto)"),
        "{text}"
    );
    assert!(text.contains("hotkeys   portal    unavailable: "), "{text}");
    assert!(
        text.contains("snippet   sway: not written (run `dettivo setup sway`)"),
        "{text}"
    );
    let report: Value = serde_json::from_str(&stdout(&cli_in(
        &d.tree,
        &["--json", "doctor"],
        &[("XDG_CURRENT_DESKTOP", "sway")],
    )))
    .unwrap();
    assert_eq!(report["hotkeys"]["backend"], "none");
    assert_eq!(report["snippet"]["compositor"], "sway");
    assert_eq!(report["snippet"]["main_config"]["sourced"], false);

    let start = d.cli(&["dictation", "start", "--expected-target-pid", "not-a-pid"]);
    assert_eq!(start.status.code(), Some(4), "{}", stderr(&start));
    assert!(
        stderr(&start).contains("expected_target_pid"),
        "{}",
        stderr(&start)
    );
}

#[test]
fn setup_omarchy_prints_its_plan_and_checks_every_step() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let hypr = d.tree.root().join("cfg/hypr");
    std::fs::create_dir_all(&hypr).unwrap();
    std::fs::write(hypr.join("hyprland.lua"), "-- mine\n").unwrap();

    // --stdout prints the Lua snippet and says what the other steps would run.
    let out = d.cli(&["setup", "omarchy", "--stdout"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out), golden("hyprland.lua"));
    let plan = stderr(&out);
    assert!(
        plan.contains("would run: systemctl --user enable --now dettivod.socket"),
        "{plan}"
    );
    assert!(
        plan.contains("omarchy plugin enable gmickel.dettivo right"),
        "{plan}"
    );
    let out = d.cli(&[
        "setup",
        "omarchy",
        "--stdout",
        "--git",
        "https://example.invalid/omarchy-dettivo.git",
    ]);
    assert!(
        stderr(&out).contains(
            "would run: omarchy plugin add https://example.invalid/omarchy-dettivo.git --enable --yes"
        ),
        "{}",
        stderr(&out)
    );
    let out = d.cli(&["setup", "omarchy", "--stdout", "--no-plugin"]);
    assert!(!stderr(&out).contains("omarchy plugin"), "{}", stderr(&out));

    // --check reports every step; the fresh tree has no snippet, so the
    // command exits 1 naming it.
    let out = d.cli(&["--json", "setup", "omarchy", "--check"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let report: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(report["snippet"]["state"], "failed");
    assert_eq!(report["snippet"]["compositor"], "hyprland");
    assert_eq!(report["plugin"]["id"], "gmickel.dettivo");
    assert_eq!(report["plugin"]["installed"], false);
    assert!(
        report["plugin"]["state"] == "skipped" || report["plugin"]["state"] == "failed",
        "{report}"
    );
    assert_eq!(report["panel"]["name"], "dev.dettivo.OmarchyPanel");
    assert!(report["socket"]["state"].is_string(), "{report}");
    assert!(stderr(&out).contains("snippet"), "{}", stderr(&out));

    // doctor carries the same facts on its omarchy line.
    let out = d.cli(&["doctor"]);
    let text = stdout(&out);
    assert!(text.contains("omarchy   shell "), "{text}");
}

#[test]
fn lua_setup_activates_and_reports_reload_failure_for_retry() {
    use std::os::unix::fs::PermissionsExt;
    let d = Daemon::spawn(Tree::new(), &[]);
    let hypr = d.tree.root().join("cfg/hypr");
    std::fs::create_dir_all(&hypr).unwrap();
    let main = hypr.join("hyprland.lua");
    std::fs::write(&main, "require('defaults')\n").unwrap();
    let bin = d.tree.root().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let hyprctl = bin.join("hyprctl");
    std::fs::write(&hyprctl, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&hyprctl, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env = [("PATH", bin.to_str().unwrap())];
    for _ in 0..2 {
        let out = cli_in(&d.tree, &["--json", "setup", "hyprland"], &env);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
        assert_eq!(report["check"]["main_config"]["sourced"], true);
    }
    let text = std::fs::read_to_string(&main).unwrap();
    assert!(text.starts_with("require('defaults')\n"));
    assert_eq!(text.matches("dofile(").count(), 1);
    std::fs::write(&hyprctl, "#!/bin/sh\necho 'reload refused' >&2\nexit 1\n").unwrap();
    let out = cli_in(&d.tree, &["setup", "hyprland"], &env);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("reload refused"));
    assert!(stderr(&out).contains("retry shortcut setup"));
}

#[test]
fn omarchy_setup_installs_plugin_and_reports_every_step_after_activation_failure() {
    use std::os::unix::fs::PermissionsExt;
    for failure in ["reload", "configerrors"] {
        let d = Daemon::spawn(Tree::new(), &[]);
        let hypr = d.tree.root().join("cfg/hypr");
        std::fs::create_dir_all(&hypr).unwrap();
        std::fs::write(hypr.join("hyprland.lua"), "-- mine\n").unwrap();
        let bin = d.tree.root().join("bin");
        std::fs::create_dir(&bin).unwrap();
        for (name, script) in [
            ("systemctl", "exit 0"),
            ("omarchy", "echo \"$*\" >> \"$HOME/plugin-commands\""),
            ("omarchy-shell", "echo \"$*\" >> \"$HOME/shell-commands\""),
            (
                "hyprctl",
                "if [ \"$1\" = \"$SETUP_FAILURE\" ]; then echo 'activation refused'; [ \"$1\" = configerrors ]; fi",
            ),
        ] {
            let path = bin.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../omarchy");
        let out = cli_in(
            &d.tree,
            &["--json", "setup", "omarchy"],
            &[
                ("PATH", bin.to_str().unwrap()),
                ("DETTIVO_OMARCHY_PLUGIN_DIR", source.to_str().unwrap()),
                ("SETUP_FAILURE", failure),
            ],
        );
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
        assert_eq!(report["snippet"]["state"], "failed", "{report}");
        let detail = report["snippet"]["detail"].as_str().unwrap();
        assert!(detail.contains("activation refused"), "{detail}");
        assert!(detail.contains("retry shortcut setup"), "{detail}");
        for name in ["socket", "plugin", "shell"] {
            assert_eq!(report[name]["state"], "ok", "{report}");
        }
        assert_eq!(
            report["hyprland"]["state"],
            if failure == "reload" { "failed" } else { "ok" }
        );
        assert!(
            d.tree
                .root()
                .join("cfg/omarchy/plugins/gmickel.dettivo/manifest.json")
                .is_file()
        );
        assert_eq!(
            std::fs::read_to_string(d.tree.root().join("plugin-commands")).unwrap(),
            "plugin list --json\nplugin enable gmickel.dettivo right\n"
        );
        assert_eq!(
            std::fs::read_to_string(d.tree.root().join("shell-commands")).unwrap(),
            "-q shell rescanPlugins\nshell rescanPlugins\n"
        );
        assert!(stderr(&out).contains("steps failed:"));
        assert!(stderr(&out).contains("snippet"));
    }
}

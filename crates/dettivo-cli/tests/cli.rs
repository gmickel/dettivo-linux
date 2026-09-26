//! R6 end to end: the `dettivo` binary against a real daemon. Every
//! implemented method is reachable 1:1, `--json` and `--quiet` behave, the
//! socket and token overrides are honoured, and the exit codes are the
//! contract's: 2 unreachable, 3 refused, 4 bad arguments, 5 timeout.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::thread;
use std::time::{Duration, Instant};

use common::*;
use serde_json::Value;

#[test]
fn status_commands_map_to_system_methods_with_human_json_and_quiet_output() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let ping = d.cli(&["status", "ping"]);
    assert_eq!(ping.status.code(), Some(0), "{}", stderr(&ping));
    assert_eq!(stdout(&ping), "ok\n");

    let json = d.cli(&["--json", "status", "ping"]);
    assert_eq!(stdout(&json), "{\"ok\":true}\n");

    let quiet = d.cli(&["--quiet", "status", "health"]);
    assert_eq!(quiet.status.code(), Some(0));
    assert_eq!(stdout(&quiet), "");

    let health = d.cli(&["status", "health"]);
    assert!(
        stdout(&health).contains("recording_state: idle"),
        "{}",
        stdout(&health)
    );

    let version: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "status", "version"]))).unwrap();
    assert_eq!(version["api_version"], "1.0.0");

    let caps: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "status", "capabilities"]))).unwrap();
    assert_eq!(caps["auth"]["ipc_mode"], "peer");
    assert_eq!(caps["platform"]["os"], "linux");
}

#[test]
fn call_reaches_any_method_one_to_one_and_reports_daemon_errors() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let ok = d.cli(&["--json", "call", "system.version"]);
    assert_eq!(ok.status.code(), Some(0));
    assert!(stdout(&ok).contains("\"api_version\""));

    let not_impl = d.cli(&["--json", "call", "knowledge.search"]);
    assert_eq!(not_impl.status.code(), Some(1));
    assert!(stderr(&not_impl).contains("NOT_IMPLEMENTED"));
    let body: Value = serde_json::from_str(&stdout(&not_impl)).unwrap();
    assert_eq!(body["error"]["data"]["app_code"], "NOT_IMPLEMENTED");

    let bad_params = d.cli(&["call", "system.ping", "{\"stray\":1}"]);
    assert_eq!(bad_params.status.code(), Some(4));
    let not_json = d.cli(&["call", "system.ping", "not-json"]);
    assert_eq!(not_json.status.code(), Some(4));
}

#[test]
fn config_commands_round_trip_through_the_daemon() {
    let tree = Tree::new();
    tree.write_config("# mine\n[daemon]\nlog_level = \"warn\"\n");
    let d = Daemon::spawn(tree, &[]);
    let get = d.cli(&["config", "get", "daemon.log_level"]);
    assert_eq!(stdout(&get), "daemon.log_level = \"warn\"  (file)\n");

    let set = d.cli(&["config", "set", "daemon.log_level", "debug"]);
    assert_eq!(set.status.code(), Some(0), "{}", stderr(&set));
    assert_eq!(
        std::fs::read_to_string(d.tree.config_file()).unwrap(),
        "# mine\n[daemon]\nlog_level = \"debug\"\n"
    );
    let bad = d.cli(&["config", "set", "daemon.log_level", "loud"]);
    assert_eq!(bad.status.code(), Some(4));
    assert!(stderr(&bad).contains("unknown variant"), "{}", stderr(&bad));

    let unset = d.cli(&["--json", "config", "unset", "daemon.log_level"]);
    let entry: Value = serde_json::from_str(&stdout(&unset)).unwrap();
    assert_eq!(entry["source"], "default");

    let all = d.cli(&["config", "get"]);
    assert!(stdout(&all).lines().count() >= 9, "{}", stdout(&all));
    let path: Value = serde_json::from_str(&stdout(&d.cli(&["--json", "config", "path"]))).unwrap();
    assert_eq!(path["socket"], d.tree.socket().to_str().unwrap());
    let validate = d.cli(&["config", "validate"]);
    assert_eq!(validate.status.code(), Some(0));
    assert!(stdout(&validate).starts_with("ok: "));
    let default = d.cli(&["config", "print-default"]);
    assert!(stdout(&default).starts_with("# Dettivo configuration"));

    std::fs::write(d.tree.config_file(), "[daemon]\nlog_level = 5\n").unwrap();
    let invalid = d.cli(&["config", "validate"]);
    assert_eq!(invalid.status.code(), Some(1));
    assert!(
        stdout(&invalid).contains(":2: daemon.log_level:"),
        "{}",
        stdout(&invalid)
    );
}

#[test]
fn config_edit_runs_the_editor_and_validates() {
    let d = Daemon::spawn(Tree::new(), &[]);
    // A fake editor: a script that overwrites the file it is handed. The
    // CLI runs the editor directly (split on whitespace, no shell), so an
    // argument after the program name must survive too.
    let script = d.tree.root().join("fake-editor.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\n[ \"$1\" = --wait ] || exit 9\nprintf '[qa]\\nmode = true\\n' > \"$2\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let editor = format!("{} --wait", script.display());
    let out = cli_in(
        &d.tree,
        &["config", "edit"],
        &[("VISUAL", ""), ("EDITOR", editor.as_str())],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        std::fs::read_to_string(d.tree.config_file()).unwrap(),
        "[qa]\nmode = true\n"
    );
    assert_eq!(
        stdout(&d.cli(&["config", "get", "qa.mode"])),
        "qa.mode = true  (file)\n"
    );
}

#[test]
fn unreachable_daemon_exits_2_naming_the_socket_unit() {
    let tree = Tree::new();
    let out = cli_in(&tree, &["status", "ping"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("dettivod.socket"), "{}", stderr(&out));
    assert!(stderr(&out).contains(tree.socket().to_str().unwrap()));
    let override_out = cli_in(
        &tree,
        &["--socket", "/nonexistent/x.sock", "status", "ping"],
        &[],
    );
    assert!(stderr(&override_out).contains("/nonexistent/x.sock"));
}

#[test]
fn unknown_commands_and_bad_flags_exit_4_with_usage() {
    let tree = Tree::new();
    let out = cli_in(&tree, &["frobnicate"], &[]);
    assert_eq!(out.status.code(), Some(4));
    assert!(stderr(&out).contains("Usage"), "{}", stderr(&out));
    let out = cli_in(&tree, &["status"], &[]);
    assert_eq!(out.status.code(), Some(4));
    let out = cli_in(&tree, &["--bogus", "status", "ping"], &[]);
    assert_eq!(out.status.code(), Some(4));
    let help = cli_in(&tree, &["--help"], &[]);
    assert_eq!(help.status.code(), Some(0));
    let version = cli_in(&tree, &["--version"], &[]);
    assert_eq!(version.status.code(), Some(0));
    assert!(stdout(&version).starts_with("dettivo "));
}

#[test]
fn a_silent_server_exits_5_on_timeout() {
    let tree = Tree::new();
    let socket = tree.socket();
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let listener = UnixListener::bind(&socket).unwrap();
    let hold = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        let _ = reader.read_line(&mut line);
        thread::sleep(Duration::from_millis(1500));
        let _ = stream.write_all(b"\n");
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest);
    });
    let start = Instant::now();
    let out = cli_in(&tree, &["--timeout-ms", "300", "status", "ping"], &[]);
    assert_eq!(out.status.code(), Some(5), "{}", stderr(&out));
    assert!(start.elapsed() < Duration::from_millis(1400));
    assert!(stderr(&out).contains("timed out"), "{}", stderr(&out));
    let _ = hold.join();
}

#[test]
fn token_mode_refuses_without_the_token_and_honours_the_overrides() {
    let tree = Tree::new();
    tree.write_config("[ipc]\nauth_mode = \"peer_token\"\n");
    let d = Daemon::spawn(tree, &[("DETTIVO_IPC_TOKEN", "s3cret")]);
    let refused = d.cli(&["status", "ping"]);
    assert_eq!(refused.status.code(), Some(3), "{}", stderr(&refused));
    assert!(stderr(&refused).contains("UNAUTHORIZED_CLIENT"));

    let wrong = d.cli(&["--token", "nope", "status", "ping"]);
    assert_eq!(wrong.status.code(), Some(3));

    let flag = d.cli(&["--token", "s3cret", "status", "ping"]);
    assert_eq!(flag.status.code(), Some(0), "{}", stderr(&flag));

    let env = cli_in(
        &d.tree,
        &["status", "ping"],
        &[("DETTIVO_IPC_TOKEN", "s3cret")],
    );
    assert_eq!(env.status.code(), Some(0));

    let token_file = d.tree.root().join("tok");
    std::fs::write(&token_file, "s3cret\n").unwrap();
    let file = d.cli(&[
        "--token-file",
        token_file.to_str().unwrap(),
        "status",
        "ping",
    ]);
    assert_eq!(file.status.code(), Some(0), "{}", stderr(&file));

    let caps: Value = serde_json::from_str(&stdout(&d.cli(&[
        "--token",
        "s3cret",
        "--json",
        "status",
        "capabilities",
    ])))
    .unwrap();
    assert_eq!(caps["auth"]["ipc_mode"], "peer_token");

    let socket = d.tree.socket();
    let via_env = cli_in(
        &Tree::new(),
        &["--token", "s3cret", "status", "ping"],
        &[("DETTIVO_IPC_SOCKET", socket.to_str().unwrap())],
    );
    assert_eq!(via_env.status.code(), Some(0), "{}", stderr(&via_env));
}

#[test]
fn doctor_reports_facts_and_exits_by_health() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let out = d.cli(&["doctor"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    for heading in [
        "socket", "service", "daemon", "config", "platform", "backend", "env",
    ] {
        assert!(text.contains(heading), "{text}");
    }
    assert!(
        text.contains("virtual_keyboard") && text.contains("unavailable"),
        "{text}"
    );
    let json: Value = serde_json::from_str(&stdout(&d.cli(&["--json", "doctor"]))).unwrap();
    assert_eq!(json["daemon"]["reachable"], true);
    assert_eq!(json["config"]["ok"], true);
    assert_eq!(json["platform"]["os"], "linux");

    let down = cli_in(&Tree::new(), &["doctor"], &[]);
    assert_eq!(down.status.code(), Some(1));
    assert!(stdout(&down).contains("unreachable"));
}

#[test]
fn speech_commands_map_to_the_speech_methods() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let status: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "speech", "status"]))).unwrap();
    assert!(status["models"].as_array().is_some_and(|m| m.len() > 5));
    assert_eq!(status["catalogue_version"], 1);
    let narrowed: Value = serde_json::from_str(&stdout(&d.cli(&[
        "--json",
        "speech",
        "status",
        "--provider",
        "parakeet",
    ])))
    .unwrap();
    assert!(
        narrowed["models"]
            .as_array()
            .is_some_and(|m| m.iter().all(|r| r["provider"] == "parakeet"))
    );

    let providers: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "speech", "providers"]))).unwrap();
    assert_eq!(providers["default_provider_id"], "whisper");

    let selection: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "speech", "selection"]))).unwrap();
    assert_eq!(selection["provider_id"], "whisper");
    assert_eq!(selection["model"], "large-v3-turbo");
    let set: Value = serde_json::from_str(&stdout(&d.cli(&[
        "--json",
        "speech",
        "selection",
        "set",
        "--model",
        "tiny.en",
    ])))
    .unwrap();
    assert_eq!(set["dictation_model_id"], "tiny.en");
    let get: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "speech", "selection", "get"]))).unwrap();
    assert_eq!(get["model"], "tiny.en");

    let bad = d.cli(&["speech", "delete", "--model", "nope"]);
    assert_eq!(bad.status.code(), Some(4), "{}", stderr(&bad));
    assert!(stderr(&bad).contains("whisper/nope"));
    let human = d.cli(&["speech", "engines"]);
    assert_eq!(human.status.code(), Some(0));
    assert!(stdout(&human).contains("dettivo-engine-whisper"));

    let doctor: Value = serde_json::from_str(&stdout(&d.cli(&["--json", "doctor"]))).unwrap();
    assert!(doctor["models"]["dir"].is_string(), "{doctor}");
    assert_eq!(doctor["models"]["selected"]["id"], "tiny.en");
}

#[test]
fn dictation_and_events_verbs_map_to_their_methods() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let status: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "dictation", "status"]))).unwrap();
    assert_eq!(status["is_active"], false);
    let stop = d.cli(&["dictation", "stop"]);
    assert_eq!(stop.status.code(), Some(1), "{}", stderr(&stop));
    assert!(stderr(&stop).contains("no dictation session"));
    // The default model is not on disk: start names it and the command.
    let start = d.cli(&["dictation", "start", "--language", "en"]);
    assert_eq!(start.status.code(), Some(1));
    assert!(
        stderr(&start).contains("dettivo speech download --model large-v3-turbo"),
        "{}",
        stderr(&start)
    );
    let toggle = d.cli(&["dictation", "toggle"]);
    assert_eq!(toggle.status.code(), Some(1));
    let last = d.cli(&["dictation", "reinsert-last"]);
    assert_eq!(last.status.code(), Some(1));
    assert!(stderr(&last).contains("nothing has been dictated"));
    // events --follow subscribes; with no event coming, --count 0 waits, so
    // a wrong topic proves the subscription path answers.
    let bad = d.cli(&["events", "--follow", "--topic", "bogus.topic"]);
    assert_eq!(bad.status.code(), Some(4), "{}", stderr(&bad));
    assert!(stderr(&bad).contains("unknown event topic"));
    let health: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "status", "health"]))).unwrap();
    assert_eq!(health["recording_state"], "idle");
}

#[test]
fn meetings_segments_lists_the_transcript_by_side_and_span() {
    let d = Daemon::spawn(
        Tree::new(),
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")],
    );
    let sample = "0f8fad5b-d9cb-469f-a165-70867728950e";
    let lines = stdout(&d.cli(&["meetings", "segments", sample]));
    assert_eq!(lines, " you    00:00.000-00:01.200  Hello.\n");
    // --json is the whole answer: the cursor, the transcript it read and
    // every segment with its side.
    let raw: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "meetings", "segments", sample]))).unwrap();
    assert_eq!(raw["transcript"], "stored");
    assert_eq!(raw["cursor"], "stored:1");
    assert_eq!(raw["segments"][0]["source_type"], "microphone");
    assert_eq!(raw["segments"][0]["source"], "you");
    assert_eq!(raw["provisional"], serde_json::json!([]));
    // --since the cursor: nothing new, and nothing printed.
    let since = d.cli(&["meetings", "segments", sample, "--since", "stored:1"]);
    assert!(since.status.success(), "{}", stderr(&since));
    assert_eq!(stdout(&since), "");
    let bad = d.cli(&["meetings", "segments", sample, "--since", "later"]);
    assert_eq!(bad.status.code(), Some(4), "{}", stderr(&bad));
    assert!(stderr(&bad).contains("live:<n>"), "{}", stderr(&bad));
    // A settled meeting under --follow prints the transcript at once.
    let followed = stdout(&d.cli(&["meetings", "segments", sample, "--follow"]));
    assert_eq!(followed, lines);
    let missing = d.cli(&[
        "meetings",
        "segments",
        "00000000-0000-0000-0000-000000000000",
    ]);
    assert_eq!(missing.status.code(), Some(1), "{}", stderr(&missing));
}

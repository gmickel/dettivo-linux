//! R1 and R3 end to end: the server binary driven in both framings
//! against a live seeded daemon through the shared harness, the wire
//! fixtures for `initialize` in each framing, and the actionable texts
//! for a daemon that is down and a token the daemon refuses.

mod common;

use std::path::Path;

use common::*;
use dettivo_mcp::harness::{self, Session, Verdict};
use dettivo_mcp::messages::{ACTION_HARDENED, ACTION_UNAVAILABLE};
use dettivo_mcp::transport::{Framing, encode};
use serde_json::json;

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/framing"))
}

#[test]
fn every_step_passes_in_both_framings_against_the_seeded_daemon() {
    // With the local test model the import_audio step runs live through
    // the real engine; without it the step reports skip, never pass.
    let local = local_model();
    let tree = match &local {
        Some((models, _)) => tree_with_models(models),
        None => Tree::new(),
    };
    let d = Daemon::spawn(tree, &[]);
    let fixture = local.as_ref().map(|(_, wav)| wav.as_path());
    let rows = harness::run_both_with(Path::new(SERVER), &d.tree.socket(), fixture);
    let failed: Vec<&harness::Row> = rows.iter().filter(|r| r.verdict == Verdict::Fail).collect();
    assert!(failed.is_empty(), "{}", harness::human(&rows));
    let imports: Vec<&harness::Row> = rows.iter().filter(|r| r.step == "import_audio").collect();
    assert_eq!(imports.len(), 2);
    for row in imports {
        if fixture.is_some() {
            assert_eq!(row.verdict, Verdict::Pass, "{row:?}");
        } else {
            assert_eq!(row.verdict, Verdict::Skip, "{row:?}");
            assert!(
                row.detail
                    .as_deref()
                    .unwrap_or("")
                    .contains("fetch-test-model")
            );
        }
    }
    let steps = harness::steps().len();
    assert_eq!(rows.len(), steps * 2);
    assert_eq!(
        rows.iter()
            .filter(|r| r.framing == "line-delimited")
            .count(),
        steps
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.framing == "content-length")
            .count(),
        steps
    );
}

#[test]
fn the_initialize_exchange_matches_the_wire_fixture_per_framing() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": harness::PROTOCOL_VERSION, "capabilities": {}, "clientInfo": {"name": "harness", "version": "0"}}});
    for (framing, name) in [
        (Framing::Line, "initialize.line-delimited"),
        (Framing::ContentLength, "initialize.content-length"),
    ] {
        let mut session = Session::spawn(
            harness::server_command(Path::new(SERVER), &d.tree.socket()),
            framing,
        )
        .unwrap();
        let wire = encode(framing, &request);
        session.write_raw(&wire).unwrap();
        let response = session.read().unwrap();
        let expected_request = std::fs::read(fixtures().join(format!("{name}.request"))).unwrap();
        let expected_response = std::fs::read(fixtures().join(format!("{name}.response"))).unwrap();
        assert_eq!(wire, expected_request, "{name}.request");
        assert_eq!(
            encode(framing, &response),
            expected_response,
            "{name}.response"
        );
    }
}

#[test]
fn a_daemon_that_is_down_answers_the_unavailable_text_and_the_session_survives() {
    let tree = Tree::new();
    let mut session = Session::spawn(
        harness::server_command(Path::new(SERVER), &tree.socket()),
        Framing::Line,
    )
    .unwrap();
    let init = session
        .result("initialize", json!({"protocolVersion": "2024-11-05"}))
        .unwrap();
    assert_eq!(init["serverInfo"]["name"], "dettivo-mcp");
    let listed = session.result("tools/list", json!({})).unwrap();
    assert_eq!(listed["tools"].as_array().unwrap().len(), 19);
    for tool in ["get_status", "list_transcripts"] {
        let r = session.call_tool(tool, json!({})).unwrap();
        assert_eq!(r["isError"], true);
        let text = Session::text_of(&r);
        assert!(
            text.starts_with(&format!(
                "Dettivo daemon/IPC unavailable (socket: {}",
                tree.socket().display()
            )),
            "{text}"
        );
        assert!(text.ends_with(ACTION_UNAVAILABLE), "{text}");
        assert!(text.contains("dettivod.socket"), "{text}");
    }
    let read = session
        .request("resources/read", json!({"uri": "status://current"}))
        .unwrap();
    assert_eq!(read["error"]["code"], -32603);
    assert!(
        read["error"]["message"]
            .as_str()
            .unwrap()
            .ends_with(ACTION_UNAVAILABLE)
    );
    assert_eq!(session.result("ping", json!({})).unwrap(), json!({}));
}

#[test]
fn a_refused_token_answers_the_hardened_text() {
    let tree = Tree::new();
    tree.write_config("[ipc]\nauth_mode = \"peer_token\"\n");
    let d = Daemon::spawn(tree, &[("DETTIVO_IPC_TOKEN", "s3cret")]);
    let mut command = harness::server_command(Path::new(SERVER), &d.tree.socket());
    command.arg("--token").arg("wrong");
    let mut session = Session::spawn(command, Framing::ContentLength).unwrap();
    session.result("initialize", json!({})).unwrap();
    let r = session.call_tool("get_status", json!({})).unwrap();
    assert_eq!(r["isError"], true);
    let text = Session::text_of(&r);
    assert!(text.contains("UNAUTHORIZED_CLIENT"), "{text}");
    assert!(text.ends_with(ACTION_HARDENED), "{text}");

    let mut command = harness::server_command(Path::new(SERVER), &d.tree.socket());
    command.env("DETTIVO_IPC_TOKEN", "s3cret");
    let mut session = Session::spawn(command, Framing::Line).unwrap();
    let r = session.call_tool("get_status", json!({})).unwrap();
    assert!(r.get("isError").is_none(), "{r}");
    assert_eq!(
        r["structuredContent"]["capabilities"]["auth"]["ipc_mode"],
        "peer_token"
    );
}

//! R4 on the command line: `dettivo mcp config` matches the goldens for
//! the four hosts, plain and hardened; `--write` merges into a seeded
//! file and keeps the other servers; an unknown host exits 4 naming the
//! hosts; `dettivo mcp check` reports the tool count and the framing
//! against the live daemon; `dettivo mcp serve` speaks MCP over stdio.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use common::*;
use serde_json::Value;

const HOSTS: &[&str] = &["claude-desktop", "claude-code", "cursor", "codex"];

fn goldens() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/goldens"))
}

fn config_args(host: &str, hardened: bool) -> Vec<&str> {
    let mut args = vec![
        "mcp",
        "config",
        "--host",
        host,
        "--command",
        "/usr/bin/dettivo",
    ];
    if hardened {
        args.push("--hardened");
    }
    args
}

#[test]
fn config_matches_the_goldens_for_every_host_plain_and_hardened() {
    let tree = Tree::new();
    let write = std::env::var_os("FN18_WRITE_GOLDENS").is_some();
    for host in HOSTS {
        for hardened in [false, true] {
            let out = cli_in(&tree, &config_args(host, hardened), &[("CODEX_HOME", "")]);
            assert_eq!(out.status.code(), Some(0), "{host}: {}", stderr(&out));
            let text = stdout(&out).replace(tree.root().to_str().unwrap(), "$HOME");
            let name = format!("mcp-{host}{}.txt", if hardened { "-hardened" } else { "" });
            let path = goldens().join(&name);
            if write {
                std::fs::write(&path, &text).unwrap();
            }
            let expected = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(text, expected, "{name}");
        }
    }
    let json: Value = serde_json::from_str(&stdout(&cli_in(
        &tree,
        &[
            "--json",
            "mcp",
            "config",
            "--host",
            "cursor",
            "--command",
            "/usr/bin/dettivo",
        ],
        &[],
    )))
    .unwrap();
    assert_eq!(json["server"]["args"], serde_json::json!(["mcp", "serve"]));
    assert!(
        json["path"]
            .as_str()
            .unwrap()
            .ends_with("/.cursor/mcp.json")
    );
}

#[test]
fn write_merges_into_the_existing_file_and_keeps_other_servers() {
    let tree = Tree::new();
    let cursor = tree.root().join(".cursor/mcp.json");
    std::fs::create_dir_all(cursor.parent().unwrap()).unwrap();
    std::fs::write(
        &cursor,
        "{\"mcpServers\": {\"other\": {\"command\": \"x\", \"args\": []}}, \"keep\": 1}\n",
    )
    .unwrap();
    let out = cli_in(
        &tree,
        &[
            "mcp",
            "config",
            "--host",
            "cursor",
            "--command",
            "/usr/bin/dettivo",
            "--write",
            "--hardened",
        ],
        &[],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("Wrote MCP config to"),
        "{}",
        stdout(&out)
    );
    assert!(stdout(&out).contains("Remember to set DETTIVO_IPC_TOKEN"));
    let merged: Value = serde_json::from_str(&std::fs::read_to_string(&cursor).unwrap()).unwrap();
    assert_eq!(merged["mcpServers"]["other"]["command"], "x");
    assert_eq!(
        merged["mcpServers"]["dettivo"]["command"],
        "/usr/bin/dettivo"
    );
    assert_eq!(
        merged["mcpServers"]["dettivo"]["env"]["DETTIVO_IPC_TOKEN"],
        "<set-me>"
    );
    assert_eq!(merged["keep"], 1);

    let codex = tree.root().join("codex/config.toml");
    std::fs::create_dir_all(codex.parent().unwrap()).unwrap();
    std::fs::write(
        &codex,
        "model = \"o3\"\n\n[mcp_servers.other]\ncommand = \"x\"\n",
    )
    .unwrap();
    let out = cli_in(
        &tree,
        &[
            "mcp",
            "config",
            "--host",
            "codex",
            "--command",
            "/usr/bin/dettivo",
            "--write",
        ],
        &[("CODEX_HOME", codex.parent().unwrap().to_str().unwrap())],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = std::fs::read_to_string(&codex).unwrap();
    assert!(text.starts_with("model = \"o3\"\n"), "{text}");
    assert!(
        text.contains("[mcp_servers.other]\ncommand = \"x\"\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "[mcp_servers.dettivo]\ncommand = \"/usr/bin/dettivo\"\nargs = [\"mcp\", \"serve\"]\n"
        ),
        "{text}"
    );

    let project = tree.root().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let mut cmd = Command::new(CLI);
    tree.env(&mut cmd);
    let out = cmd
        .current_dir(&project)
        .args([
            "mcp",
            "config",
            "--host",
            "claude-code",
            "--command",
            "/usr/bin/dettivo",
            "--write",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let mcp_json: Value =
        serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp_json["mcpServers"]["dettivo"]["type"], "stdio");
}

#[test]
fn an_unknown_host_exits_4_naming_the_hosts() {
    let tree = Tree::new();
    let out = cli_in(&tree, &["mcp", "config", "--host", "vscode"], &[]);
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(
        stderr(&out),
        "dettivo: vscode is not a host (claude-desktop, claude-code, cursor, codex)\n"
    );
}

#[test]
fn check_reports_the_tool_count_and_the_framing_against_the_daemon() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let out = d.cli(&["mcp", "check"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.starts_with("server    dettivo-mcp 1.0.0 (api 1.0.0)\n"),
        "{text}"
    );
    assert!(text.contains("tools     20\n"), "{text}");
    assert!(text.contains("resources 6 templates\n"), "{text}");
    // The framing line is what the served child answered, per framing.
    assert!(
        text.contains("framing   line-delimited: initialize, tools/list 20, get_status; content-length: initialize, tools/list 20, get_status"),
        "{text}"
    );
    let json: Value = serde_json::from_str(&stdout(&d.cli(&["--json", "mcp", "check"]))).unwrap();
    assert_eq!(json["tools"], 20);
    assert_eq!(
        json["framing"],
        serde_json::json!(["line-delimited", "content-length"])
    );
    assert_eq!(json["transport"][0]["initialize"], true);
    assert_eq!(json["transport"][1]["tools_listed"], 20);
    assert_eq!(json["transport"][1]["get_status"], true);
    assert_eq!(json["hardened"], false);

    let down = cli_in(&Tree::new(), &["mcp", "check"], &[]);
    assert_eq!(down.status.code(), Some(2));
}

#[test]
fn hardened_by_config_adds_the_token_placeholder_without_the_flag() {
    let tree = Tree::new();
    tree.write_config("[mcp]\nhardened = true\n");
    let d = Daemon::spawn(tree, &[]);
    let json: Value = serde_json::from_str(&stdout(&d.cli(&[
        "--json",
        "mcp",
        "config",
        "--host",
        "cursor",
        "--command",
        "/usr/bin/dettivo",
    ])))
    .unwrap();
    assert_eq!(json["server"]["env"]["DETTIVO_IPC_TOKEN"], "<set-me>");
    let check: Value = serde_json::from_str(&stdout(&d.cli(&["--json", "mcp", "check"]))).unwrap();
    assert_eq!(check["hardened"], true);
}

#[test]
fn serve_speaks_mcp_over_stdio() {
    let d = Daemon::spawn(Tree::new(), &[]);
    let mut cmd = Command::new(CLI);
    d.tree.env(&mut cmd);
    let mut child = cmd
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-03-26\"}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n")
        .unwrap();
    drop(stdin);
    let reader = BufReader::new(child.stdout.take().unwrap());
    let lines: Vec<Value> = reader
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], "dettivo-mcp");
    assert_eq!(lines[1]["result"]["tools"].as_array().unwrap().len(), 20);
    assert!(child.wait().unwrap().success());
}

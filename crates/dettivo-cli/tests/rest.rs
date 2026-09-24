//! R4 on the command line: `dettivo rest token` names the source and
//! never the token, `dettivo rest status` reports a daemon without a
//! listener as unavailable, and `dettivo rest serve` hosts the shim as a
//! process that answers over the socket.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use common::*;

const TOKEN: &str = "cli-rest-token-9876";

#[test]
fn token_names_its_source_and_status_reports_no_listener() {
    let tree = Tree::new();
    tree.write_config("[hotkeys]\nbackend = \"none\"\n");
    let d = Daemon::spawn(tree, &[]);

    let none = d.cli(&["rest", "token"]);
    assert_eq!(none.status.code(), Some(1), "{}", stderr(&none));
    assert!(
        stdout(&none).contains("DETTIVO_IPC_TOKEN"),
        "{}",
        stdout(&none)
    );

    let env = cli_in(&d.tree, &["rest", "token"], &[("DETTIVO_IPC_TOKEN", TOKEN)]);
    assert_eq!(env.status.code(), Some(0), "{}", stderr(&env));
    assert_eq!(stdout(&env).trim(), "token from environment");
    assert!(!stdout(&env).contains(TOKEN));

    let file = d.tree.root().join("cfg/dettivo/ipc.token");
    std::fs::write(&file, format!("{TOKEN}\n")).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let from_file = d.cli(&["rest", "token"]);
    assert_eq!(from_file.status.code(), Some(0), "{}", stderr(&from_file));
    assert!(
        stdout(&from_file).starts_with("token from file "),
        "{}",
        stdout(&from_file)
    );
    assert!(!stdout(&from_file).contains(TOKEN));

    let status = d.cli(&["rest", "status"]);
    assert_eq!(status.status.code(), Some(2), "{}", stderr(&status));
    assert!(
        stdout(&status).contains("not listening"),
        "{}",
        stdout(&status)
    );
    assert!(
        stdout(&status).contains("token     from file"),
        "{}",
        stdout(&status)
    );
}

#[test]
fn serve_hosts_the_shim_as_a_process_over_the_socket() {
    let tree = Tree::new();
    tree.write_config("[hotkeys]\nbackend = \"none\"\n");
    let d = Daemon::spawn(tree, &[]);
    let mut cmd = Command::new(CLI);
    d.tree.env(&mut cmd);
    let mut child = cmd
        .env("DETTIVO_IPC_TOKEN", TOKEN)
        .args(["rest", "serve", "--port", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut addr = None;
    for line in lines.by_ref().map_while(Result::ok) {
        if let Some(rest) = line.strip_prefix("dettivo rest: listening on http://") {
            addr = Some(rest.trim().to_string());
            break;
        }
    }
    let addr = addr.expect("the listening line");
    let mut stream = TcpStream::connect(&addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .write_all(
            format!(
                "GET /v1/system/version HTTP/1.1\r\nHost: {addr}\r\nX-Dettivo-Token: {TOKEN}\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    assert!(raw.starts_with("HTTP/1.1 200 OK"), "{raw}");
    assert!(raw.contains("\"api_version\""), "{raw}");
    let _ = child.kill();
    let _ = child.wait();
}

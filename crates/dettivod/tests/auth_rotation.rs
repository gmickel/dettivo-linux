//! daemon/F9: the authentication mode and the expected token are read per
//! request, so a connection that opened before a rotation or a mode change
//! follows the reload instead of keeping the token it saw at open.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use common::{Conn, Daemon, Tree};
use serde_json::{Value, json};

fn write_token(tree: &Tree, token: &str) {
    let path = tree.root().join("cfg/dettivo/ipc.token");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("{token}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn ping(conn: &mut Conn, token: Option<&str>) -> Value {
    let mut request = json!({"jsonrpc":"2.0","id":"1","method":"system.ping","params":{}});
    if let Some(t) = token {
        request["auth_token"] = Value::String(t.into());
    }
    conn.send(&request.to_string())
}

fn wait_until(budget: Duration, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + budget;
    while !check() {
        assert!(
            Instant::now() < deadline,
            "the reload did not reach the connection"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn an_open_connection_follows_a_mode_change_and_a_token_rotation() {
    let tree = Tree::new();
    tree.write_config("[ipc]\nauth_mode = \"peer\"\n");
    write_token(&tree, "first-token");
    let daemon = Daemon::spawn(tree, &[]);
    let mut open = Conn::open(&daemon.tree.socket());
    let first = ping(&mut open, None);
    assert!(
        first["error"].is_null(),
        "peer mode: no token needed: {first}"
    );

    // peer -> peer_token: the connection that opened in peer mode must now
    // present the token the file holds, which it never saw at open.
    daemon.result(
        "config.set",
        json!({"key":"ipc.auth_mode","value":"peer_token"}),
    );
    wait_until(Duration::from_secs(5), || {
        ping(&mut open, None)["error"]["message"] == json!("Invalid auth token")
    });
    let accepted = ping(&mut open, Some("first-token"));
    assert!(accepted["error"].is_null(), "{accepted}");
    assert_eq!(
        ping(&mut open, Some("second-token"))["error"]["message"],
        json!("Invalid auth token")
    );

    // A rotation: the file changes, the reload picks it up, and the same
    // connection accepts the new token and refuses the old one.
    write_token(&daemon.tree, "second-token");
    let mut editor = Conn::open(&daemon.tree.socket());
    let set = editor.send(&json!({"jsonrpc":"2.0","id":"2","method":"config.set","params":{"key":"daemon.log_level","value":"debug"},"auth_token":"first-token"}).to_string());
    assert!(set["error"].is_null(), "{set}");
    wait_until(Duration::from_secs(5), || {
        ping(&mut open, Some("second-token"))["error"].is_null()
    });
    assert_eq!(
        ping(&mut open, Some("first-token"))["error"]["message"],
        json!("Invalid auth token")
    );

    // peer_token -> peer: the token is no longer asked for.
    let set = editor.send(&json!({"jsonrpc":"2.0","id":"3","method":"config.set","params":{"key":"ipc.auth_mode","value":"peer"},"auth_token":"second-token"}).to_string());
    assert!(set["error"].is_null(), "{set}");
    wait_until(Duration::from_secs(5), || {
        ping(&mut open, None)["error"].is_null()
    });
    daemon.stop();
}

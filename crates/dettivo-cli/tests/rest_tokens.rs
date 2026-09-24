//! The resolved REST credential authenticates settings and backend calls.
mod common;

use common::*;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const TOKEN: &str = "rest-source-routing-test";

struct Serve(Child);
impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn resolved_sources_authenticate_settings_and_ipc_in_both_host_modes() {
    for source in [
        "file",
        "environment",
        "secret_service",
        "flag",
        "token_file_flag",
    ] {
        for daemon_hosted in [false, true] {
            let tree = Tree::new();
            tree.write_config(&format!("[ipc]\nauth_mode = \"peer_token\"\n[rest]\nenabled = {daemon_hosted}\nport = 0\nmax_body_bytes = 4\n[hotkeys]\nbackend = \"none\"\n"));
            let token_file = tree.root().join("cfg/dettivo/ipc.token");
            if source == "file" {
                std::fs::write(&token_file, TOKEN).unwrap();
                std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600))
                    .unwrap();
            }
            let bin = tree.root().join("bin");
            std::fs::create_dir(&bin).unwrap();
            let secret_tool = bin.join("secret-tool");
            std::fs::write(&secret_tool, if source == "secret_service" {
                format!("#!/bin/sh\n[ \"$*\" = \"lookup service dettivo key ipc-token\" ] || exit 1\nprintf '%s\\n' '{TOKEN}'\n")
            } else { "#!/bin/sh\nexit 1\n".into() }).unwrap();
            std::fs::set_permissions(&secret_tool, std::fs::Permissions::from_mode(0o700)).unwrap();
            let path = bin.to_string_lossy().into_owned();
            let mut env = vec![("PATH", path.as_str())];
            if matches!(source, "environment" | "flag" | "token_file_flag") {
                env.push(("DETTIVO_IPC_TOKEN", TOKEN));
            }
            let d = Daemon::spawn(tree, &env);
            if matches!(source, "flag" | "token_file_flag") {
                env.retain(|(key, _)| *key != "DETTIVO_IPC_TOKEN");
            }
            let mut flags = Vec::new();
            let explicit_file = d.tree.root().join("explicit.token");
            let explicit_path = explicit_file.to_string_lossy().into_owned();
            if source == "flag" {
                flags.extend(["--token", TOKEN]);
            }
            if source == "token_file_flag" {
                std::fs::write(&explicit_file, TOKEN).unwrap();
                flags.extend(["--token-file", explicit_path.as_str()]);
            }
            let mut serving = None;
            let addr = if daemon_hosted {
                let mut args = flags.clone();
                args.extend(["--json", "rest", "status"]);
                let output = cli_in(&d.tree, &args, &env);
                assert!(
                    output.status.success(),
                    "{source}: {} {}",
                    stdout(&output),
                    stderr(&output)
                );
                let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
                assert_eq!(report["host"], "daemon");
                format!("127.0.0.1:{}", report["port"])
            } else {
                let mut command = Command::new(CLI);
                d.tree.env(&mut command);
                command
                    .envs(env.iter().copied())
                    .args(&flags)
                    .args(["rest", "serve"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped());
                let mut child = Serve(command.spawn().unwrap());
                let lines = BufReader::new(child.0.stderr.take().unwrap()).lines();
                let addr = lines
                    .map_while(Result::ok)
                    .find_map(|line| {
                        line.strip_prefix("dettivo rest: listening on http://")
                            .map(str::to_string)
                    })
                    .expect("listener started");
                serving = Some(child);
                addr
            };
            let reply = dettivo_rest::client::request(
                addr.parse().unwrap(),
                "GET",
                "/v1/system/ping",
                &[("Authorization".into(), format!("Bearer {TOKEN}"))],
                b"",
                None,
                Duration::from_secs(3),
            )
            .unwrap();
            assert_eq!(
                reply.status, 200,
                "{source} daemon={daemon_hosted}: {reply:?}"
            );
            let oversized = dettivo_rest::client::request(
                addr.parse().unwrap(),
                "GET",
                "/v1/system/ping",
                &[("Authorization".into(), format!("Bearer {TOKEN}"))],
                b"12345",
                None,
                Duration::from_secs(3),
            )
            .unwrap();
            assert_eq!(
                oversized.status, 413,
                "settings must load max_body_bytes=4 through authenticated IPC"
            );
            drop(serving);
        }
    }
}

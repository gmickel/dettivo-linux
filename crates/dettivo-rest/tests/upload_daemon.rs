//! A committed multi-chunk upload reaches import validation in both REST hosts.
mod common;
use common::*;
use dettivo_rest::{Settings, Shim, SocketBackend};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn both_hosts_commit_uploads_with_default_and_reduced_daemon_line_limits() {
    for cap in [1 << 20, 4096] {
        let tree = rest_tree(None);
        let mut config = std::fs::read_to_string(tree.config_file()).unwrap();
        config.push_str(&format!("[ipc]\nmax_line_bytes = {cap}\n"));
        tree.write_config(&config);
        let daemon = Daemon::spawn(tree, &[("DETTIVO_IPC_TOKEN", TOKEN)]);
        let daemon_port = daemon.rest_caps()["port"].as_u64().unwrap() as u16;
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let process = runtime
            .block_on(dettivo_rest::server::start(Arc::new(Shim {
                backend: Arc::new(SocketBackend {
                    socket: daemon.tree.socket(),
                    token: None,
                    timeout: Duration::from_secs(5),
                }),
                token: TOKEN.into(),
                settings: Settings {
                    port: 0,
                    ..Settings::default()
                },
            })))
            .unwrap();
        let bytes = vec![43; (1 << 20) + 13];
        for port in [process.port(), daemon_port] {
            let reply = dettivo_rest::client::request(
                ([127, 0, 0, 1], port).into(),
                "POST",
                "/v1/transcripts/import/stream?mode=enhanced",
                &[
                    ("Authorization".into(), format!("Bearer {TOKEN}")),
                    ("Content-Type".into(), "audio/wav".into()),
                ],
                &bytes,
                None,
                Duration::from_secs(10),
            )
            .unwrap();
            let error = reply.json().unwrap();
            assert_eq!(reply.status, 501, "cap={cap}, port={port}: {error}");
            assert_eq!(error["error"]["data"]["app_code"], "NOT_IMPLEMENTED");
            assert!(
                error["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("an import runs the raw layer")
            );
        }
        runtime.block_on(process.stop());
    }
}

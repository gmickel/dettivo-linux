//! The harness: a client that drives a spawned server over its pipes in
//! one framing and runs the representative steps against a seeded daemon
//! (initialize, the lists, the read tools, an insertion through the mock
//! backend, an export, the resources, the guidance answers). The crate's
//! own test and `dettivo-qa mcp` run the same steps and report one row
//! per step and framing. The steps live in `harness_steps`.

use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::transport::{Framing, Reader, encode};

/// The contract's sample dictation, the newest row of the QA seed.
pub const SAMPLE_ID: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
/// The contract's sample meeting id, the seed's one meeting.
pub const MEETING_ID: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

/// The protocol version the harness names in `initialize`.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// A server process spoken to in one framing.
pub struct Session {
    child: Child,
    framing: Framing,
    reader: Reader<ChildStdout>,
    stdin: ChildStdin,
    next_id: i64,
    /// The audio file the `import_audio` step uploads; absent, the step
    /// reports `skip` (no test model on this machine).
    pub fixture: Option<std::path::PathBuf>,
    /// Items the steps created; removed through the daemon after the run.
    pub created: Vec<String>,
}

impl Session {
    /// Spawns `command` with piped stdio and speaks `framing` to it.
    pub fn spawn(mut command: Command, framing: Framing) -> std::io::Result<Self> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        Ok(Self {
            child,
            framing,
            reader: Reader::new(stdout, usize::MAX),
            stdin,
            next_id: 1,
            fixture: None,
            created: Vec::new(),
        })
    }

    /// The framing this session speaks.
    pub fn framing(&self) -> Framing {
        self.framing
    }

    /// The server's process id, for a caller that must end a hung server.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Writes raw bytes to the server's stdin.
    pub fn write_raw(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.stdin.write_all(bytes)?;
        self.stdin.flush()
    }

    /// Reads the next message the server wrote.
    pub fn read(&mut self) -> Result<Value, String> {
        match self.reader.read_message() {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err("the server closed its output".into()),
            Err(e) => Err(format!("unreadable server message: {e:?}")),
        }
    }

    /// Sends a notification.
    pub fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let message = json!({"jsonrpc": "2.0", "method": method, "params": params});
        self.write_raw(&encode(self.framing, &message))
            .map_err(|e| format!("write: {e}"))
    }

    /// Sends a request and returns the whole response.
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write_raw(&encode(self.framing, &message))
            .map_err(|e| format!("write: {e}"))?;
        let response = self.read()?;
        if response["id"] != json!(id) {
            return Err(format!("response id {} for request {id}", response["id"]));
        }
        Ok(response)
    }

    /// Sends a request and returns its `result`, or the error's message.
    pub fn result(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let response = self.request(method, params)?;
        if let Some(error) = response.get("error") {
            return Err(format!("{method}: {error}"));
        }
        Ok(response["result"].clone())
    }

    /// `tools/call` and its result.
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        self.result("tools/call", json!({"name": name, "arguments": arguments}))
    }

    /// The text of a tool result's first content block.
    pub fn text_of(result: &Value) -> String {
        result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One step's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The check held.
    Pass,
    /// The check failed; `detail` says how.
    Fail,
    /// The step needs something this machine lacks; `detail` names it.
    /// Never counted as passed.
    Skip,
}

/// One row of the harness report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// The step (a protocol method, a tool or a resource).
    pub step: String,
    /// `line-delimited` or `content-length`.
    pub framing: String,
    /// Pass or fail.
    pub verdict: Verdict,
    /// Why, for a failure.
    pub detail: Option<String>,
}

/// The server command for `server` against `socket`.
pub fn server_command(server: &Path, socket: &Path) -> Command {
    let mut command = Command::new(server);
    command.arg("--socket").arg(socket);
    command.env_remove("DETTIVO_IPC_SOCKET");
    command.env_remove("DETTIVO_IPC_TOKEN");
    command
}

pub use crate::harness_steps::{SKIP, steps};

/// Runs every step in `framing` against `server` on `socket`; `fixture`
/// is the audio file the `import_audio` step uploads.
pub fn run_with(
    server: &Path,
    socket: &Path,
    framing: Framing,
    fixture: Option<&Path>,
) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut session = match Session::spawn(server_command(server, socket), framing) {
        Ok(s) => s,
        Err(e) => {
            rows.push(Row {
                step: "spawn".into(),
                framing: framing.name().into(),
                verdict: Verdict::Fail,
                detail: Some(e.to_string()),
            });
            return rows;
        }
    };
    session.fixture = fixture.map(Path::to_path_buf);
    let created = run_steps(&mut session, framing, &mut rows);
    // The items a step created leave through the daemon so the seed's
    // sample stays the newest row for the next framing.
    let client = crate::client::Client {
        socket: socket.to_path_buf(),
        token: None,
        timeout: std::time::Duration::from_secs(10),
    };
    for id in created {
        let _ = client.call(
            "transcripts.delete",
            json!({"ref": {"kind": "dictation", "id": id}}),
        );
    }
    rows
}

fn run_steps(session: &mut Session, framing: Framing, rows: &mut Vec<Row>) -> Vec<String> {
    for (name, step) in steps() {
        let outcome = step(session);
        let verdict = match &outcome {
            Ok(()) => Verdict::Pass,
            Err(e) if e.starts_with(SKIP) => Verdict::Skip,
            Err(_) => Verdict::Fail,
        };
        rows.push(Row {
            step: name.to_string(),
            framing: framing.name().into(),
            verdict,
            detail: outcome
                .err()
                .map(|e| e.strip_prefix(SKIP).map(str::to_string).unwrap_or(e)),
        });
    }
    std::mem::take(&mut session.created)
}

/// Runs every step in both framings.
pub fn run_both(server: &Path, socket: &Path) -> Vec<Row> {
    run_both_with(server, socket, None)
}

/// `run_both` with the audio file the `import_audio` step uploads.
pub fn run_both_with(server: &Path, socket: &Path, fixture: Option<&Path>) -> Vec<Row> {
    let mut rows = run_with(server, socket, Framing::Line, fixture);
    rows.extend(run_with(server, socket, Framing::ContentLength, fixture));
    rows
}

/// Runs every step in `framing` against `server` on `socket`.
pub fn run(server: &Path, socket: &Path, framing: Framing) -> Vec<Row> {
    run_with(server, socket, framing, None)
}

/// The human report: one line per row, then the totals.
pub fn human(rows: &[Row]) -> String {
    let mut out = String::new();
    for r in rows {
        let tag = match r.verdict {
            Verdict::Pass => "pass",
            Verdict::Fail => "FAIL",
            Verdict::Skip => "skip",
        };
        out.push_str(&format!(
            "{tag}  {:<32} {:<15} {}\n",
            r.step,
            r.framing,
            r.detail.as_deref().unwrap_or("")
        ));
    }
    let failed = rows.iter().filter(|r| r.verdict == Verdict::Fail).count();
    let skipped = rows.iter().filter(|r| r.verdict == Verdict::Skip).count();
    out.push_str(&format!(
        "mcp: {} passed, {failed} failed, {skipped} skipped over {} framings\n",
        rows.len() - failed - skipped,
        rows.iter()
            .map(|r| r.framing.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_step_list_covers_the_representative_calls_and_renders() {
        let names: Vec<&str> = steps().iter().map(|(n, _)| *n).collect();
        for expected in [
            "initialize",
            "tools/list",
            "resources/list",
            "get_status",
            "list_transcripts",
            "search_transcripts",
            "get_latest_transcript",
            "insert_transcript",
            "export_transcript",
            "unknown_tool",
        ] {
            assert!(names.contains(&expected), "{expected}");
        }
        let rows = vec![Row {
            step: "ping".into(),
            framing: "line-delimited".into(),
            verdict: Verdict::Fail,
            detail: Some("x".into()),
        }];
        assert!(human(&rows).ends_with("mcp: 0 passed, 1 failed, 0 skipped over 1 framings\n"));
    }
}

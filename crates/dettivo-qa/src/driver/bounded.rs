//! A child process run under an absolute deadline: the rig's helpers
//! (`cua-driver call`) answer or are killed, so a hung tool can never
//! outlive the scenario timeout that wraps it.

use crate::child::ChildOwner;

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Runs `command` and returns its output when it exits within `budget`;
/// a process still running at the deadline is killed and the error is
/// `TimedOut`, naming the budget.
pub fn output_within(command: &mut Command, budget: Duration) -> std::io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map(ChildOwner::new)?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + budget;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("no answer within {budget:?}; the process was killed"),
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Reads a pipe to its end on a thread of its own, so a chatty child
/// never blocks on a full pipe while the parent waits for its exit.
fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut out);
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// qa-rig/F17: a hung helper is killed at the budget; a quick one
    /// answers with its output.
    #[test]
    fn a_hung_process_is_killed_at_the_budget_and_a_quick_one_answers() {
        let started = Instant::now();
        let err =
            output_within(Command::new("sleep").arg("30"), Duration::from_millis(150)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        let out = output_within(Command::new("echo").arg("hi"), Duration::from_secs(5)).unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
    }
}

//! The actionable texts a tool answers with when the daemon cannot serve
//! it. The macOS texts are kept word for word where Linux has the same
//! remedy; the unavailable text names the systemd unit instead of the
//! app, and a `NOT_IMPLEMENTED` answer says which spec brings the tool.

use dettivo_proto::error::AppCode;

use crate::client::ClientError;

/// The line under a daemon that is not running.
pub const ACTION_UNAVAILABLE: &str = "Action: Start the Dettivo daemon (systemctl --user start dettivod.socket) and retry. If needed, override socket with DETTIVO_IPC_SOCKET or --socket.";
/// The line under a refused token (the macOS text).
pub const ACTION_HARDENED: &str = "Action: If IPC hardened mode is enabled, provide DETTIVO_IPC_TOKEN (or --token/--token-file) and retry.";
/// The line under a timeout (the macOS text).
pub const ACTION_TIMEOUT: &str = "Action: Retry or increase timeout with --timeout-ms.";
/// The line under a `NOT_IMPLEMENTED` answer for the automation tools.
pub const ACTION_AUTOMATION_OFF: &str = "Action: automation jobs are off on this daemon (system.capabilities.automation.jobs = false); the tool answers once the automation spec lands. Check get_status before retrying.";
/// The line under a `NOT_IMPLEMENTED` answer for the meeting tools: the
/// capture-level methods answer; transcription of a meeting arrives with
/// the transcription spec.
pub const ACTION_MEETINGS_PENDING: &str = "Action: this daemon records meetings (start_meeting, list_meetings, get_meeting, search_meetings answer); what you asked for arrives with the meeting transcription spec. Check get_status for the capability flags.";

/// The tool error text for a failed daemon call.
pub fn actionable(tool: &str, error: &ClientError) -> String {
    let message = error.message();
    match error {
        ClientError::Unavailable(_) => format!("{message}\n{ACTION_UNAVAILABLE}"),
        ClientError::PermissionDenied(_) => format!("{message}\n{ACTION_HARDENED}"),
        ClientError::Timeout(_) => format!("{message}\n{ACTION_TIMEOUT}"),
        ClientError::Rpc(e) if e.app_code() == AppCode::NotImplemented => {
            format!("{message}\n{}", not_implemented_guidance(tool))
        }
        ClientError::Rpc(_) | ClientError::Protocol(_) => message,
    }
}

/// The guidance line for a tool the daemon has not implemented.
pub fn not_implemented_guidance(tool: &str) -> String {
    match tool {
        "create_automation_job" | "list_automation_jobs" => ACTION_AUTOMATION_OFF.to_string(),
        "start_meeting" | "list_meetings" | "get_meeting" | "search_meetings" => {
            ACTION_MEETINGS_PENDING.to_string()
        }
        other => format!(
            "Action: this daemon has not implemented what {other} needs yet; check system.capabilities (get_status) for the flag that enables it."
        ),
    }
}

/// The text for a tool name the server does not know: the macOS line
/// plus the tools it does know.
pub fn unknown_tool(name: &str, known: &[&str]) -> String {
    format!(
        "Unknown tool: {name}. Available tools: {}",
        known.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_proto::error::{ErrorDetails, JsonRpcError};

    #[test]
    fn each_failure_class_gets_its_action_line() {
        let down = actionable("get_status", &ClientError::Unavailable("gone".into()));
        assert_eq!(down, format!("gone\n{ACTION_UNAVAILABLE}"));
        let refused = actionable("get_status", &ClientError::PermissionDenied("no".into()));
        assert!(refused.ends_with(ACTION_HARDENED));
        let slow = actionable("get_status", &ClientError::Timeout("late".into()));
        assert!(slow.ends_with(ACTION_TIMEOUT));
        let pending = actionable(
            "list_automation_jobs",
            &ClientError::Rpc(JsonRpcError::not_implemented("automation.jobs.list")),
        );
        assert!(pending.starts_with("automation.jobs.list is not implemented (NOT_IMPLEMENTED)\n"));
        assert!(pending.ends_with(ACTION_AUTOMATION_OFF));
        let plain = actionable(
            "get_transcript",
            &ClientError::Rpc(JsonRpcError::new(
                AppCode::NotFound,
                "no such item",
                ErrorDetails::empty(),
            )),
        );
        assert_eq!(plain, "no such item (NOT_FOUND)");
        assert_eq!(
            unknown_tool("frob", &["a", "b"]),
            "Unknown tool: frob. Available tools: a, b"
        );
    }
}

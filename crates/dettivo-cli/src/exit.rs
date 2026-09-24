//! The contract's exit codes (IPC v1 §10.1) and the failure value every
//! command returns.

use std::process::ExitCode;

use dettivo_proto::error::{AppCode, JsonRpcError};

/// Exit codes, normative for every adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 0: success.
    Success,
    /// 1: generic or runtime failure.
    Failure,
    /// 2: the daemon or its socket is unavailable.
    Unavailable,
    /// 3: permission denied (peer or token).
    PermissionDenied,
    /// 4: invalid command or arguments.
    InvalidArgs,
    /// 5: timeout or interrupted operation.
    Timeout,
}

impl Exit {
    /// The process exit code.
    pub fn code(self) -> ExitCode {
        ExitCode::from(self.number())
    }

    /// The numeric process exit code, also carried by adapter errors.
    pub fn number(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::Unavailable => 2,
            Self::PermissionDenied => 3,
            Self::InvalidArgs => 4,
            Self::Timeout => 5,
        }
    }

    /// The exit code a daemon error maps to.
    pub fn for_rpc(error: &JsonRpcError) -> Self {
        match error.app_code() {
            AppCode::UnauthorizedClient => Self::PermissionDenied,
            AppCode::InvalidParams => Self::InvalidArgs,
            AppCode::AppNotRunning => Self::Unavailable,
            _ => Self::Failure,
        }
    }
}

/// Why a command failed: the exit code, a message for people, and the
/// daemon's error when there was one (for `--json`).
#[derive(Debug)]
pub struct Failure {
    /// The exit code.
    pub exit: Exit,
    /// One line for standard error.
    pub message: String,
    /// The daemon's error object, when the failure came from it.
    pub rpc: Option<JsonRpcError>,
    /// A structured report or protocol transport already owns stdout.
    pub reported: bool,
}

impl Failure {
    /// A failure with no daemon error behind it.
    pub fn new(exit: Exit, message: impl Into<String>) -> Self {
        Self {
            exit,
            message: message.into(),
            rpc: None,
            reported: false,
        }
    }

    /// A command whose report or protocol transport already owns stdout.
    pub fn reported(exit: Exit, message: impl Into<String>) -> Self {
        Self {
            reported: true,
            ..Self::new(exit, message)
        }
    }

    /// A failure the daemon reported.
    pub fn rpc(error: JsonRpcError) -> Self {
        Self {
            exit: Exit::for_rpc(&error),
            message: format!("{} ({})", error.message, error.app_code().as_str()),
            rpc: Some(error),
            reported: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_proto::error::ErrorDetails;

    #[test]
    fn rpc_codes_map_to_the_contract_exit_codes() {
        let err = |code| JsonRpcError::new(code, "x", ErrorDetails::empty());
        assert_eq!(
            Exit::for_rpc(&err(AppCode::UnauthorizedClient)),
            Exit::PermissionDenied
        );
        assert_eq!(
            Exit::for_rpc(&err(AppCode::InvalidParams)),
            Exit::InvalidArgs
        );
        assert_eq!(Exit::for_rpc(&err(AppCode::NotImplemented)), Exit::Failure);
        assert_eq!(
            Exit::for_rpc(&err(AppCode::AppNotRunning)),
            Exit::Unavailable
        );
    }
}

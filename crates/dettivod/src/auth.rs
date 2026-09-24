//! Peer authentication (R2): `SO_PEERCRED` on every connection, and in
//! `peer_token` mode the shared token on every request, resolved in the
//! macOS order with the Keychain replaced by the freedesktop Secret
//! Service: `DETTIVO_IPC_TOKEN`, then the Secret Service item, then a
//! `0600` token file.

use std::fs;
use std::path::Path;

use dettivo_proto::capabilities::IpcMode;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::config::Source;
use serde_json::Value;

/// Result of the connection-level check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Peer {
    /// Same uid as the daemon.
    SameUser,
    /// A different uid; refuse.
    OtherUser {
        /// The peer's uid, for the refusal log line.
        uid: u32,
    },
}

/// Compares the peer's uid with the daemon's.
pub fn check_peer(peer_uid: u32, own_uid: u32) -> Peer {
    if peer_uid == own_uid {
        Peer::SameUser
    } else {
        Peer::OtherUser { uid: peer_uid }
    }
}

/// The daemon's own uid, from `/proc/self/status` (no libc dependency).
pub fn own_uid() -> u32 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|u| u.parse().ok())
        })
        .unwrap_or(u32::MAX)
}

/// The error every refusal carries.
pub fn unauthorized(message: &str) -> JsonRpcError {
    JsonRpcError::new(AppCode::UnauthorizedClient, message, ErrorDetails::empty())
}

/// The shared token and where it came from, or `None` when no source has
/// one (`dettivo_core::token`). Only consulted in `peer_token` mode.
pub fn resolve_token(token_file: &Path) -> Option<(String, Source)> {
    dettivo_core::token::resolve(token_file).map(|(t, origin)| (t, origin.source()))
}

/// Pulls `auth_token` out of a raw request (top level or inside `params`),
/// the two places the macOS server accepts it, so the typed envelope
/// never sees it.
pub fn take_request_token(raw: &mut Value) -> Option<String> {
    let mut found = None;
    if let Some(obj) = raw.as_object_mut() {
        if let Some(Value::String(t)) = obj.remove("auth_token") {
            found = Some(t);
        }
        if let Some(params) = obj.get_mut("params").and_then(Value::as_object_mut) {
            if let Some(Value::String(t)) = params.remove("auth_token") {
                found.get_or_insert(t);
            }
        }
    }
    found
}

/// Checks a request's token against the configured mode.
pub fn check_request(
    mode: IpcMode,
    expected: Option<&str>,
    provided: Option<&str>,
) -> Result<(), JsonRpcError> {
    if mode == IpcMode::Peer {
        return Ok(());
    }
    let Some(expected) = expected.filter(|e| !e.is_empty()) else {
        return Err(unauthorized("IPC token not configured"));
    };
    match provided {
        Some(p) if dettivo_core::token::constant_time_eq(p.as_bytes(), expected.as_bytes()) => {
            Ok(())
        }
        _ => Err(unauthorized("Invalid auth token")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn peer_uid_must_match() {
        assert_eq!(check_peer(1000, 1000), Peer::SameUser);
        assert_eq!(check_peer(1001, 1000), Peer::OtherUser { uid: 1001 });
        assert_ne!(own_uid(), u32::MAX);
    }

    #[test]
    fn token_is_taken_from_top_level_or_params() {
        let mut raw = json!({"jsonrpc":"2.0","id":"1","method":"system.ping","params":{"auth_token":"p"},"auth_token":"t"});
        assert_eq!(take_request_token(&mut raw).as_deref(), Some("t"));
        assert_eq!(raw["params"], json!({}));
        assert!(raw.get("auth_token").is_none());
        let mut raw =
            json!({"jsonrpc":"2.0","id":"1","method":"system.ping","params":{"auth_token":"p"}});
        assert_eq!(take_request_token(&mut raw).as_deref(), Some("p"));
    }

    #[test]
    fn peer_mode_ignores_tokens_and_token_mode_requires_a_match() {
        assert!(check_request(IpcMode::Peer, None, None).is_ok());
        let err = check_request(IpcMode::PeerToken, None, Some("x")).unwrap_err();
        assert_eq!(err.app_code(), AppCode::UnauthorizedClient);
        assert!(check_request(IpcMode::PeerToken, Some("s"), Some("s")).is_ok());
        assert!(check_request(IpcMode::PeerToken, Some("s"), Some("t")).is_err());
        assert!(check_request(IpcMode::PeerToken, Some("s"), None).is_err());
    }
}

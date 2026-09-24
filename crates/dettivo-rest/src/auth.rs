//! The token on every request: `Authorization: Bearer <token>` or
//! `X-Dettivo-Token: <token>`, compared in constant time with the token
//! resolved in the macOS order (`DETTIVO_IPC_TOKEN`, the Secret Service
//! item, the `0600` token file). A missing or wrong token is `401` with
//! the contract's `UNAUTHORIZED_CLIENT` body and the macOS message.

use std::path::PathBuf;

use dettivo_core::token::{self, Origin};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};

pub use dettivo_core::token::SOURCES;

/// The token the request carried, from either header form.
pub fn header_token(headers: &[(String, String)]) -> Option<String> {
    let find = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.trim().to_string())
    };
    if let Some(auth) = find("authorization") {
        if auth
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer "))
        {
            return Some(auth[7..].trim().to_string());
        }
    }
    find("x-dettivo-token").filter(|t| !t.is_empty())
}

/// Checks the request's token against the expected one.
pub fn check(expected: &str, headers: &[(String, String)]) -> Result<(), JsonRpcError> {
    let Some(provided) = header_token(headers) else {
        return Err(unauthorized("Missing auth token"));
    };
    if token::constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(unauthorized("Invalid auth token"))
    }
}

/// The error every refusal carries.
pub fn unauthorized(message: &str) -> JsonRpcError {
    JsonRpcError::new(AppCode::UnauthorizedClient, message, ErrorDetails::empty())
}

/// The token file the third source reads: `[ipc] token_file` when the
/// configuration names one, else `$XDG_CONFIG_HOME/dettivo/ipc.token`.
pub fn token_file() -> PathBuf {
    let paths = dettivo_core::paths::Paths::resolve();
    let loaded = dettivo_core::config::Loaded::load(&paths.config_file, |k| std::env::var_os(k));
    loaded.token_file(&paths)
}

/// The shared token and its origin, resolved in the macOS order, or
/// `None` when no source has one.
pub fn resolve_token() -> Option<(String, Origin)> {
    token::resolve(&token_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn both_header_forms_are_read_and_checked() {
        assert_eq!(
            header_token(&h(&[("authorization", "Bearer abc ")])).as_deref(),
            Some("abc")
        );
        assert_eq!(
            header_token(&h(&[("authorization", "bearer abc")])).as_deref(),
            Some("abc")
        );
        assert_eq!(
            header_token(&h(&[("x-dettivo-token", " abc")])).as_deref(),
            Some("abc")
        );
        assert_eq!(header_token(&h(&[("authorization", "Basic abc")])), None);
        assert_eq!(header_token(&h(&[])), None);
        assert!(check("abc", &h(&[("x-dettivo-token", "abc")])).is_ok());
        let missing = check("abc", &h(&[])).unwrap_err();
        assert_eq!(missing.app_code(), AppCode::UnauthorizedClient);
        assert_eq!(missing.message, "Missing auth token");
        let wrong = check("abc", &h(&[("authorization", "Bearer abd")])).unwrap_err();
        assert_eq!(wrong.message, "Invalid auth token");
    }
}

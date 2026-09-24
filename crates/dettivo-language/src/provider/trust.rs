//! The endpoint trust gate (FR-M6), ported from the macOS
//! `PolishRemoteEndpointAllowlist` and `PolishPrivacyGate`: a loopback
//! endpoint is always allowed, and anything else must be confirmed once
//! and is then stored in `[llm] trusted_endpoints` by its canonical form.

use url::Url;

/// The hosts that mean this machine.
const LOOPBACK_HOSTS: &[&str] = &[
    "localhost",
    "127.0.0.1",
    "::1",
    "0.0.0.0",
    "0:0:0:0:0:0:0:1",
];

/// What the gate decided about an endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    /// A loopback endpoint, or one the user confirmed.
    Allowed,
    /// A remote endpoint nobody confirmed; the caller answers `CONFLICT`
    /// with `kind = endpointNotTrusted`.
    NeedsConfirmation {
        /// The canonical endpoint to confirm.
        endpoint: String,
    },
    /// The string is not a URL the gate can reason about.
    Invalid {
        /// Why.
        reason: String,
    },
}

/// The canonical form an endpoint is stored and compared as: the scheme
/// and host lowercased, a loopback host normalised to `localhost`,
/// credentials, query and fragment dropped and a trailing `/v1` or `/`
/// removed.
pub fn canonicalize(endpoint: &str) -> Result<String, String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err("the endpoint is empty".into());
    }
    let mut url = Url::parse(trimmed).map_err(|e| format!("{trimmed}: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!(
            "{trimmed}: only http and https endpoints are supported"
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| format!("{trimmed}: the endpoint names no host"))?
        .to_lowercase();
    let host = host.trim_matches(|c| c == '[' || c == ']').to_string();
    if LOOPBACK_HOSTS.contains(&host.as_str()) {
        url.set_host(Some("localhost"))
            .map_err(|e| format!("{trimmed}: {e}"))?;
    } else {
        url.set_host(Some(&host))
            .map_err(|e| format!("{trimmed}: {e}"))?;
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    let mut path = url.path().to_string();
    while path.ends_with('/') {
        path.pop();
    }
    if let Some(rest) = path.strip_suffix("/v1") {
        path = rest.to_string();
    }
    url.set_path(&path);
    let mut out = url.to_string();
    while out.ends_with('/') {
        out.pop();
    }
    Ok(out)
}

/// True when the endpoint answers on this machine.
pub fn is_loopback(endpoint: &str) -> bool {
    Url::parse(endpoint.trim()).ok().is_some_and(|url| {
        url.host_str().is_some_and(|h| {
            LOOPBACK_HOSTS.contains(&h.to_lowercase().trim_matches(|c| c == '[' || c == ']'))
        })
    })
}

/// The gate's verdict for `endpoint` against the stored allowlist.
pub fn check(endpoint: &str, trusted: &[String]) -> Trust {
    let canonical = match canonicalize(endpoint) {
        Ok(c) => c,
        Err(reason) => return Trust::Invalid { reason },
    };
    if is_loopback(&canonical) {
        return Trust::Allowed;
    }
    let allowed = trusted
        .iter()
        .filter_map(|t| canonicalize(t).ok())
        .any(|t| t == canonical);
    if allowed {
        Trust::Allowed
    } else {
        Trust::NeedsConfirmation {
            endpoint: canonical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_endpoints_are_canonical_and_always_allowed() {
        assert_eq!(
            canonicalize("http://127.0.0.1:11434/v1/").unwrap(),
            "http://localhost:11434"
        );
        assert_eq!(
            canonicalize("HTTP://LocalHost:11434").unwrap(),
            "http://localhost:11434"
        );
        assert_eq!(check("http://127.0.0.1:11434", &[]), Trust::Allowed);
        assert_eq!(check("http://[::1]:8080/v1", &[]), Trust::Allowed);
    }

    #[test]
    fn a_remote_endpoint_needs_confirmation_until_it_is_stored() {
        assert_eq!(
            check("https://llm.example.com/v1", &[]),
            Trust::NeedsConfirmation {
                endpoint: "https://llm.example.com".into()
            }
        );
        assert_eq!(
            check(
                "https://llm.example.com/v1",
                &["https://llm.example.com".to_string()]
            ),
            Trust::Allowed
        );
        // The stored form is compared canonically, so the same endpoint
        // written differently still matches.
        assert_eq!(
            check(
                "https://LLM.example.com/v1/",
                &["https://llm.example.com/v1".to_string()]
            ),
            Trust::Allowed
        );
        assert_eq!(
            canonicalize("https://user:secret@llm.example.com/v1?key=1#x").unwrap(),
            "https://llm.example.com"
        );
    }

    #[test]
    fn a_string_that_is_not_an_endpoint_is_named() {
        assert!(matches!(check("", &[]), Trust::Invalid { .. }));
        assert!(matches!(check("not a url", &[]), Trust::Invalid { .. }));
        assert!(matches!(
            check("ftp://example.com", &[]),
            Trust::Invalid { .. }
        ));
    }
}

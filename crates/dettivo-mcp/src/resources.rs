//! The resources: the five macOS templates (`status://current`,
//! `transcript://{id}`, `meeting://{id}`, `transcripts://search/{query}`,
//! `transcripts://latest/{kind}`) plus `speech://providers` from the
//! Windows port. `resources/list` names the current status, the providers
//! and the newest fifty history items; `resources/read` resolves a URI to
//! one daemon call.

use serde_json::{Value, json};

use crate::bounds::MAX_ITEMS;
use crate::client::Client;
use crate::tools::dispatch::{ToolError, transcript_get};

/// The URI of the speech providers resource (a Linux addition).
pub const SPEECH_PROVIDERS: &str = "speech://providers";

/// `resources/templates/list`.
pub fn templates() -> Vec<Value> {
    vec![
        json!({"uriTemplate": "status://current", "name": "Current Status", "description": "Current app/session status", "mimeType": "application/json"}),
        json!({"uriTemplate": "transcript://{id}", "name": "Transcript By ID", "description": "Fetch transcript by id (dictation first, then meeting fallback)", "mimeType": "application/json"}),
        json!({"uriTemplate": "meeting://{id}", "name": "Meeting By ID", "description": "Fetch meeting transcript + segments", "mimeType": "application/json"}),
        json!({"uriTemplate": "transcripts://search/{query}?kinds=dictation,meeting&limit=10", "name": "Search Transcripts", "description": "Search transcripts by text query", "mimeType": "application/json"}),
        json!({"uriTemplate": "transcripts://latest/{kind}", "name": "Latest Transcript", "description": "Fetch latest transcript reference for kind dictation|meeting|any", "mimeType": "application/json"}),
        json!({"uriTemplate": SPEECH_PROVIDERS, "name": "Speech Providers", "description": "Speech providers with their models and readiness (speech.providers.list)", "mimeType": "application/json"}),
    ]
}

/// `resources/list`: the status, the providers, then the newest items.
pub fn list(client: &Client) -> Result<Value, ToolError> {
    let mut resources = vec![
        json!({"uri": "status://current", "name": "Current Status", "description": "Current app/session state from system.health", "mimeType": "application/json"}),
        json!({"uri": SPEECH_PROVIDERS, "name": "Speech Providers", "description": "Speech providers, models and readiness", "mimeType": "application/json"}),
    ];
    let listed = client.call(
        "transcripts.list",
        json!({"kinds": ["dictation", "meeting"], "limit": MAX_ITEMS, "cursor": null}),
    )?;
    for item in listed["items"].as_array().into_iter().flatten() {
        let (Some(kind), Some(id)) = (item["ref"]["kind"].as_str(), item["ref"]["id"].as_str())
        else {
            continue;
        };
        let uri = if kind == "meeting" {
            format!("meeting://{id}")
        } else {
            format!("transcript://{id}")
        };
        let label = capitalised(kind);
        resources.push(json!({
            "uri": uri,
            "name": item["title"].as_str().map_or_else(|| format!("{label} {id}"), str::to_string),
            "description": format!("{label} transcript"),
            "mimeType": "application/json",
        }));
    }
    Ok(json!({"resources": resources}))
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The payload behind `uri`.
pub fn read(client: &Client, uri: &str) -> Result<Value, ToolError> {
    let (scheme, rest) = uri
        .split_once("://")
        .ok_or_else(|| ToolError::InvalidParams(format!("Invalid resource URI: {uri}")))?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (host, tail) = path.split_once('/').unwrap_or((path, ""));
    let tail = tail.trim_matches('/');
    match scheme {
        "status" => {
            if host != "current" {
                return Err(ToolError::InvalidParams(
                    "status resource supports status://current".into(),
                ));
            }
            let mut health = client.call("system.health", json!({}))?;
            if let Ok(caps) = client.call("system.capabilities", json!({})) {
                health["capabilities"] = caps;
            }
            Ok(health)
        }
        "transcript" => {
            let id = first_component(host, tail)
                .ok_or_else(|| ToolError::InvalidParams("transcript URI must include id".into()))?;
            transcript_get(client, &json!({"id": id}))
        }
        "meeting" => {
            let id = first_component(host, tail)
                .ok_or_else(|| ToolError::InvalidParams("meeting URI must include id".into()))?;
            Ok(client.call("meetings.get", json!({"meeting_id": id}))?)
        }
        "transcripts" => collection(client, host, tail, query),
        "speech" => {
            if host != "providers" {
                return Err(ToolError::InvalidParams(
                    "speech resource supports speech://providers".into(),
                ));
            }
            Ok(client.call("speech.providers.list", json!({}))?)
        }
        other => Err(ToolError::InvalidParams(format!(
            "Unsupported resource scheme: {other}"
        ))),
    }
}

/// The single id of a `scheme://{id}` template: the host, with nothing
/// after it. `scheme:///{id}` (an empty host and one tail segment) is the
/// spelling some clients produce and is accepted; an extra segment is not.
fn first_component<'a>(host: &'a str, tail: &'a str) -> Option<&'a str> {
    match (host.is_empty(), tail.is_empty()) {
        (false, true) => Some(host),
        (true, false) if !tail.contains('/') => Some(tail),
        _ => None,
    }
}

fn collection(client: &Client, host: &str, tail: &str, query: &str) -> Result<Value, ToolError> {
    match host {
        "search" => {
            let text = percent_decode(tail)?;
            if text.is_empty() {
                return Err(ToolError::InvalidParams(
                    "transcripts://search/<query> requires query".into(),
                ));
            }
            if text.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}') {
                return Err(ToolError::InvalidParams(
                    "transcripts://search query contains unsupported control characters".into(),
                ));
            }
            let mut limit = 10i64;
            let mut kinds = json!(["dictation", "meeting"]);
            for pair in query.split('&').filter(|p| !p.is_empty()) {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                let value = percent_decode(value)?;
                match name {
                    "limit" => {
                        if let Ok(n) = value.parse::<i64>() {
                            limit = n.clamp(1, MAX_ITEMS as i64);
                        }
                    }
                    "kinds" => {
                        kinds = json!(value.split(',').collect::<Vec<_>>());
                    }
                    _ => {}
                }
            }
            Ok(client.call(
                "transcripts.search",
                json!({"query": text, "kinds": kinds, "limit": limit}),
            )?)
        }
        "latest" => {
            let kind = if tail.is_empty() { "any" } else { tail };
            Ok(client.call("transcripts.latest", json!({"kind": kind}))?)
        }
        _ => Err(ToolError::InvalidParams(
            "Unsupported transcripts resource: transcripts://search/<query> or transcripts://latest/<kind>".into(),
        )),
    }
}

/// Decodes `%XX` escapes as UTF-8; `+` stays a plus as in a URI path.
/// Malformed escapes and invalid decoded UTF-8 are invalid parameters.
pub fn percent_decode(text: &str) -> Result<String, ToolError> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = |offset| bytes.get(offset).and_then(|b| (*b as char).to_digit(16));
            let (Some(high), Some(low)) = (hex(i + 1), hex(i + 2)) else {
                return Err(ToolError::InvalidParams(
                    "Malformed percent escape in resource URI".into(),
                ));
            };
            out.push((high * 16 + low) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out)
        .map_err(|_| ToolError::InvalidParams("Resource URI is not valid UTF-8".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_templates_including_the_providers() {
        let t = templates();
        assert_eq!(t.len(), 6);
        assert_eq!(t[5]["uriTemplate"], SPEECH_PROVIDERS);
        assert!(t.iter().all(|x| x["mimeType"] == "application/json"));
    }

    #[test]
    fn percent_decoding_and_bad_uris() {
        assert_eq!(percent_decode("api%20gateway").unwrap(), "api gateway");
        assert_eq!(percent_decode("caf%C3%A9").unwrap(), "café");
        assert_eq!(percent_decode("é%20+💬").unwrap(), "é +💬");
        assert!(percent_decode("100%").is_err());
        let client = Client {
            socket: std::path::PathBuf::from("/nonexistent.sock"),
            token: None,
            timeout: std::time::Duration::from_millis(50),
        };
        let err = read(&client, "bogus").unwrap_err();
        assert_eq!(err.text("resources/read"), "Invalid resource URI: bogus");
        let err = read(&client, "mail://x").unwrap_err();
        assert_eq!(
            err.text("resources/read"),
            "Unsupported resource scheme: mail"
        );
        let err = read(&client, "transcripts://search/").unwrap_err();
        assert!(err.text("resources/read").contains("requires query"));
        assert_eq!(capitalised("dictation"), "Dictation");
    }

    #[test]
    fn a_resource_id_is_one_component_and_nothing_more() {
        assert_eq!(first_component("abc", ""), Some("abc"));
        assert_eq!(first_component("", "abc"), Some("abc"));
        assert_eq!(first_component("abc", "extra"), None);
        assert_eq!(first_component("", "abc/extra"), None);
        assert_eq!(first_component("", ""), None);
    }
}

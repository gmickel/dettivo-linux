//! One request to one answer: the `/v1/` prefix, the loopback `Host`,
//! the token, then the two streaming routes or the generic
//! `/v1/{namespace}/{method}` mapping (segments joined by dots, query
//! parameters coerced the macOS way and overlaid by a JSON body). A path
//! outside the catalog is `404`; a reserved method is the daemon's
//! `NOT_IMPLEMENTED` (`501`); events are not served over REST.

use dettivo_proto::catalog;
use dettivo_proto::error::JsonRpcError;
use serde_json::{Map, Value};

use crate::http::{Request, Response};
use crate::server::Shim;
use crate::{auth, status, stream};

/// The route prefix every endpoint sits under.
pub const PREFIX: &str = "/v1/";

/// Answers one request.
pub fn handle(shim: &Shim, request: &Request) -> Response {
    let Some(rest) = request.path.strip_prefix(PREFIX) else {
        return status::error(&status::not_found("Unknown endpoint"));
    };
    if let Some(host) = request.header("host") {
        if !is_loopback_host(host) {
            return status::error(&auth::unauthorized("Invalid Host header"));
        }
    }
    if let Err(e) = auth::check(&shim.token, &request.headers) {
        return status::error(&e);
    }
    match rest {
        "transcripts/import/stream" => return stream::import(shim, request),
        "transcripts/export/stream" => return stream::export(shim, request),
        "events/subscribe" | "events/unsubscribe" => {
            return status::error(&JsonRpcError::not_implemented(
                "events over REST; subscribe over the Unix socket (`dettivo events --follow`) or MCP instead",
            ));
        }
        _ => {}
    }
    match call(shim, request, rest) {
        Ok(value) => status::ok(&value),
        Err(e) => status::error(&e),
    }
}

fn call(shim: &Shim, request: &Request, rest: &str) -> Result<Value, JsonRpcError> {
    if request.method != "GET" && request.method != "POST" {
        return Err(status::invalid(format!(
            "{} is not accepted; use GET or POST",
            request.method
        )));
    }
    let method = rest
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(".");
    if method.is_empty() {
        return Err(status::invalid("Missing method"));
    }
    if catalog::lookup(&method).is_none() {
        return Err(status::not_found(format!(
            "Unknown endpoint {PREFIX}{rest}"
        )));
    }
    let params = params(request)?;
    shim.backend.call(&method, Value::Object(params))
}

/// The query parameters coerced, overlaid by the JSON body object.
pub fn params(request: &Request) -> Result<Map<String, Value>, JsonRpcError> {
    let mut params = query_params(&request.query);
    if !request.body.is_empty() {
        let body: Value = serde_json::from_slice(&request.body)
            .map_err(|_| status::invalid("Body must be JSON object"))?;
        let Value::Object(object) = body else {
            return Err(status::invalid("Body must be JSON object"));
        };
        for (k, v) in object {
            params.insert(k, v);
        }
    }
    Ok(params)
}

/// The query string as params, each value coerced.
pub fn query_params(query: &str) -> Map<String, Value> {
    url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| (k.into_owned(), coerce(&v)))
        .collect()
}

/// The macOS coercion: `true`/`false`, an integer, a comma list of
/// strings, else the string.
pub fn coerce(value: &str) -> Value {
    match value.to_ascii_lowercase().as_str() {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(n) = value.parse::<i64>() {
        return Value::from(n);
    }
    if value.contains(',') {
        return Value::Array(
            value
                .split(',')
                .map(|s| Value::String(s.trim().to_string()))
                .collect(),
        );
    }
    Value::String(value.to_string())
}

/// `127.0.0.1`, `::1` or `localhost`, with an optional port and IPv6
/// brackets.
pub fn is_loopback_host(header: &str) -> bool {
    let trimmed = header.trim();
    let host = if let Some(rest) = trimmed.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        trimmed.split(':').next().unwrap_or("")
    };
    match host.parse::<std::net::IpAddr>() {
        Ok(addr) => addr.is_loopback(),
        Err(_) => host.eq_ignore_ascii_case("localhost"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn query_values_are_coerced_and_the_body_overlays() {
        assert_eq!(coerce("true"), json!(true));
        assert_eq!(coerce("False"), json!(false));
        assert_eq!(coerce("42"), json!(42));
        assert_eq!(coerce("-3"), json!(-3));
        assert_eq!(coerce("a, b,c"), json!(["a", "b", "c"]));
        assert_eq!(coerce("en"), json!("en"));
        assert_eq!(coerce("4.5"), json!("4.5"));
        let q = query_params("limit=5&kinds=dictation,meeting&q=hello%20world");
        assert_eq!(
            Value::Object(q),
            json!({"limit": 5, "kinds": ["dictation", "meeting"], "q": "hello world"})
        );
        let request = Request {
            method: "POST".into(),
            path: "/v1/transcripts/list".into(),
            query: "limit=5&offset=1".into(),
            headers: vec![],
            body: br#"{"limit": 10}"#.to_vec(),
            keep_alive: true,
        };
        assert_eq!(
            Value::Object(params(&request).unwrap()),
            json!({"limit": 10, "offset": 1})
        );
        let bad = Request {
            body: b"[1]".to_vec(),
            ..request
        };
        assert_eq!(
            params(&bad).unwrap_err().message,
            "Body must be JSON object"
        );
    }

    #[test]
    fn host_headers_are_loopback_only() {
        for ok in [
            "127.0.0.1",
            "127.0.0.1:45831",
            "[::1]:45831",
            "localhost",
            "LOCALHOST:1",
        ] {
            assert!(is_loopback_host(ok), "{ok}");
        }
        for bad in ["10.0.0.1", "example.com:80", "[fe80::1]", ""] {
            assert!(!is_loopback_host(bad), "{bad}");
        }
    }
}

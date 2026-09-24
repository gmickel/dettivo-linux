//! The MCP layer: `initialize`, `ping`, `tools/list`, `tools/call`,
//! `resources/list`, `resources/read` and `resources/templates/list`
//! over JSON-RPC 2.0, one message in, one message out, notifications
//! swallowed. A tool that fails answers an `isError` result with the
//! actionable text; only a malformed request is a JSON-RPC error.

use std::io::{self, Read, Write};

use serde_json::{Value, json};

use crate::bounds::{MAX_MESSAGE_BYTES, TRUNCATED, bounded};
use crate::client::Client;
use crate::tools::dispatch::{self, ToolError};
use crate::transport::{self, ReadError, Reader};
use crate::{DEFAULT_PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION, resources, tools};

/// JSON-RPC: the request is not a valid request object.
pub const INVALID_REQUEST: i64 = -32600;
/// JSON-RPC: the method does not exist.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC: the params are invalid.
pub const INVALID_PARAMS: i64 = -32602;
/// JSON-RPC: the server failed.
pub const INTERNAL_ERROR: i64 = -32603;
/// The contract's `RATE_LIMITED_LOCAL` code, answered for a message over
/// the byte cap (registered in `docs/api/linux-deltas.md`).
pub const RATE_LIMITED_LOCAL: i64 = -32016;

/// How the server runs.
#[derive(Debug, Clone)]
pub struct Config {
    /// The daemon connection.
    pub client: Client,
    /// The transport cap in bytes (`[mcp] max_message_bytes`).
    pub max_message_bytes: u64,
    /// Log every message on standard error (`DETTIVO_MCP_DEBUG=1`).
    pub debug: bool,
}

impl Config {
    /// A configuration with the contract's cap and no logging.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            max_message_bytes: MAX_MESSAGE_BYTES,
            debug: std::env::var("DETTIVO_MCP_DEBUG").is_ok_and(|v| v == "1"),
        }
    }

    /// The configuration with `[mcp] max_message_bytes` read through the
    /// daemon (`config.get`); the contract's cap when the daemon does not
    /// answer, so the server starts whether or not the daemon is up.
    pub fn from_daemon(client: Client) -> Self {
        let mut config = Self::new(client);
        if let Ok(entries) = config
            .client
            .call("config.get", json!({"key": "mcp.max_message_bytes"}))
        {
            if let Some(n) = entries["entries"][0]["value"].as_u64().filter(|n| *n > 0) {
                config.max_message_bytes = n;
            }
        }
        config
    }
}

/// The server state.
pub struct Server {
    config: Config,
    initialized: bool,
}

struct RpcError {
    code: i64,
    message: String,
}

impl Server {
    /// A server for `config`.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            initialized: false,
        }
    }

    /// Whether `initialize` or `notifications/initialized` arrived.
    pub fn initialized(&self) -> bool {
        self.initialized
    }

    /// Reads messages from `input` and answers on `output` until the
    /// input ends or the output is closed.
    pub fn run<R: Read, W: Write>(&mut self, input: R, mut output: W) -> io::Result<()> {
        let mut reader = Reader::new(input, self.config.max_message_bytes as usize);
        self.log(&format!(
            "MCP server ready (socket={})",
            self.config.client.socket.display()
        ));
        loop {
            let response = match reader.read_message() {
                Ok(Some(message)) => {
                    self.log(&format!("read {message}"));
                    self.handle(&message)
                }
                Ok(None) => {
                    self.log("MCP server stdin closed");
                    return Ok(());
                }
                Err(ReadError::TooLarge { max_bytes }) => Some(too_large(max_bytes)),
                Err(ReadError::Invalid(message)) => Some(error_response(
                    Value::Null,
                    RpcError {
                        code: INVALID_REQUEST,
                        message,
                    },
                )),
                Err(ReadError::Io(e)) => return Err(e),
            };
            if let Some(response) = response {
                let response = bounded_response(response, self.config.max_message_bytes)?;
                self.log(&format!("write {response}"));
                if let Err(e) = transport::write_message(&mut output, reader.framing(), &response) {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        return Ok(());
                    }
                    return Err(e);
                }
            }
        }
    }

    /// Handles one message: a response for a request, nothing for a
    /// notification.
    pub fn handle(&mut self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        if message["jsonrpc"] != "2.0" {
            return id.map(|id| {
                error_response(
                    id,
                    RpcError {
                        code: INVALID_REQUEST,
                        message: "jsonrpc must be 2.0".into(),
                    },
                )
            });
        }
        let Some(method) = message["method"].as_str() else {
            return id.map(|id| {
                error_response(
                    id,
                    RpcError {
                        code: INVALID_REQUEST,
                        message: "method is required".into(),
                    },
                )
            });
        };
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let Some(id) = id else {
            if method == "notifications/initialized" {
                self.initialized = true;
            }
            return None;
        };
        Some(match self.request(method, &params) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(e) => error_response(id, e),
        })
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        match method {
            "initialize" => {
                self.initialized = true;
                let version = params["protocolVersion"]
                    .as_str()
                    .unwrap_or(DEFAULT_PROTOCOL_VERSION);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {
                        "tools": {"listChanged": true},
                        "resources": {"subscribe": false, "listChanged": true},
                    },
                    "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => {
                let caps = self
                    .config
                    .client
                    .call("system.capabilities", json!({}))
                    .ok();
                let defs: Vec<Value> = tools::available(caps.as_ref())
                    .iter()
                    .map(tools::Tool::definition)
                    .collect();
                Ok(json!({"tools": defs}))
            }
            "tools/call" => {
                let name = params["name"]
                    .as_str()
                    .filter(|n| !n.is_empty())
                    .ok_or(RpcError {
                        code: INVALID_PARAMS,
                        message: "tools/call requires name".into(),
                    })?;
                // Absent or null arguments mean the defaults; arguments
                // of another type are a malformed call, never `{}`.
                let args = match params.get("arguments") {
                    None | Some(Value::Null) => json!({}),
                    Some(a) if a.is_object() => a.clone(),
                    Some(_) => {
                        return Err(RpcError {
                            code: INVALID_PARAMS,
                            message: "tools/call arguments must be an object".into(),
                        });
                    }
                };
                Ok(match dispatch::call(&self.config.client, name, &args) {
                    Ok(payload) => tool_success(&payload),
                    Err(e) => tool_error(&e.text(name)),
                })
            }
            "resources/list" => resources::list(&self.config.client).map_err(rpc_error),
            "resources/read" => {
                let uri = params["uri"]
                    .as_str()
                    .filter(|u| !u.is_empty())
                    .ok_or(RpcError {
                        code: INVALID_PARAMS,
                        message: "resources/read requires uri".into(),
                    })?;
                let payload = resources::read(&self.config.client, uri).map_err(rpc_error)?;
                let text = pretty(&bounded(&payload).value);
                Ok(
                    json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]}),
                )
            }
            "resources/templates/list" => Ok(json!({"resourceTemplates": resources::templates()})),
            other => Err(RpcError {
                code: METHOD_NOT_FOUND,
                message: format!("Unsupported MCP method: {other}"),
            }),
        }
    }

    fn log(&self, line: &str) {
        if self.config.debug {
            eprintln!("[dettivo-mcp] {line}");
        }
    }
}

fn rpc_error(e: ToolError) -> RpcError {
    match e {
        ToolError::InvalidParams(message) => RpcError {
            code: INVALID_PARAMS,
            message,
        },
        other => RpcError {
            code: INTERNAL_ERROR,
            message: other.text("resources/read"),
        },
    }
}

fn error_response(id: Value, e: RpcError) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": e.code, "message": e.message}})
}

/// The answer to a message over the byte cap: the cap named, the id
/// unknown, `RATE_LIMITED_LOCAL` semantics so a client may retry smaller.
pub fn too_large(max_bytes: usize) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": {
            "code": RATE_LIMITED_LOCAL,
            "message": format!("MCP message exceeds max_message_bytes ({max_bytes})"),
            "data": {"app_code": "RATE_LIMITED_LOCAL", "retryable": true, "details": {"max_message_bytes": max_bytes}},
        },
    })
}

fn bounded_response(response: Value, max_bytes: u64) -> io::Result<Value> {
    let fits = |value: &Value| value.to_string().len() as u64 <= max_bytes;
    if fits(&response) {
        return Ok(response);
    }
    let mut error = too_large(usize::try_from(max_bytes).unwrap_or(usize::MAX));
    error["id"] = response["id"].clone();
    if fits(&error) {
        return Ok(error);
    }
    let compact = json!({"jsonrpc":"2.0", "id":response["id"],
        "error":{"code":RATE_LIMITED_LOCAL, "message":"MCP response too large"}});
    if fits(&compact) {
        return Ok(compact);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "max_message_bytes cannot hold the error with its request ID",
    ))
}

/// Pretty JSON with sorted keys, the text form of every payload.
pub fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// The `tools/call` result for a payload: the bounded structure and its
/// text, with `_truncated: true` when a bound cut something.
pub fn tool_success(payload: &Value) -> Value {
    let b = bounded(payload);
    let mut result = json!({
        "content": [{"type": "text", "text": pretty(&b.value)}],
        "structuredContent": b.value,
    });
    if b.truncated {
        result[TRUNCATED] = json!(true);
    }
    result
}

/// The `tools/call` result for a failure.
pub fn tool_error(message: &str) -> Value {
    json!({"isError": true, "content": [{"type": "text", "text": message}]})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> Server {
        Server::new(Config {
            client: Client {
                socket: std::path::PathBuf::from("/nonexistent/dettivo.sock"),
                token: None,
                timeout: std::time::Duration::from_millis(50),
            },
            max_message_bytes: MAX_MESSAGE_BYTES,
            debug: false,
        })
    }

    /// agent-surfaces/F4: arguments of the wrong type are refused before
    /// any tool runs with defaults it was not given.
    #[test]
    fn arguments_of_the_wrong_type_are_refused_before_dispatch() {
        let mut s = server();
        for bad in [json!([]), json!("x"), json!(7)] {
            let r = s
                .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "cancel_session", "arguments": bad}}))
                .unwrap();
            assert_eq!(r["error"]["code"], INVALID_PARAMS, "{r}");
            assert_eq!(
                r["error"]["message"],
                "tools/call arguments must be an object"
            );
        }
    }

    #[test]
    fn initialize_echoes_the_version_and_names_the_server() {
        let mut s = server();
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}))
            .unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "dettivo-mcp");
        assert_eq!(r["result"]["serverInfo"]["version"], "1.0.0");
        assert!(s.initialized());
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": "p", "method": "initialize", "params": {}}))
            .unwrap();
        assert_eq!(r["result"]["protocolVersion"], DEFAULT_PROTOCOL_VERSION);
        assert_eq!(r["id"], "p");
    }

    #[test]
    fn notifications_are_silent_and_bad_envelopes_are_errors() {
        let mut s = server();
        assert!(
            s.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
                .is_none()
        );
        assert!(s.initialized());
        let r = s
            .handle(&json!({"jsonrpc": "1.0", "id": 1, "method": "ping"}))
            .unwrap();
        assert_eq!(r["error"]["code"], INVALID_REQUEST);
        let r = s.handle(&json!({"jsonrpc": "2.0", "id": 1})).unwrap();
        assert_eq!(r["error"]["message"], "method is required");
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "frob"}))
            .unwrap();
        assert_eq!(r["error"]["code"], METHOD_NOT_FOUND);
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}))
            .unwrap();
        assert_eq!(r["result"], json!({}));
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {}}))
            .unwrap();
        assert_eq!(r["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn a_tool_failure_is_an_is_error_result_not_a_protocol_error() {
        let mut s = server();
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "get_status"}}))
            .unwrap();
        assert_eq!(r["result"]["isError"], true);
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("/nonexistent/dettivo.sock"), "{text}");
        assert!(
            text.ends_with(crate::messages::ACTION_UNAVAILABLE),
            "{text}"
        );
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "nope", "arguments": {}}}))
            .unwrap();
        assert!(
            r["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("Unknown tool: nope. Available tools: get_status")
        );
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}))
            .unwrap();
        assert_eq!(r["result"]["tools"].as_array().unwrap().len(), 19);
    }

    #[test]
    fn oversized_and_truncated_shapes() {
        let r = too_large(64);
        assert_eq!(r["error"]["code"], RATE_LIMITED_LOCAL);
        assert!(r["error"]["message"].as_str().unwrap().contains("(64)"));
        let ok = tool_success(&json!({"items": (0..60).collect::<Vec<u32>>()}));
        assert_eq!(ok[TRUNCATED], true);
        assert_eq!(
            ok["structuredContent"]["items"].as_array().unwrap().len(),
            50
        );
        let small = tool_success(&json!({"ok": true}));
        assert!(small.get(TRUNCATED).is_none());
        assert_eq!(small["content"][0]["text"], "{\n  \"ok\": true\n}");
    }
}

//! The two streaming routes. `POST /v1/transcripts/import/stream` takes
//! the raw audio body (already bounded by `rest.max_body_bytes`) through
//! `transfer.begin`, `transfer.chunk` and `transfer.commit` into
//! `transcripts.import` and answers the import result; a failure cancels
//! the transfer with the macOS reason. `GET /v1/transcripts/export/stream`
//! begins a download, runs `transcripts.export`, pulls every chunk and
//! answers the bytes with the export's content type and an attachment
//! filename.

use dettivo_proto::error::JsonRpcError;
use dettivo_proto::transfer_io::{self, Error, Failure};
use serde_json::{Value, json};
use std::io::Cursor;

use crate::http::{Body, Request, Response};
use crate::routes;
use crate::server::Shim;
use crate::status;

/// The filename a content type implies when the request names none.
pub fn filename_for(content_type: &str) -> String {
    let ext = match content_type {
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mpeg" => "mp3",
        "audio/mp4" | "audio/m4a" => "m4a",
        "audio/aac" => "aac",
        "audio/x-caf" | "audio/caf" => "caf",
        "audio/aiff" => "aiff",
        "audio/flac" => "flac",
        "audio/ogg" => "ogg",
        "application/zip" => "zip",
        _ => "bin",
    };
    format!("audio.{ext}")
}

/// The media type without its parameters, lower-cased.
fn media_type(header: Option<&str>) -> String {
    header
        .unwrap_or("application/octet-stream")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn transfer_error(error: Error<JsonRpcError>) -> JsonRpcError {
    match error {
        Error::Remote(error) => error,
        Error::Local(error) => status::internal(error.to_string()),
    }
}

fn transfer_id_of(result: &Value) -> Result<String, JsonRpcError> {
    result["transfer_id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| status::internal("transfer.begin did not return transfer_id"))
}

/// `POST /v1/transcripts/import/stream`.
pub fn import(shim: &Shim, request: &Request) -> Response {
    match import_call(shim, request) {
        Ok(v) => status::ok(&v),
        Err(e) => status::error(&e),
    }
}

fn import_call(shim: &Shim, request: &Request) -> Result<Value, JsonRpcError> {
    if request.method != "POST" {
        return Err(status::invalid("Use POST for import stream"));
    }
    if request.body.is_empty() {
        return Err(status::invalid(
            "the request body is empty; send the audio bytes",
        ));
    }
    let content_type = media_type(request.header("content-type"));
    let mut params = routes::query_params(&request.query);
    if !params.contains_key("filename") {
        let name = request
            .header("x-dettivo-filename")
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| filename_for(&content_type));
        params.insert("filename".into(), Value::String(name));
    }
    params
        .entry("target_kind")
        .or_insert_with(|| Value::String("dictation".into()));
    params
        .entry("mode")
        .or_insert_with(|| Value::String("raw".into()));
    params
        .entry("language")
        .or_insert_with(|| Value::String("auto".into()));
    let begin = shim.backend.call(
        "transfer.begin",
        json!({"direction": "upload", "content_type": content_type, "size_hint": request.body.len()}),
    )?;
    let transfer_id = transfer_id_of(&begin)?;
    let call = |method: &str, params: Value| shim.backend.call(method, params);
    transfer_io::guarded(call, &transfer_id, || {
        let chunk_size = begin["chunk_max_bytes"]
            .as_u64()
            .ok_or_else(|| status::internal("transfer.begin did not return chunk_max_bytes"))
            .and_then(|raw| shim.backend.upload_chunk_bytes(&transfer_id, raw))
            .map_err(|e| Failure::remote("stream_chunk_failed", e))?;
        transfer_io::upload(
            call,
            &transfer_id,
            &mut Cursor::new(&request.body),
            chunk_size,
        )?;
        params.insert("transfer_id".into(), Value::String(transfer_id.clone()));
        call("transcripts.import", Value::Object(params))
            .map_err(|e| Failure::remote("import_failed", e))
    })
    .map_err(transfer_error)
}

/// `GET /v1/transcripts/export/stream`.
pub fn export(shim: &Shim, request: &Request) -> Response {
    match export_call(shim, request) {
        Ok(r) => r,
        Err(e) => status::error(&e),
    }
}

fn export_call(shim: &Shim, request: &Request) -> Result<Response, JsonRpcError> {
    if request.method != "GET" {
        return Err(status::invalid("Use GET for export stream"));
    }
    let query = routes::query_params(&request.query);
    let text = |k: &str| query.get(k).and_then(Value::as_str).map(str::to_string);
    let format = text("format").ok_or_else(|| status::invalid("format is required"))?;
    let kind = text("kind").unwrap_or_else(|| "dictation".into());
    let scope = text("scope");
    let id = text("id");
    if id.is_none() && !matches!(scope.as_deref(), Some("range") | Some("all")) {
        return Err(status::invalid(
            "id is required (or scope=range with from and to, or scope=all)",
        ));
    }
    let begin = shim.backend.call(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/octet-stream", "size_hint": 0}),
    )?;
    let transfer_id = transfer_id_of(&begin)?;
    let mut params = json!({"format": format, "transfer_id": transfer_id});
    if let Some(id) = id {
        params["ref"] = json!({"kind": kind, "id": id});
    }
    for key in ["scope", "from", "to"] {
        if let Some(v) = text(key) {
            params[key] = Value::String(v);
        }
    }
    let call = |method: &str, params: Value| shim.backend.call(method, params);
    transfer_io::guarded(call, &transfer_id, || {
        let exported =
            call("transcripts.export", params).map_err(|e| Failure::remote("export_failed", e))?;
        let content_type = exported["content_type"]
            .as_str()
            .unwrap_or("application/octet-stream")
            .to_string();
        let filename = exported["filename"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("transcript.{format}"));
        let mut file = tempfile::tempfile().map_err(|e| Failure::local("export_failed", e))?;
        let len = transfer_io::download(call, &transfer_id, &mut file)?;
        Ok(Response {
            status: 200,
            headers: vec![
                ("Content-Type".into(), content_type),
                (
                    "Content-Disposition".into(),
                    format!("attachment; filename=\"{}\"", filename.replace('"', "")),
                ),
            ],
            body: Body::File(file, len),
        })
    })
    .map_err(transfer_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_follow_the_content_type() {
        assert_eq!(filename_for("audio/wav"), "audio.wav");
        assert_eq!(filename_for("audio/x-wav"), "audio.wav");
        assert_eq!(filename_for("audio/mpeg"), "audio.mp3");
        assert_eq!(filename_for("application/zip"), "audio.zip");
        assert_eq!(filename_for("text/plain"), "audio.bin");
        assert_eq!(media_type(Some("Audio/WAV; rate=16000")), "audio/wav");
        assert_eq!(media_type(None), "application/octet-stream");
    }
}

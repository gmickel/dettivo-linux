//! The two tools that move bytes: `import_audio` pushes a local file up
//! through `transfer.*` and hands it to `transcripts.import`;
//! `export_transcript` binds a download, pulls it chunk by chunk and
//! either writes the file the agent named or returns a bounded preview.

use std::path::Path;

use base64::Engine as _;
use dettivo_proto::transfer_io::{self, Failure as TransferFailure};
use serde_json::{Value, json};

use crate::bounds::bounded_text;
use crate::client::Client;
use crate::tools::dispatch::{ToolError, str_arg};

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("m4a" | "mp4") => "audio/m4a",
        Some("aiff" | "aif") => "audio/aiff",
        Some("flac") => "audio/flac",
        Some("ogg" | "oga" | "opus") => "audio/ogg",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

fn expected_content_type(format: &str) -> &'static str {
    match format {
        "txt" | "srt" | "vtt" => "text/plain",
        "md" => "text/markdown",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// `import_audio`: upload the file, then `transcripts.import`.
pub fn import_audio(client: &Client, args: &Value) -> Result<Value, ToolError> {
    let file_path = str_arg(args, "file_path")
        .ok_or_else(|| ToolError::InvalidParams("file_path is required".into()))?;
    let path = Path::new(file_path);
    if !path.is_file() {
        return Err(ToolError::InvalidParams(format!(
            "File not found: {file_path}"
        )));
    }
    let mut reader = std::fs::File::open(path)
        .map_err(|e| ToolError::Runtime(format!("cannot read {file_path}: {e}")))?;
    let size = reader
        .metadata()
        .map_err(|e| ToolError::Runtime(e.to_string()))?
        .len();
    if size == 0 {
        return Err(ToolError::InvalidParams(format!(
            "File is empty: {file_path}"
        )));
    }
    let begun = client.call(
        "transfer.begin",
        json!({"direction":"upload", "content_type":content_type(path), "size_hint":size}),
    )?;
    let transfer = begun["transfer_id"].as_str().unwrap_or("").to_string();
    let call = |method: &str, params| client.call(method, params).map_err(ToolError::from);
    transfer_io::guarded(call, &transfer, || {
        let chunk_bytes = upload_chunk_bytes(client, &begun, &transfer)
            .map_err(|e| TransferFailure::remote("stream_chunk_failed", e))?;
        transfer_io::upload(call, &transfer, &mut reader, chunk_bytes)?;
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "import".into());
        call(
            "transcripts.import",
            json!({
                "transfer_id": transfer,
                "target_kind": str_arg(args, "target_kind").unwrap_or("dictation"),
                "filename": filename,
                "language": str_arg(args, "language").unwrap_or("en"),
                "mode": str_arg(args, "mode").unwrap_or("raw"),
            }),
        )
        .map_err(|e| TransferFailure::remote("import_failed", e))
    })
    .map_err(transfer_error)
}

fn upload_chunk_bytes(client: &Client, begin: &Value, transfer: &str) -> Result<usize, ToolError> {
    let config = client.call("config.get", json!({"key":"ipc.max_line_bytes"}))?;
    let size = begin["chunk_max_bytes"]
        .as_u64()
        .zip(config["entries"][0]["value"].as_u64())
        .and_then(|(raw, cap)| {
            dettivo_proto::upload::chunk_bytes(raw, cap, "1", transfer, client.token.as_deref())
        });
    size.ok_or_else(|| {
        ToolError::Runtime("upload limits cannot fit a transfer.chunk request".into())
    })
}

/// `export_transcript`: bind a download, export into it, pull it.
pub fn export_transcript(client: &Client, args: &Value) -> Result<Value, ToolError> {
    let reference = args.get("ref").filter(|r| r.is_object());
    let (kind, id) = match reference {
        Some(r) => (
            r["kind"].as_str().unwrap_or(""),
            r["id"].as_str().unwrap_or(""),
        ),
        None => ("", ""),
    };
    if kind.is_empty() || id.is_empty() {
        return Err(ToolError::InvalidParams(
            "ref.kind and ref.id are required".into(),
        ));
    }
    let format = str_arg(args, "format")
        .ok_or_else(|| ToolError::InvalidParams("format is required".into()))?;
    let begun = client.call(
        "transfer.begin",
        json!({"direction": "download", "content_type": expected_content_type(format), "size_hint": 0}),
    )?;
    let transfer = begun["transfer_id"].as_str().unwrap_or("").to_string();
    let call = |method: &str, params| client.call(method, params).map_err(ToolError::from);
    transfer_io::guarded(call, &transfer, || {
        call(
            "transcripts.export",
            json!({"ref":{"kind":kind,"id":id},"format":format,"transfer_id":transfer}),
        )
        .map_err(|e| TransferFailure::remote("export_failed", e))?;
        if let Some(out) = str_arg(args, "out_path") {
            transfer_io::download_file(call, &transfer, Path::new(out))?;
            return Ok(json!({"transfer_id":transfer,"out_path":out}));
        }
        let mut preview = Preview::default();
        let total = transfer_io::download(call, &transfer, &mut preview)?;
        let captured = preview.0.len() as u64;
        let bound = |text: &str| {
            if total > captured {
                let marker = "\n[truncated; use out_path for the complete export]";
                let prefix: String = text
                    .chars()
                    .take(crate::bounds::MAX_TEXT_CHARS - marker.len())
                    .collect();
                format!("{prefix}{marker}")
            } else {
                bounded_text(text)
            }
        };
        let text = match String::from_utf8(preview.0) {
            Ok(text) => bound(&text),
            Err(e) if total > captured && e.utf8_error().error_len().is_none() => {
                let valid = e.utf8_error().valid_up_to();
                bound(std::str::from_utf8(&e.as_bytes()[..valid]).expect("validated UTF-8 prefix"))
            }
            Err(e) => bound(&base64::engine::general_purpose::STANDARD.encode(e.into_bytes())),
        };
        Ok(json!({"transfer_id":transfer,"preview":text}))
    })
    .map_err(transfer_error)
}

#[derive(Default)]
struct Preview(Vec<u8>);
impl std::io::Write for Preview {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let take = bytes
            .len()
            .min((crate::bounds::MAX_TEXT_CHARS * 4).saturating_sub(self.0.len()));
        self.0.extend_from_slice(&bytes[..take]);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn transfer_error(error: transfer_io::Error<ToolError>) -> ToolError {
    match error {
        transfer_io::Error::Remote(e) => e,
        transfer_io::Error::Local(e) => ToolError::Runtime(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types_follow_the_extension_and_the_format() {
        assert_eq!(content_type(Path::new("a.WAV")), "audio/wav");
        assert_eq!(content_type(Path::new("a.m4a")), "audio/m4a");
        assert_eq!(content_type(Path::new("a.bin")), "application/octet-stream");
        assert_eq!(expected_content_type("md"), "text/markdown");
        assert_eq!(expected_content_type("json"), "application/json");
        assert_eq!(expected_content_type("srt"), "text/plain");
    }

    #[test]
    fn a_missing_file_and_a_missing_ref_are_refused_before_any_call() {
        let client = Client {
            socket: std::path::PathBuf::from("/nonexistent.sock"),
            token: None,
            timeout: std::time::Duration::from_millis(50),
        };
        let err = import_audio(&client, &json!({"file_path": "/nonexistent/a.wav"})).unwrap_err();
        assert_eq!(
            err.text("import_audio"),
            "File not found: /nonexistent/a.wav"
        );
        let err = export_transcript(&client, &json!({"format": "json"})).unwrap_err();
        assert_eq!(
            err.text("export_transcript"),
            "ref.kind and ref.id are required"
        );
    }
}

//! Shared upload sizing for the raw transfer bound and the encoded IPC line.
use crate::envelope::Request;
use serde_json::{Value, json};

/// Raw bytes that fit one chunk, reserving the largest sequence number and
/// the complete serialized request including an optional authentication token.
/// `None` means the limits cannot carry even one byte.
pub fn chunk_bytes(
    raw_limit: u64,
    line_limit: u64,
    request_id: &str,
    transfer_id: &str,
    token: Option<&str>,
) -> Option<usize> {
    let mut envelope = serde_json::to_value(Request::new(
        request_id,
        "transfer.chunk",
        json!({"transfer_id":transfer_id, "seq":u64::MAX, "data_b64":""}),
    ))
    .ok()?;
    if let Some(token) = token {
        envelope["auth_token"] = Value::String(token.into());
    }
    let available = line_limit.checked_sub(serde_json::to_vec(&envelope).ok()?.len() as u64)?;
    let raw = raw_limit.min(available / 4 * 3);
    usize::try_from(raw).ok().filter(|n| *n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizing_covers_escaped_tokens_sequences_and_tiny_limits() {
        let token = "字\"\\\n";
        let size = chunk_bytes(1000, 512, "rest", "字", Some(token)).unwrap();
        let envelope = json!({"jsonrpc":"2.0", "id":"rest", "method":"transfer.chunk",
            "auth_token":token, "params":{"transfer_id":"字", "seq":u64::MAX,
            "data_b64":"A".repeat(size.div_ceil(3) * 4)}});
        assert!(envelope.to_string().len() <= 512);
        assert!(envelope.to_string().len() + 4 > 512);
        assert_eq!(chunk_bytes(17, 512, "1", "transfer", None), Some(17));
        assert_eq!(chunk_bytes(0, 512, "1", "transfer", None), None);
        assert_eq!(chunk_bytes(1000, 1, "1", "transfer", None), None);
    }
}

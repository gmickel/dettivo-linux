//! R3 and R5 without a model: the stateful `transfer.*` fixtures over one
//! connection, the seed exported through a download transfer against the
//! goldens, exports by item, range and everything, the spool staying
//! inside the test tree, and an archive that restores into an empty
//! profile with the same ids.

mod common;

use std::path::Path;

use base64::Engine as _;
use common::{Daemon, Tree};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SAMPLE: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const SEED_01: &str = "5eed0000-0000-4000-8000-000000000001";

fn fixture(rel: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-proto/fixtures")
        .join(rel);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn golden(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dettivo-storage/tests/goldens")
            .join(name),
    )
    .unwrap()
}

fn seeded() -> Daemon {
    Daemon::spawn(
        Tree::new(),
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_E2E_SEED", "1"),
            ("DETTIVO_MOCK_INSERT", "1"),
        ],
    )
}

/// Pulls a bound download to its end.
fn pull_all(daemon: &Daemon, transfer_id: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut seq = 1;
    loop {
        let chunk = daemon.result(
            "transfer.pull",
            json!({"transfer_id": transfer_id, "seq": seq}),
        );
        out.extend(
            base64::engine::general_purpose::STANDARD
                .decode(chunk["data_b64"].as_str().unwrap())
                .unwrap(),
        );
        if chunk["eof"] == true {
            return out;
        }
        seq += 1;
    }
}

/// Uploads bytes in chunks and commits; returns the transfer id.
fn upload(daemon: &Daemon, content_type: &str, bytes: &[u8]) -> String {
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "upload", "content_type": content_type, "size_hint": bytes.len()}),
    );
    let id = begun["transfer_id"].as_str().unwrap().to_string();
    let size = 256 * 1024;
    let mut total = 0;
    for (i, chunk) in bytes.chunks(size).enumerate() {
        let r = daemon.result(
            "transfer.chunk",
            json!({"transfer_id": id, "seq": i + 1, "data_b64": base64::engine::general_purpose::STANDARD.encode(chunk)}),
        );
        assert_eq!(r["accepted"], true);
        total += 1;
    }
    let digest = format!("{:x}", Sha256::digest(bytes));
    let committed = daemon.result(
        "transfer.commit",
        json!({"transfer_id": id, "total_chunks": total, "sha256": digest}),
    );
    assert_eq!(committed["committed"], true);
    id
}

#[test]
fn the_stateful_transfer_fixtures_pass_over_one_upload_and_one_download() {
    let daemon = seeded();
    // transfer/begin.json, then chunk.json, chunk.error-rate-limited.json,
    // commit.json and cancel.json against the transfer it opened.
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "upload", "content_type": "audio/m4a", "size_hint": 4200000}),
    );
    let id = begun["transfer_id"].as_str().unwrap().to_string();
    assert_eq!(begun["chunk_max_bytes"], 1_048_576);
    let chunk = daemon.result(
        "transfer.chunk",
        json!({"transfer_id": id, "seq": 1, "data_b64": "AAAA"}),
    );
    assert_eq!(chunk, fixture("transfer/chunk.json")["response"]["result"]);
    let limited = daemon.request(
        "transfer.chunk",
        json!({"transfer_id": id, "seq": 9, "data_b64": "AAAA"}),
    );
    assert_eq!(
        limited["error"],
        fixture("transfer/chunk.error-rate-limited.json")["error"]["error"]
    );
    let out_of_order = daemon.request(
        "transfer.chunk",
        json!({"transfer_id": id, "seq": 3, "data_b64": "AAAA"}),
    );
    assert_eq!(out_of_order["error"]["data"]["app_code"], "INVALID_PARAMS");
    let wrong_hash = daemon.request(
        "transfer.commit",
        json!({"transfer_id": id, "total_chunks": 1, "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}),
    );
    assert_eq!(wrong_hash["error"]["data"]["app_code"], "INVALID_PARAMS");
    let digest = format!("{:x}", Sha256::digest([0u8; 3]));
    let committed = daemon.result(
        "transfer.commit",
        json!({"transfer_id": id, "total_chunks": 1, "sha256": digest}),
    );
    assert_eq!(
        committed,
        fixture("transfer/commit.json")["response"]["result"]
    );
    let cancelled = daemon.result(
        "transfer.cancel",
        json!({"transfer_id": id, "reason": "client_abort"}),
    );
    assert_eq!(
        cancelled,
        fixture("transfer/cancel.json")["response"]["result"]
    );
    let gone = daemon.request(
        "transfer.cancel",
        json!({"transfer_id": id, "reason": "again"}),
    );
    assert_eq!(
        gone["error"],
        fixture("transfer/chunk.error-not-found.json")["error"]["error"]
    );
    let unsupported = daemon.request(
        "transfer.begin",
        json!({"direction": "upload", "content_type": "text/csv", "size_hint": 1}),
    );
    assert_eq!(unsupported["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert!(
        unsupported["error"]["message"]
            .as_str()
            .unwrap()
            .contains("audio/flac")
    );

    // transfer/pull.json against an export of the seed, checked against
    // the golden files (R5).
    for (format, name) in [
        ("json", "seed.json"),
        ("md", "seed.md"),
        ("txt", "seed.txt"),
    ] {
        let download = daemon.result(
            "transfer.begin",
            json!({"direction": "download", "content_type": "application/octet-stream", "size_hint": 0}),
        );
        let out = download["transfer_id"].as_str().unwrap().to_string();
        let early = daemon.request("transfer.pull", json!({"transfer_id": out, "seq": 1}));
        assert_eq!(
            early["error"]["data"]["app_code"], "CONFLICT",
            "nothing bound yet"
        );
        let exported = daemon.result(
            "transcripts.export",
            json!({"format": format, "transfer_id": out, "scope": "all"}),
        );
        assert_eq!(exported["transfer_id"], out);
        assert_eq!(exported["filename"], format!("dictations.{format}"));
        let first = daemon.result("transfer.pull", json!({"transfer_id": out, "seq": 1}));
        assert_eq!(first["seq"], 1);
        assert!(first["data_b64"].is_string() && first["eof"].is_boolean());
        let bytes = pull_all(&daemon, &out);
        assert_eq!(String::from_utf8(bytes).unwrap(), golden(name), "{format}");
        let ack = daemon.result(
            "transfer.commit",
            json!({"transfer_id": out, "total_chunks": 1, "sha256": ""}),
        );
        assert_eq!(ack["committed"], true);
    }
    daemon.stop();
}

#[test]
fn exports_cover_one_item_a_range_or_everything_and_refuse_bad_scopes() {
    let daemon = seeded();
    let begin = |ct: &str| {
        daemon.result(
            "transfer.begin",
            json!({"direction": "download", "content_type": ct, "size_hint": 0}),
        )["transfer_id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let out = begin("application/json");
    let one = daemon.result(
        "transcripts.export",
        json!({"format": "json", "transfer_id": out, "ref": {"kind": "dictation", "id": SAMPLE}}),
    );
    assert_eq!(one["content_type"], "application/json");
    assert_eq!(one["filename"], "dictation-7c9e6679.json");
    let doc: Value = serde_json::from_slice(&pull_all(&daemon, &out)).unwrap();
    assert_eq!(doc["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        doc["items"][0]["final_text"],
        "The contract sample dictation. Thirty seconds of speech."
    );

    let out = begin("text/plain");
    daemon.result(
        "transcripts.export",
        json!({"format": "txt", "transfer_id": out, "scope": "range", "from": "2026-02-12", "to": "2026-02-13"}),
    );
    let text = String::from_utf8(pull_all(&daemon, &out)).unwrap();
    assert_eq!(text.matches("2026-02-12T").count(), 4, "{text}");

    let out = begin("application/json");
    daemon.result(
        "transcripts.export",
        json!({"format": "json", "transfer_id": out, "scope": "range", "from": "2020-01-01", "to": "2020-01-02"}),
    );
    assert_eq!(
        String::from_utf8(pull_all(&daemon, &out)).unwrap(),
        "{\n  \"items\": []\n}\n"
    );

    let out = begin("application/json");
    let missing = daemon.request(
        "transcripts.export",
        json!({"format": "json", "transfer_id": out, "ref": {"kind": "dictation", "id": "00000000-0000-4000-8000-000000000000"}}),
    );
    assert_eq!(missing["error"]["data"]["app_code"], "NOT_FOUND");
    let no_ref = daemon.request(
        "transcripts.export",
        json!({"format": "json", "transfer_id": out}),
    );
    assert_eq!(no_ref["error"]["data"]["app_code"], "INVALID_PARAMS");
    let bad_range = daemon.request(
        "transcripts.export",
        json!({"format": "json", "transfer_id": out, "scope": "range", "from": "yesterday", "to": "2026-02-13"}),
    );
    assert_eq!(bad_range["error"]["data"]["app_code"], "INVALID_PARAMS");
    let no_transfer = daemon.request(
        "transcripts.export",
        json!({"format": "json", "transfer_id": "xfer_out_99", "scope": "all"}),
    );
    assert_eq!(no_transfer["error"]["data"]["app_code"], "NOT_FOUND");
    daemon.stop();
}

/// Regression: a test daemon that inherited the developer's
/// `XDG_CACHE_HOME` spooled its exports under `~/.cache/dettivo/exports`,
/// where every daemon the parallel tests spawn names its first download
/// `xfer_out_1`; one daemon's `bind_export` then overwrote the bytes
/// another was about to pull. The spool must stay inside the tree.
#[test]
fn exports_spool_inside_the_test_tree_not_the_inherited_cache_home() {
    let daemon = seeded();
    let out = daemon.result(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/json", "size_hint": 0}),
    )["transfer_id"]
        .as_str()
        .unwrap()
        .to_string();
    daemon.result(
        "transcripts.export",
        json!({"format": "json", "transfer_id": out, "scope": "all"}),
    );
    let spooled = daemon.tree.root().join("cache/dettivo/exports").join(&out);
    assert!(
        spooled.is_file(),
        "export {out} is not spooled under the tree at {}",
        spooled.display()
    );
    let bytes = pull_all(&daemon, &out);
    assert_eq!(std::fs::read(&spooled).unwrap(), bytes);
    daemon.stop();
}

#[test]
fn an_archive_export_imports_into_an_empty_profile_with_the_same_ids() {
    let source = seeded();
    let out = source.result(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/zip", "size_hint": 0}),
    )["transfer_id"]
        .as_str()
        .unwrap()
        .to_string();
    let exported = source.result(
        "transcripts.export",
        json!({"format": "zip", "transfer_id": out, "scope": "all"}),
    );
    assert_eq!(exported["content_type"], "application/zip");
    assert_eq!(exported["filename"], "dictations.zip");
    let archive = pull_all(&source, &out);
    assert!(archive.starts_with(b"PK"));
    source.stop();

    let target = Daemon::spawn(Tree::new(), &[("DETTIVO_QA_MODE", "1")]);
    let none = target.request("transcripts.latest", json!({"kind": "any"}));
    assert_eq!(none["error"]["data"]["app_code"], "NOT_FOUND");
    let id = upload(&target, "application/zip", &archive);
    let imported = target.result(
        "transcripts.import",
        json!({"transfer_id": id, "target_kind": "dictation", "filename": "dictations.zip", "language": "en", "mode": "raw"}),
    );
    assert_eq!(imported["job"]["state"], "succeeded");
    assert_eq!(imported["is_partial"], false);
    assert_eq!(imported["ref"]["id"], SAMPLE, "the newest restored item");
    let listed = target.result(
        "transcripts.list",
        json!({"kinds": ["dictation"], "limit": 20, "cursor": null}),
    );
    assert_eq!(listed["items"].as_array().unwrap().len(), 12);
    let got = target.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": SEED_01}}),
    );
    assert!(got["text_polish"].as_str().unwrap().contains("API gateway"));
    let again = target.request(
        "transcripts.import",
        json!({"transfer_id": id, "target_kind": "dictation", "filename": "dictations.zip", "language": "en", "mode": "raw"}),
    );
    assert_eq!(
        again["error"]["data"]["app_code"], "NOT_FOUND",
        "the upload was consumed"
    );
    target.stop();
}

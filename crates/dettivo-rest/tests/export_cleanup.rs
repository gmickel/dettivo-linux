//! Export failures preserve their error response and cancel the transfer.
use dettivo_proto::error::JsonRpcError;
use dettivo_rest::{Settings, Shim, backend::Backend, http::Request, status, stream};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

struct Export {
    malformed: bool,
    cancelled: Mutex<Vec<Value>>,
}
impl Backend for Export {
    fn call(&self, method: &str, params: Value) -> Result<Value, JsonRpcError> {
        match method {
            "transfer.begin" => Ok(json!({"transfer_id":"export"})),
            "transcripts.export" => Ok(json!({"content_type":"text/plain","filename":"out.txt"})),
            "transfer.pull" => {
                Ok(json!({"data_b64":if self.malformed { "%%%" } else { "b2s=" },"eof":true}))
            }
            "transfer.commit" => Err(status::internal("final acknowledgment failed")),
            "transfer.cancel" => {
                self.cancelled.lock().unwrap().push(params);
                Ok(json!({}))
            }
            _ => panic!("unexpected {method}"),
        }
    }
}
#[test]
fn export_failures_cancel_and_never_report_success() {
    for malformed in [false, true] {
        let backend = Arc::new(Export {
            malformed,
            cancelled: Mutex::new(Vec::new()),
        });
        let shim = Shim {
            backend: backend.clone(),
            token: "test".into(),
            settings: Settings::default(),
        };
        let response = stream::export(
            &shim,
            &Request {
                method: "GET".into(),
                path: "/v1/transcripts/export/stream".into(),
                query: "scope=all&format=txt".into(),
                headers: vec![],
                body: vec![],
                keep_alive: false,
            },
        );
        assert_eq!(response.status, 500);
        assert_eq!(backend.cancelled.lock().unwrap().len(), 1);
    }
}

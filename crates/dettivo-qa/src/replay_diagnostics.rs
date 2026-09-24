//! Validate diagnostic observations against the direct methods on this daemon.

use serde_json::{Value, json};

pub(crate) fn agrees(
    expected: &Value,
    actual: &Value,
    mut read: impl FnMut(&str) -> Option<Value>,
) -> bool {
    use dettivo_proto::methods::system::{DiagnosticsResult, ProbeState};
    let (mut want_envelope, mut got_envelope) = (expected.clone(), actual.clone());
    let Some(want_object) = want_envelope.as_object_mut() else {
        return false;
    };
    let Some(got_object) = got_envelope.as_object_mut() else {
        return false;
    };
    let want = want_object.remove("result");
    let got = got_object.remove("result");
    if want_envelope != got_envelope {
        return false;
    }
    let (Some(want), Some(got)) = (want, got) else {
        return false;
    };
    let Ok(want) = serde_json::from_value::<DiagnosticsResult>(want) else {
        return false;
    };
    let Ok(got) = serde_json::from_value::<DiagnosticsResult>(got) else {
        return false;
    };
    if want.probes.len() != 13 || want.probes.keys().ne(got.probes.keys()) {
        return false;
    }
    got.probes.iter().all(|(method, probe)| {
        let observed = match probe.state {
            ProbeState::Success => {
                let Some(result) = &probe.result else {
                    return false;
                };
                if probe.error.is_some()
                    || dettivo_proto::catalog::round_trip_result(method, result).is_err()
                {
                    return false;
                }
                json!({"jsonrpc":"2.0", "id":"probe", "result":result})
            }
            ProbeState::Failure => {
                let Some(error) = &probe.error else {
                    return false;
                };
                if probe.result.is_some() {
                    return false;
                }
                json!({"jsonrpc":"2.0", "id":"probe", "error":error})
            }
            ProbeState::Unknown => return probe.result.is_none() && probe.error.is_none(),
        };
        let Some(direct) = read(method) else {
            return false;
        };
        crate::replay::replies_agree(
            method,
            &crate::replay::normalise(method, observed),
            &crate::replay::normalise(method, direct),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(path: &str) -> Value {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dettivo-proto/fixtures")
            .join(path);
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
    }

    #[test]
    fn diagnostics_compares_current_probe_results_not_fixture_machine() {
        let want = fixture("system/diagnostics.json")["response"].clone();
        let mut actual = want.clone();
        actual["result"]["probes"]["llm.engine.status"] = json!({"state":"success", "result":fixture("llm/engine.status.json")["response"]["result"], "error":null});
        let read = |method: &str| {
            let probe = &actual["result"]["probes"][method];
            Some(if probe["state"] == "success" {
                json!({"jsonrpc":"2.0","id":"probe", "result":probe["result"]})
            } else {
                json!({"jsonrpc":"2.0","id":"probe", "error":probe["error"]})
            })
        };
        assert!(agrees(&want, &actual, read));
    }
    #[test]
    fn diagnostics_rejects_missing_probes_invalid_states_and_lost_errors() {
        let want = fixture("system/diagnostics.json")["response"].clone();
        let read = |method: &str| {
            let probe = &want["result"]["probes"][method];
            Some(if probe["state"] == "success" {
                json!({"jsonrpc":"2.0","id":"probe", "result":probe["result"]})
            } else {
                json!({"jsonrpc":"2.0","id":"probe", "error":probe["error"]})
            })
        };
        assert!(agrees(&want, &want, read));
        for pointer in [
            "/result/probes/system.health/state",
            "/result/probes/system.health/result/ok",
            "/result/probes/llm.engine.status/error",
        ] {
            let mut bad = want.clone();
            *bad.pointer_mut(pointer).unwrap() = Value::Null;
            assert!(!agrees(&want, &bad, read), "{pointer}");
        }
        let mut bad = want.clone();
        bad["result"]["probes"]
            .as_object_mut()
            .unwrap()
            .remove("system.health");
        assert!(!agrees(&want, &bad, read));
        let mut bad = want.clone();
        bad["result"]["probes"]["llm.engine.status"]["error"]["message"] = json!("lost failure");
        assert!(!agrees(&want, &bad, read));
    }
}

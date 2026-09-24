//! How the daemon's capabilities relate to what a fixture requires
//! (fn-41 R1): a flag that holds is a promise, a required `true` flag the
//! daemon declares `false` admits a pending answer, and any other value
//! skips the fixture.

use serde_json::Value;

/// How the daemon's capabilities relate to what a fixture requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flags {
    /// Every required flag holds; `promised` names the boolean flags
    /// declared `true`, the ones a pending answer would break.
    Declared {
        /// The boolean flags declared `true` that the fixture requires.
        promised: Vec<String>,
    },
    /// A required `true` flag the daemon declares `false`: the fixture is
    /// still sent and a pending answer is admitted by that flag.
    Admitted(String),
    /// A required flag holds another value; `detail` says which.
    Missing(String),
}

/// Reads a fixture's `capabilities` block against the daemon's snapshot.
pub fn flags(fixture: &Value, caps: &Value) -> Flags {
    let mut promised = Vec::new();
    let Some(required) = fixture["capabilities"].as_object() else {
        return Flags::Declared { promised };
    };
    for (path, expected) in required {
        let mut cursor = caps;
        for segment in path.split('.') {
            cursor = &cursor[segment];
        }
        if cursor == expected {
            if expected == &Value::Bool(true) {
                promised.push(path.clone());
            }
            continue;
        }
        if expected == &Value::Bool(true) && cursor == &Value::Bool(false) {
            return Flags::Admitted(path.clone());
        }
        return Flags::Missing(format!(
            "{path} = {expected} not declared (daemon has {cursor})"
        ));
    }
    Flags::Declared { promised }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::{Report, Row, Verdict};
    use serde_json::json;

    #[test]
    fn missing_flags_are_named() {
        let fixture = json!({"capabilities": {"auth.ipc_mode": "peer_token"}});
        let caps = json!({"auth": {"ipc_mode": "peer"}});
        let Flags::Missing(why) = flags(&fixture, &caps) else {
            panic!("a flag with another value is missing");
        };
        assert!(why.contains("auth.ipc_mode"));
        assert_eq!(
            flags(&json!({"capabilities": {}}), &caps),
            Flags::Declared {
                promised: Vec::new()
            }
        );
    }

    #[test]
    fn a_false_flag_admits_a_pending_row_and_a_true_flag_promises_it() {
        let caps = json!({"automation": {"dictation_macros": false, "jobs": false}, "history": {"fts": true}});
        assert_eq!(
            flags(
                &json!({"capabilities": {"automation.dictation_macros": true}}),
                &caps
            ),
            Flags::Admitted("automation.dictation_macros".into())
        );
        assert_eq!(
            flags(&json!({"capabilities": {"history.fts": true}}), &caps),
            Flags::Declared {
                promised: vec!["history.fts".into()]
            }
        );
        assert_eq!(
            flags(&json!({"capabilities": {"automation.jobs": false}}), &caps),
            Flags::Declared {
                promised: Vec::new()
            }
        );
        assert!(matches!(
            flags(
                &json!({"capabilities": {"automation.jobs": true}}),
                &json!({"automation": {"jobs": "no"}})
            ),
            Flags::Missing(_)
        ));
        let row = |admitted_by: Option<&str>| Row {
            fixture: "automation/macros.list.json".into(),
            method: "automation.macros.list".into(),
            verdict: Verdict::Pending,
            detail: Some("not implemented yet".into()),
            admitted_by: admitted_by.map(str::to_string),
        };
        let admitted = Report {
            rows: vec![row(Some("automation.dictation_macros"))],
            mcp: Vec::new(),
            rest: Vec::new(),
        };
        assert!(admitted.ok(true) && admitted.broken_promises().is_empty());
        assert_eq!(admitted.admitted().len(), 1);
        let promised = Report {
            rows: vec![row(None)],
            mcp: Vec::new(),
            rest: Vec::new(),
        };
        assert!(promised.ok(false) && !promised.ok(true));
        assert_eq!(
            promised.broken_promises(),
            ["automation.macros.list (not implemented yet)"]
        );
    }
}

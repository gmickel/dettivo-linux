//! What the settings drive shares (fn-26 R1): the route table the app
//! compiles in (`qt/host/app/settings_keys.cpp`, the one table the rows,
//! the lint and this drive read), the value the drive types for a key of
//! each kind, the keys the profile's environment locks, the few keys the
//! drive leaves to the file with the reason, and the comparison between a
//! typed text and the value `config.get` answers with.

use std::path::Path;

use serde_json::Value;

use crate::driver::Element;

/// One section's keys in row order, as the app's table lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteKeys {
    /// The section (`general`, `hotkeys`, ...).
    pub section: String,
    /// Its keys, dotted.
    pub keys: Vec<String>,
}

/// The block of `settings_keys.cpp` between two marker comments, as the
/// quoted strings of each line.
fn marked_block(text: &str, start: &str, end: &str) -> Result<Vec<Vec<String>>, String> {
    let from = text
        .find(start)
        .ok_or_else(|| format!("settings_keys.cpp has no {start} marker"))?;
    let to = text
        .find(end)
        .ok_or_else(|| format!("settings_keys.cpp has no {end} marker"))?;
    let mut rows = Vec::new();
    for line in text[from..to].lines() {
        let quoted: Vec<String> = line
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect();
        if line.trim_start().starts_with('{') && quoted.len() >= 2 {
            rows.push(quoted);
        }
    }
    Ok(rows)
}

/// The route table: every section in nav order with its keys.
pub fn route_keys(repo_root: &Path) -> Result<Vec<RouteKeys>, String> {
    let path = repo_root.join("qt/host/app/settings_keys.cpp");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut routes: Vec<RouteKeys> = Vec::new();
    for row in marked_block(&text, "// routes:start", "// routes:end")? {
        let (section, key) = (row[0].clone(), row[1].clone());
        match routes.iter_mut().find(|r| r.section == section) {
            Some(r) => r.keys.push(key),
            None => routes.push(RouteKeys {
                section,
                keys: vec![key],
            }),
        }
    }
    if routes.is_empty() {
        return Err("settings_keys.cpp lists no route keys".into());
    }
    Ok(routes)
}

/// The keys no route edits, with their reasons.
pub fn excluded_keys(repo_root: &Path) -> Result<Vec<(String, String)>, String> {
    let path = repo_root.join("qt/host/app/settings_keys.cpp");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(marked_block(&text, "// excluded:start", "// excluded:end")?
        .into_iter()
        .map(|row| (row[0].clone(), row[1].clone()))
        .collect())
}

/// Why the drive leaves a key to the file instead of its control; `None`
/// for a key the drive edits.
pub fn left_to_file(key: &str) -> Option<&'static str> {
    match key {
        "ipc.auth_mode" => Some("peer_token would lock the drive out of its own daemon"),
        "speech.provider" => Some("written by the speech.model choice together with the model"),
        _ => None,
    }
}

/// The variable that locks a key under the profile's environment (the
/// rig sets the socket; its QA switch is `DETTIVO_QA_MODE`, not the
/// `DETTIVO_QA` the schema reads, so `qa.mode` stays a row to edit).
pub fn locked_by(key: &str) -> Option<&'static str> {
    match key {
        "ipc.socket" => Some("DETTIVO_IPC_SOCKET"),
        _ => None,
    }
}

/// What the drive types into a field for `key` of the registry's `kind`;
/// every value validates and leaves the profile's daemon usable.
pub fn typed_value(key: &str, kind: &str, profile_root: &Path, bin_dir: &Path) -> String {
    let root = |sub: &str| profile_root.join(sub).to_string_lossy().into_owned();
    match key {
        "hotkeys.hold" | "hotkeys.toggle" | "hotkeys.cancel" | "hotkeys.reinsert" => {
            "SUPER ALT, F7".into()
        }
        // The daemon resolves a path key to the directory in force, so the
        // typed text has to differ from it while naming the same place.
        "paths.data_dir" => root("data/dettivo/"),
        "paths.models_dir" => root("data/dettivo/models/"),
        "history.db_path" => root("data/dettivo/drive.db"),
        "engines.directory" => bin_dir.to_string_lossy().into_owned(),
        "models.catalogue_file" => root("cfg/dettivo/catalogue.toml"),
        "ipc.token_file" => root("cfg/dettivo/drive.token"),
        "llm.api_key_file" => root("cfg/dettivo/llm.key"),
        "llm.experiments_dir" => root("data/dettivo/polish-experiments/"),
        "audio.input_device" => "qa-microphone".into(),
        "osd.monitor" => "DP-3".into(),
        "dictation.language" => "de".into(),
        "dictation.replacements" => "teh=the".into(),
        "speech.meeting_model" => "tiny.en".into(),
        "speech.parakeet_model_id" => "parakeet-v2".into(),
        "llm.analysis_model" => "qwen3-1.7b".into(),
        "llm.ollama_url" => "http://127.0.0.1:11435".into(),
        "llm.ollama_model" => "qwen3:1.7b".into(),
        "llm.endpoint_url" => "http://localhost:8080".into(),
        "llm.endpoint_model" => "gpt-4o-mini".into(),
        "llm.trusted_endpoints" => "http://localhost:9999".into(),
        "polish.transforms" => "fixGrammar".into(),
        "insert.paste_keys" => "qa=ctrl+v".into(),
        "insert.terminal_app_ids" => "foot, kitty".into(),
        "insert.self_app_ids" => "dettivo, dettivo-app".into(),
        "hotkeys.evdev_devices" => "/dev/input/event0".into(),
        "rest.bind" => "::1".into(),
        "omarchy.open_shortcut" => "SUPER ALT, D".into(),
        // The default is the generic float below; a value equal to the
        // default never moves the source to `file`.
        "meetings.diarization.clustering_threshold" => "0.35".into(),
        // Chunk length must exceed overlap plus the safety margin.
        "transcribe.chunk_seconds" => "30".into(),
        "meetings.live_window_ms" => "4000".into(),
        _ => match kind {
            "integer" if key.ends_with("_bytes") => "9000000".into(),
            "integer" => "7".into(),
            "float" => "0.5".into(),
            "list" => "alpha, beta".into(),
            "table" => "alpha=beta".into(),
            _ => "qa".into(),
        },
    }
}

/// Whether the value `config.get` answers with is the typed text after
/// the daemon's coercion: numbers by value, lists by their items, tables
/// by their pairs, text as is.
pub fn matches(typed: &str, value: &Value) -> bool {
    match value {
        Value::String(s) => s == typed,
        Value::Bool(b) => typed.parse::<bool>() == Ok(*b),
        // A parse that fails matches nothing: `None == None` once let
        // unequal floats (and text that is not a number) pass.
        Value::Number(n) => match (typed.parse::<i64>().ok(), n.as_i64()) {
            (Some(want), Some(got)) => want == got,
            _ => {
                matches!((typed.parse::<f64>().ok(), n.as_f64()), (Some(want), Some(got)) if want == got)
            }
        },
        Value::Array(items) => {
            let want: Vec<&str> = typed.split(',').map(str::trim).collect();
            let got: Vec<String> = items
                .iter()
                .map(|i| {
                    i.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| i.to_string())
                })
                .collect();
            want == got
        }
        // The typed pairs are the whole table: a surplus entry the daemon
        // kept is a failed round trip, not a coercion.
        Value::Object(map) => {
            let want: Option<serde_json::Map<String, Value>> = typed
                .split(',')
                .map(str::trim)
                .map(|pair| {
                    pair.split_once('=')
                        .map(|(k, v)| (k.trim().to_string(), Value::String(v.trim().to_string())))
                })
                .collect();
            want.as_ref() == Some(map)
        }
        Value::Null => false,
    }
}

/// The page tabs inside a segmented control, or following it when the
/// snapshot has no container geometry.
pub fn tabs_after<'a>(tree: &'a [Element], list: &Element) -> Vec<&'a Element> {
    if let Some((x, y, width, height)) = list.bounds {
        let (x, y) = (i64::from(x) * 2, i64::from(y) * 2);
        return tree
            .iter()
            .filter(|tab| {
                tab.role == "page tab"
                    && tab.bounds.is_some_and(|(tx, ty, tw, th)| {
                        let cx = i64::from(tx) * 2 + i64::from(tw);
                        let cy = i64::from(ty) * 2 + i64::from(th);
                        tw > 0
                            && th > 0
                            && cx >= x
                            && cy >= y
                            && cx < x + i64::from(width) * 2
                            && cy < y + i64::from(height) * 2
                    })
            })
            .collect();
    }
    tree.iter()
        .skip_while(|e| e.id != list.id)
        .skip(1)
        .take_while(|e| e.role == "page tab")
        .collect()
}

/// The elements of `after` that `before` did not have (a popup's items).
pub fn new_elements<'a>(before: &[Element], after: &'a [Element]) -> Vec<&'a Element> {
    after
        .iter()
        .filter(|e| !before.iter().any(|b| b.id == e.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn segmented_tabs_keep_action_tokens_when_the_container_is_appended() {
        let element = |id: &str, role: &str, bounds| Element {
            index: 0,
            id: id.into(),
            role: role.into(),
            name: id.into(),
            value: None,
            bounds: Some(bounds),
            focusable: false,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        };
        let list = element("list", "page tab list", (100, 100, 100, 20));
        let a = element("snapshot:1", "page tab", (100, 100, 50, 20));
        let b = element("snapshot:2", "page tab", (150, 100, 50, 20));
        let other_list = element("other", "page tab list", (100, 200, 100, 20));
        let other = element("snapshot:3", "page tab", (100, 200, 50, 20));
        for tree in [
            vec![
                list.clone(),
                a.clone(),
                b.clone(),
                other_list.clone(),
                other.clone(),
            ],
            vec![a, b, other, list.clone(), other_list],
        ] {
            assert_eq!(
                tabs_after(&tree, &list)
                    .iter()
                    .map(|tab| tab.id.as_str())
                    .collect::<Vec<_>>(),
                ["snapshot:1", "snapshot:2"]
            );
        }
        let mut geometryless = list.clone();
        geometryless.bounds = None;
        let tree = vec![
            geometryless.clone(),
            element("snapshot:1", "page tab", (100, 100, 50, 20)),
            element("snapshot:2", "page tab", (150, 100, 50, 20)),
        ];
        assert_eq!(tabs_after(&tree, &geometryless).len(), 2);
    }

    fn repo() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn timing_samples_fit_the_default_overlap_budgets() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../dettivo-proto/fixtures/config/print_default.json"
        ))
        .unwrap();
        let defaults: toml::Value =
            toml::from_str(fixture["response"]["result"]["text"].as_str().unwrap()).unwrap();
        let sample = |key| {
            typed_value(
                key,
                "integer",
                Path::new("/tmp/profile"),
                Path::new("/tmp/bin"),
            )
            .parse::<i64>()
            .unwrap()
        };
        let transcribe = &defaults["transcribe"];
        assert!(
            sample("transcribe.chunk_seconds")
                > transcribe["overlap_seconds"].as_integer().unwrap()
                    + transcribe["safety_margin_seconds"].as_integer().unwrap()
        );
        let meeting = &defaults["meetings"];
        assert!(
            sample("meetings.live_window_ms")
                >= meeting["live_overlap_ms"].as_integer().unwrap()
                    + meeting["live_tick_ms"].as_integer().unwrap()
        );
        assert!(
            meeting["live_window_ms"].as_integer().unwrap()
                >= sample("meetings.live_tick_ms")
                    + meeting["live_overlap_ms"].as_integer().unwrap()
        );
        assert!(
            meeting["live_window_ms"].as_integer().unwrap()
                >= sample("meetings.live_overlap_ms")
                    + meeting["live_tick_ms"].as_integer().unwrap()
        );
    }

    #[test]
    fn the_route_table_reads_every_section_once() {
        let routes = route_keys(&repo()).unwrap();
        let sections: Vec<&str> = routes.iter().map(|r| r.section.as_str()).collect();
        assert_eq!(
            sections,
            [
                "general",
                "vocabulary",
                "hotkeys",
                "models",
                "polish",
                "insertion",
                "meetings",
                "agents",
                "diagnostics"
            ]
        );
        assert!(routes[1].keys.contains(&"dictation.vocabulary".to_string()));
        assert!(routes[2].keys.contains(&"hotkeys.hold".to_string()));
        let excluded = excluded_keys(&repo()).unwrap();
        assert!(excluded.iter().any(|(k, _)| k == "polish.rules"));
    }

    #[test]
    fn typed_values_match_their_coerced_answers() {
        assert!(matches("7", &json!(7)));
        assert!(matches("0.5", &json!(0.5)));
        assert!(matches("alpha, beta", &json!(["alpha", "beta"])));
        assert!(matches("teh=the", &json!({"teh": "the"})));
        assert!(matches("SUPER ALT, F7", &json!("SUPER ALT, F7")));
        assert!(!matches("7", &json!("7 ")));
        assert!(!matches("0.35", &json!(0.9)));
        assert!(!matches("abc", &json!(0.9)));
        assert!(!matches("7", &json!(7.5)));
        assert!(!matches("teh=the", &json!({"teh": "the", "x": "y"})));
        assert!(!matches("teh=the", &json!({"teh": "teh"})));
        assert!(!matches("teh", &json!({"teh": "the"})));
        let root = Path::new("/p");
        assert_eq!(
            typed_value("ipc.max_line_bytes", "integer", root, root),
            "9000000"
        );
        assert_eq!(
            typed_value("paths.data_dir", "text", root, root),
            "/p/data/dettivo/"
        );
        assert!(left_to_file("ipc.auth_mode").is_some());
        assert_eq!(locked_by("ipc.socket"), Some("DETTIVO_IPC_SOCKET"));
        assert_eq!(locked_by("qa.mode"), None);
    }
}

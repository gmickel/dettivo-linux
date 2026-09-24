//! The negative text scan (fn-17 R1, fn-35 R6): no accessible name on a
//! release route carries developer text. The same classes
//! `scripts/lint-release-text.sh` refuses in QML literals, checked on the
//! live tree so a value that arrived from the daemon (a model path, an
//! error string) is caught too. The classes are the masterplan's full
//! list: exception text, a stack trace frame, raw JSON, a raw path, a
//! model file name, an internal identifier (`parity_gap`, a spec id, a
//! marker word), the word `debug`, and a hash or an address.

use crate::driver::Element;

/// One finding: which class of text, in which element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// One of [`CLASSES`].
    pub class: &'static str,
    /// The element's accessible name.
    pub name: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} in {:?}", self.class, self.name)
    }
}

/// Every class the scan names, in the order they are tried.
pub const CLASSES: &[&str] = &[
    "exception text",
    "stack trace",
    "raw JSON",
    "raw path",
    "model file name",
    "internal identifier",
    "debug text",
    "hash or address",
];

/// The prefix of every drive profile root (`profile::Profile::create`
/// builds `/tmp/dq<scenario>-<random>`): a path under it in an accessible
/// name is the rig's own location shown back, which a user's machine
/// would show home-relative, so the pack-level scan sets it aside. It is
/// a location exemption for the rig. Named path editor values have their
/// own semantic check below.
pub const PROFILE_ROOT_PREFIX: &str = "/tmp/dq";

/// Classifies one text; `None` when it is clean.
pub fn classify(text: &str) -> Option<&'static str> {
    const EXCEPTIONS: &[&str] = &[
        "Exception",
        "Traceback",
        "panicked",
        "unwrap()",
        "NullPointer",
    ];
    const PATHS: &[&str] = &["/home/", "/tmp/", "/usr/", "/var/"];
    const MODELS: &[&str] = &[".gguf", ".onnx", "ggml-", ".safetensors"];
    const IDENTIFIERS: &[&str] = &[
        "TODO",
        "FIXME",
        "XXX",
        "lorem ipsum",
        "placeholder text",
        "parity_gap",
    ];
    if EXCEPTIONS.iter().any(|p| text.contains(p)) {
        return Some("exception text");
    }
    if has_source_frame(text) {
        return Some("stack trace");
    }
    if is_raw_json(text) {
        return Some("raw JSON");
    }
    if PATHS.iter().any(|p| text.contains(p)) || has_bare_hidden_dir(text) {
        return Some("raw path");
    }
    if MODELS.iter().any(|p| text.contains(p)) || has_bin_suffix(text) {
        return Some("model file name");
    }
    if IDENTIFIERS.iter().any(|p| text.contains(p)) || has_spec_id(text) {
        return Some("internal identifier");
    }
    if has_word_debug(text) {
        return Some("debug text");
    }
    if has_hex_run(text, 32) || has_hex_address(text) {
        return Some("hash or address");
    }
    None
}

/// The texts an element shows: its name and, when it exposes one that is
/// not the name again, its value (the text of a field, the label of a
/// static text the driver reads through the value interface).
fn texts(e: &Element) -> impl Iterator<Item = &str> {
    std::iter::once(e.name.as_str())
        .chain(e.value.as_deref().filter(|v| !v.is_empty() && *v != e.name))
}

/// A path editor must expose its exact value so it can be read and edited.
/// This exception applies to the value of a named path key only, never a
/// label, a diagnostic, or text that merely contains a path. A path such as
/// a checkout's `target/debug` may trip the `debug` word instead of a path
/// marker, and is still the key's value.
fn configuration_path_value(e: &Element, text: &str, class: &str) -> bool {
    const PATH_KEYS: &[&str] = &[
        "paths.data_dir",
        "paths.models_dir",
        "history.db_path",
        "engines.directory",
        "models.catalogue_file",
        "ipc.socket",
        "ipc.token_file",
        "llm.api_key_file",
        "llm.experiments_dir",
    ];
    matches!(class, "raw path" | "debug text")
        && matches!(e.role.as_str(), "text" | "entry")
        && PATH_KEYS.contains(&e.name.as_str())
        && e.value.as_deref() == Some(text)
        && std::path::Path::new(text).is_absolute()
        && !text.contains(['\n', '\r'])
}

/// Every element whose name or value carries developer text.
pub fn findings(elements: &[Element]) -> Vec<Finding> {
    elements
        .iter()
        .flat_map(|e| {
            texts(e).filter_map(|text| {
                classify(text)
                    .filter(|class| !configuration_path_value(e, text, class))
                    .map(|class| Finding {
                        class,
                        name: text.to_string(),
                    })
            })
        })
        .collect()
}

/// `text` with every span that is the drive's own profile root (a
/// `PROFILE_ROOT_PREFIX` path up to the next whitespace) taken out, so
/// what is left is classified on its own: another path, a diagnostic or
/// an identifier beside the profile path still counts.
pub fn without_profile_root(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(PROFILE_ROOT_PREFIX) {
        out.push_str(&rest[..at]);
        let span = rest[at..]
            .find(char::is_whitespace)
            .unwrap_or(rest.len() - at);
        rest = &rest[at + span..];
    }
    out.push_str(rest);
    out
}

/// `findings` with the additional exemption of the drive's own profile root
/// (`PROFILE_ROOT_PREFIX`), taken out of each text before it is judged.
pub fn findings_outside_profile(elements: &[Element]) -> Vec<Finding> {
    elements
        .iter()
        .flat_map(|e| {
            texts(e).filter_map(|text| {
                classify(&without_profile_root(text))
                    .filter(|class| !configuration_path_value(e, text, class))
                    .map(|class| Finding {
                        class,
                        name: text.to_string(),
                    })
            })
        })
        .collect()
}

/// A source location as a stack frame prints it: `.rs:`, `.cpp:`, `.h:`
/// or `.qml:` followed by a line number.
fn has_source_frame(text: &str) -> bool {
    [".rs:", ".cpp:", ".qml:", ".h:"].iter().any(|ext| {
        text.match_indices(ext).any(|(i, _)| {
            text[i + ext.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        })
    })
}

/// Text that is a JSON object or array: a serialised structure shown
/// where a sentence belongs.
fn is_raw_json(text: &str) -> bool {
    let trimmed = text.trim();
    let shaped = (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'));
    if !shaped {
        return false;
    }
    matches!(
        serde_json::from_str::<serde_json::Value>(trimmed),
        Ok(serde_json::Value::Object(_)) | Ok(serde_json::Value::Array(_))
    )
}

/// The word `debug` inside a text, in any case: `debugging` is a word of
/// its own and passes, and a text that is nothing but the word is a
/// value the product enumerates (the `debug` log level between `info`
/// and `trace`), not a sentence about debugging.
fn has_word_debug(text: &str) -> bool {
    let lower = text.to_lowercase();
    if lower.trim() == "debug" {
        return false;
    }
    lower.match_indices("debug").any(|(i, _)| {
        let before = lower[..i].chars().next_back();
        let after = lower[i + 5..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// `.local/` or `.config/` not written home-relative (`~/.config/...`) and
/// not inside a quoted configuration path (`"/.config/hypr/dettivo.lua"`):
/// a location the user is told about is user text, a path that leaked
/// from the daemon is not.
fn has_bare_hidden_dir(text: &str) -> bool {
    ["/.local/", "/.config/"].iter().any(|dir| {
        text.match_indices(dir).any(|(i, _)| {
            let before = text[..i].chars().next_back();
            !matches!(before, Some('~') | Some('"'))
        })
    }) || [".local/", ".config/"].iter().any(|dir| {
        text.match_indices(dir).any(|(i, _)| {
            let before = text[..i].chars().next_back();
            !matches!(before, Some('/'))
        })
    })
}

/// `.bin` as a file suffix (`ggml-small.bin`), not the word `bin`.
fn has_bin_suffix(text: &str) -> bool {
    text.match_indices(".bin").any(|(i, _)| {
        let after = text[i + 4..].chars().next();
        after.is_none_or(|c| !c.is_alphanumeric())
    })
}

/// `fn-` followed by a digit: a flow-next id.
fn has_spec_id(text: &str) -> bool {
    text.match_indices("fn-").any(|(i, _)| {
        text[i + 3..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    })
}

fn has_hex_run(text: &str, at_least: usize) -> bool {
    let mut run = 0;
    for c in text.chars() {
        if c.is_ascii_hexdigit() {
            run += 1;
            if run >= at_least {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// `0x` followed by six or more hex digits.
fn has_hex_address(text: &str) -> bool {
    text.match_indices("0x").any(|(i, _)| {
        text[i + 2..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .count()
            >= 6
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(name: &str) -> Element {
        Element {
            index: 0,
            id: "a".into(),
            role: "label".into(),
            name: name.into(),
            value: None,
            bounds: None,
            focusable: false,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        }
    }

    #[test]
    fn classes_are_caught_and_clean_text_passes() {
        let cases = [
            ("Ready.", None),
            (
                "Hold F9 to dictate into ghostty. Super+Ctrl+X toggles.",
                None,
            ),
            ("whisper large-v3-turbo: warm", None),
            ("[history] keep_audio · audio_retention_days", None),
            ("Start it with systemctl --user start dettivod.socket", None),
            ("thread main panicked at", Some("exception text")),
            ("/home/gordon/.local/share/dettivo", Some("raw path")),
            ("ggml-large-v3-turbo.bin", Some("model file name")),
            ("model small.bin", Some("model file name")),
            ("binary dettivo-engine-whisper", None),
            ("TODO wire this", Some("internal identifier")),
            ("fn-17 shell", Some("internal identifier")),
            ("fn-x", None),
            (
                "7c9e66797425e07fc1f90ae77c9e66797425e07f",
                Some("hash or address"),
            ),
            ("at 0x7fff5a3b", Some("hash or address")),
            ("at src/app/router.rs:42", Some("stack trace")),
            ("AppWindow.qml:12: TypeError", Some("stack trace")),
            ("{\"outcome\": \"inserted\"}", Some("raw JSON")),
            ("[1, 2, 3]", Some("raw JSON")),
            ("[history] keep_audio", None),
            ("parity_gap: meetings.export", Some("internal identifier")),
            ("Debug log written", Some("debug text")),
            ("debugging", None),
            ("Debugger attached", None),
            ("debug", None),
            ("DEBUG", None),
            ("debug mode", Some("debug text")),
        ];
        for (text, expected) in cases {
            assert_eq!(classify(text), expected, "{text}");
        }
        let elements = vec![element("Ready."), element("/tmp/x")];
        let found = findings(&elements);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].to_string(), "raw path in \"/tmp/x\"");
        for class in CLASSES {
            assert!(!class.is_empty());
        }
    }

    #[test]
    fn the_profile_root_is_the_one_path_set_aside() {
        let elements = vec![
            element("/tmp/dqsettings-abc/data/dettivo/"),
            element("/home/gordon/.local/share/dettivo"),
            element("{\"a\": 1}"),
        ];
        assert_eq!(findings(&elements).len(), 3);
        let outside = findings_outside_profile(&elements);
        assert_eq!(outside.len(), 2);
        assert_eq!(outside[0].class, "raw path");
        assert_eq!(outside[1].class, "raw JSON");
    }

    #[test]
    fn path_editors_keep_exact_values_without_exempting_labels_or_diagnostics() {
        let field = Element {
            role: "text".into(),
            value: Some("/home/alice/custom-engines".into()),
            ..element("engines.directory")
        };
        assert!(findings(std::slice::from_ref(&field)).is_empty());
        assert!(findings_outside_profile(std::slice::from_ref(&field)).is_empty());
        let checkout = Element {
            value: Some("/__w/dettivo-linux/dettivo-linux/target/debug".into()),
            ..field.clone()
        };
        assert!(findings(std::slice::from_ref(&checkout)).is_empty());
        let label = Element {
            role: "label".into(),
            ..field.clone()
        };
        assert_eq!(findings(&[label])[0].class, "raw path");
        let other_field = Element {
            name: "Status".into(),
            ..field.clone()
        };
        assert_eq!(findings(&[other_field])[0].class, "raw path");
        let diagnostic = Element {
            value: Some("/home/alice/custom-engines\nthread panicked".into()),
            ..field
        };
        assert_eq!(findings(&[diagnostic])[0].class, "exception text");
    }

    #[test]
    fn only_the_profile_span_is_set_aside_and_values_are_scanned_too() {
        // A profile path beside a real path, a diagnostic or an identifier
        // used to hide the whole string.
        let mixed = vec![
            element("Stored under /tmp/dqhist-1/data and /home/gordon/.local/share/dettivo"),
            element("/tmp/dqhist-1/data/dettivo: thread main panicked at"),
            element("TODO wire /tmp/dqhist-1/state"),
        ];
        let classes: Vec<&str> = findings_outside_profile(&mixed)
            .iter()
            .map(|f| f.class)
            .collect();
        assert_eq!(
            classes,
            ["raw path", "exception text", "internal identifier"]
        );
        assert_eq!(without_profile_root("a /tmp/dqhist-1/data b"), "a  b");
        assert!(findings_outside_profile(&[element("/tmp/dqhist-1/data/dettivo/")]).is_empty());

        // A value carries text a name does not.
        let field = Element {
            index: 0,
            id: "f".into(),
            role: "text".into(),
            name: "Model directory".into(),
            value: Some("/home/gordon/.local/share/dettivo/models".into()),
            bounds: None,
            focusable: false,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        };
        let found = findings(std::slice::from_ref(&field));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].class, "raw path");
        assert_eq!(found[0].name, "/home/gordon/.local/share/dettivo/models");
        let same = Element {
            value: Some("Model directory".into()),
            ..field
        };
        assert!(
            findings(&[same]).is_empty(),
            "a value that repeats the name is one text"
        );
    }
}

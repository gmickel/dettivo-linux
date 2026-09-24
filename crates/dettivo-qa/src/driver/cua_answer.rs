//! Reading a `cua-driver` answer: a refusal or failure the tool reports
//! inside an exit-0 answer, the image payload of a screenshot, the
//! accessibility tree the driver renders as markdown, and the base64 the
//! payloads carry. Split from `cua.rs` to keep both files readable.

use serde_json::Value;

use super::Element;

/// CUA accepts modifiers separately; shifted punctuation needs an explicit
/// modifier because a keysym alone does not encode the physical chord.
pub(super) fn key_press(pid: u32, chord: &str) -> Value {
    let (modifiers, key) = chord.rsplit_once('+').unwrap_or(("", chord));
    let mut modifiers: Vec<_> = modifiers
        .split('+')
        .filter(|m| !m.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let key = if key == "?" {
        if !modifiers.iter().any(|m| m == "shift") {
            modifiers.push("shift".into());
        }
        "slash".to_string()
    } else if key.len() == 1 && key.is_ascii() {
        // Driver key names are case-insensitive, like the AT-SPI path.
        // CUA treats an uppercase glyph as an implicit Shift request.
        key.to_ascii_lowercase()
    } else {
        key.to_string()
    };
    serde_json::json!({"pid": pid, "key": key, "modifiers": modifiers})
}

/// Why a tool answer is a refusal or a failure rather than a result:
/// `{"status": "refused", "refusal": {code, message}}`, the input
/// tools' `{"effect": "refused", "code": ...}` form, or an MCP
/// `isError` result with its text.
pub(super) fn refusal(answer: &Value) -> Option<String> {
    if let (Some(code), Some(detail)) = (answer["code"].as_str(), answer["detail"].as_str()) {
        return Some(format!("{code}: {detail}"));
    }
    if answer["status"] == "refused" || answer["effect"] == "refused" {
        let code = answer["refusal"]["code"]
            .as_str()
            .or_else(|| answer["code"].as_str())
            .unwrap_or("refused");
        let message = answer["refusal"]["message"]
            .as_str()
            .or_else(|| answer["message"].as_str())
            .unwrap_or_default();
        return Some(
            format!("{code}: {message}")
                .trim_end_matches(": ")
                .to_string(),
        );
    }
    if answer["isError"] == Value::Bool(true) {
        let text = answer["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        return Some(if text.is_empty() {
            "the tool reported an error".to_string()
        } else {
            text
        });
    }
    None
}

/// The first base64 image payload in a tool answer (`screenshot`, `image`,
/// or an MCP `content[].data` block).
pub(super) fn find_image(v: &Value) -> Option<&str> {
    for key in [
        "screenshot_png_b64",
        "screenshot",
        "image",
        "screenshot_base64",
        "data",
    ] {
        if let Some(s) = v.get(key).and_then(Value::as_str) {
            if s.len() > 64 {
                return Some(s);
            }
        }
    }
    match v {
        Value::Object(map) => map.values().find_map(find_image),
        Value::Array(items) => items.iter().find_map(find_image),
        _ => None,
    }
}

/// Elements from cua-driver's markdown tree: lines like
/// `  - label = "Dettivo" [element_index 3]`. Rows without an index (the
/// non-actionable ones) get a high synthetic index so they never collide.
pub(super) fn parse_tree_markdown(markdown: &str) -> Vec<Element> {
    let mut out = Vec::new();
    for (n, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("- ") else {
            continue;
        };
        let Some((role, tail)) = rest.split_once(" = ") else {
            continue;
        };
        let tail = tail.trim();
        let Some(after_quote) = tail.strip_prefix('"') else {
            continue;
        };
        let (quoted, bracket) = match after_quote.rfind(" [element_index ") {
            Some(at) => (&after_quote[..at], &after_quote[at..]),
            None => (after_quote, ""),
        };
        let Some(quoted) = quoted.trim_end().strip_suffix('"') else {
            continue;
        };
        let name = quoted.replace("\\\"", "\"");
        let index = bracket
            .split("[element_index ")
            .nth(1)
            .and_then(|t| t.split(']').next())
            .and_then(|t| t.trim().parse::<u32>().ok())
            .unwrap_or(100_000 + n as u32);
        out.push(Element {
            index,
            id: format!("label:{name}"),
            role: role.trim().to_string(),
            name,
            value: None,
            bounds: None,
            focusable: false,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        });
    }
    out
}

/// CUA's markdown-only containers carry no geometry. Supplement missing
/// facts from an unambiguous AT-SPI twin; keep every CUA action token.
pub(super) fn supplement_missing_facts(elements: &mut [Element], native: &[Element]) {
    for element in elements.iter_mut() {
        if element.name.is_empty() {
            continue;
        }
        let matches: Vec<_> = native
            .iter()
            .filter(|e| e.role == element.role && e.name == element.name)
            .collect();
        let twin = if matches.len() == 1 {
            matches.first().copied()
        } else {
            let exact: Vec<_> = matches
                .into_iter()
                .filter(|e| element.bounds.is_some() && e.bounds == element.bounds)
                .collect();
            (exact.len() == 1).then(|| exact[0])
        };
        if let Some(twin) = twin {
            element.bounds = element.bounds.or(twin.bounds);
            if element.value.is_none() {
                element.value = twin.value.clone();
            }
            element.native_id = Some(twin.id.clone());
            element.focusable = twin.focusable;
            element.focused = twin.focused;
            element.enabled = twin.enabled;
        }
    }
    // Map the nearest represented ancestor into CUA's index namespace.
    // This keeps controls inside arrow-navigated rows out of the Tab chain.
    let ids: std::collections::BTreeMap<_, _> = elements
        .iter()
        .filter_map(|e| e.native_id.as_ref().map(|id| (id.clone(), e.index)))
        .collect();
    for element in elements.iter_mut() {
        let mut parent = element
            .native_id
            .as_ref()
            .and_then(|id| native.iter().find(|e| &e.id == id))
            .and_then(|e| e.parent);
        while let Some(index) = parent {
            let Some(ancestor) = native.iter().find(|e| e.index == index) else {
                break;
            };
            if let Some(mapped) = ids.get(&ancestor.id) {
                element.parent = Some(*mapped);
                break;
            }
            parent = ancestor.parent;
        }
    }
}

pub(super) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.split_once(',').map(|(_, b)| b).unwrap_or(text);
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits = 0;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn key_chords_use_cua_modifiers_and_question_mark_uses_shift_slash() {
        assert_eq!(
            key_press(7, "Shift+Tab"),
            json!({"pid":7,"key":"Tab","modifiers":["shift"]})
        );
        assert_eq!(
            key_press(7, "Super+F"),
            json!({"pid":7,"key":"f","modifiers":["super"]})
        );
        assert_eq!(
            key_press(7, "?"),
            json!({"pid":7,"key":"slash","modifiers":["shift"]})
        );
        assert_eq!(key_press(7, "/"), json!({"pid":7,"key":"/","modifiers":[]}));
    }

    #[test]
    fn markdown_tree_rows_become_elements() {
        let md = "- frame = \"Dettivo\"\n  - label = \"Dettivo\"\n  - push button = \"Start\" [element_index 3]\n";
        let els = parse_tree_markdown(md);
        assert_eq!(els.len(), 3);
        assert_eq!(els[1].name, "Dettivo");
        assert_eq!(els[1].role, "label");
        assert_eq!(els[2].index, 3);
        assert_eq!(els[2].role, "push button");
        let quoted = parse_tree_markdown(
            "- label = \"No match for \"design\".\"\n- label = \"say \\\"hi\\\"\" [element_index 7]\n",
        );
        assert_eq!(quoted[0].name, "No match for \"design\".");
        assert_eq!(quoted[1].name, "say \"hi\"");
        assert_eq!(quoted[1].index, 7);
    }

    #[test]
    fn native_geometry_supplements_unique_containers_without_replacing_tokens() {
        let mut cua = parse_tree_markdown("- list = \"History\"\n- text = \"Search\"\n");
        cua[1].id = "snapshot:7".into();
        let mut native = cua.clone();
        native[0].bounds = Some((1, 2, 300, 400));
        native[1].value = Some("query".into());
        supplement_missing_facts(&mut cua, &native);
        assert_eq!(cua[0].bounds, Some((1, 2, 300, 400)));
        assert_eq!(cua[1].value.as_deref(), Some("query"));
        assert_eq!(cua[1].id, "snapshot:7");
        cua[0].bounds = None;
        native.push(native[0].clone());
        supplement_missing_facts(&mut cua, &native);
        assert_eq!(
            cua[0].bounds, None,
            "ambiguous twins cannot supply geometry"
        );
    }

    #[test]
    fn native_focus_follows_observed_tab_changes_while_action_tokens_change() {
        let mut cua = parse_tree_markdown(
            "- push button = \"Home\" [element_index 1]\n- push button = \"History\" [element_index 2]\n",
        );
        let mut native = cua.clone();
        for (index, element) in native.iter_mut().enumerate() {
            element.id = format!("/accessible/{index}");
            element.bounds = Some((10, 20 + index as i32 * 30, 100, 30));
            element.focusable = true;
        }
        for focus in [None, Some(0), Some(1), None] {
            for (index, element) in native.iter_mut().enumerate() {
                element.focused = focus == Some(index);
            }
            for (index, element) in cua.iter_mut().enumerate() {
                element.id = format!("snapshot-{focus:?}:{index}");
            }
            supplement_missing_facts(&mut cua, &native);
            for (index, element) in cua.iter().enumerate() {
                assert_eq!(element.focused, focus == Some(index));
                assert_eq!(
                    element.native_id.as_deref(),
                    Some(native[index].id.as_str())
                );
                assert_eq!(element.id, format!("snapshot-{focus:?}:{index}"));
            }
        }
    }

    #[test]
    fn native_parent_indices_are_translated_and_ambiguous_focus_is_not_invented() {
        let mut cua = parse_tree_markdown(
            "- list item = \"Row\" [element_index 3]\n- push button = \"Play\" [element_index 7]\n",
        );
        let mut native = cua.clone();
        native[0].id = "/row".into();
        native[0].index = 20;
        native[1].id = "/play".into();
        native[1].index = 21;
        native[1].parent = Some(20);
        native[1].focused = true;
        supplement_missing_facts(&mut cua, &native);
        assert_eq!(cua[1].parent, Some(3));
        assert!(cua[1].focused);
        let mut ambiguous = parse_tree_markdown("- push button = \"Play\" [element_index 7]\n");
        native.push(native[1].clone());
        supplement_missing_facts(&mut ambiguous, &native);
        assert!(!ambiguous[0].focused);
        assert!(ambiguous[0].native_id.is_none());
    }

    #[test]
    fn a_refused_or_failed_answer_is_named_and_a_result_is_not() {
        let unavailable =
            json!({"code": "background_unavailable", "detail": "no focus-free input backend"});
        assert_eq!(
            refusal(&unavailable).as_deref(),
            Some("background_unavailable: no focus-free input backend")
        );
        let bare_index = json!({"status": "refused", "refusal": {"code": "snapshot_id_required", "message": "click: bare element_index is not accepted"}});
        assert_eq!(
            refusal(&bare_index).as_deref(),
            Some("snapshot_id_required: click: bare element_index is not accepted")
        );
        let no_window = json!({"effect": "refused", "code": "window_target_not_found", "pid": 7});
        assert_eq!(
            refusal(&no_window).as_deref(),
            Some("window_target_not_found")
        );
        let mcp = json!({"isError": true, "content": [{"type": "text", "text": "no such window"}]});
        assert_eq!(refusal(&mcp).as_deref(), Some("no such window"));
        let state =
            json!({"elements": [], "degraded": true, "degraded_reason": "atspi_walk_failed"});
        assert!(refusal(&state).is_none());
        assert!(refusal(&json!({"effect": "clicked"})).is_none());
    }

    #[test]
    fn base64_and_image_lookup_work() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        let long = "A".repeat(100);
        let v = json!({"content": [{"type": "image", "data": long}]});
        assert!(find_image(&v).is_some());
        assert!(find_image(&json!({"x": 1})).is_none());
    }
}

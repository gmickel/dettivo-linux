//! Reading an analysis out of a model's answer, strictly: the thinking
//! block and any code fence are stripped, the first JSON object is
//! parsed, the macOS keys and their camel-case spellings are accepted,
//! and an answer with no usable summary is degenerate so the caller can
//! ask once more. An answer that opens a JSON object and never closes it
//! was cut off by the output budget; `cut_off` tells the caller so it
//! splits the part instead of asking for a repair.

use dettivo_proto::methods::meetings_notes::{ActionItem, Analysis};
use serde_json::Value;

use crate::enhanced::guard::strip_thinking;

/// The JSON object in `answer`, or why there is none.
fn object_of(answer: &str) -> Result<Value, String> {
    let stripped = strip_thinking(answer, false);
    let text = stripped.trim();
    if text.is_empty() {
        return Err("empty output".into());
    }
    let start = text.find('{').ok_or("no JSON object in the output")?;
    let end = text.rfind('}').ok_or("no JSON object in the output")?;
    if end < start {
        return Err("no JSON object in the output".into());
    }
    serde_json::from_str::<Value>(&text[start..=end]).map_err(|e| format!("invalid JSON: {e}"))
}

/// Whether `answer` opens a JSON object the output budget cut short: the
/// object never closes, or what closes is an inner value and the outer
/// object ends at the end of the text. Such an answer is not repaired
/// (the repair prompt is longer still); the part is split instead.
pub fn cut_off(answer: &str) -> bool {
    let stripped = strip_thinking(answer, false);
    let text = stripped.trim();
    let Some(start) = text.find('{') else {
        return false;
    };
    let Some(end) = text.rfind('}') else {
        return true;
    };
    if end < start {
        return true;
    }
    match serde_json::from_str::<Value>(&text[start..=end]) {
        Ok(_) => false,
        Err(e) => e.classify() == serde_json::error::Category::Eof,
    }
}

fn string_of(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn strings_of(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::Object(o) => o
                    .get("text")
                    .or_else(|| o.get("decision"))
                    .and_then(string_of),
                other => string_of(other),
            })
            .collect(),
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        _ => Vec::new(),
    }
}

fn action_of(value: &Value) -> Option<ActionItem> {
    match value {
        Value::Object(o) => {
            let text = o
                .get("text")
                .or_else(|| o.get("task"))
                .or_else(|| o.get("action"))
                .and_then(string_of)?;
            let owner = o
                .get("owner")
                .or_else(|| o.get("assignee"))
                .and_then(string_of);
            let due = o
                .get("due")
                .or_else(|| o.get("due_date"))
                .or_else(|| o.get("dueDate"))
                .and_then(string_of);
            Some(ActionItem { text, owner, due })
        }
        other => string_of(other).map(|text| ActionItem {
            text,
            owner: None,
            due: None,
        }),
    }
}

/// The analysis in `answer`, or the reason it is unusable.
pub fn parse(answer: &str) -> Result<Analysis, String> {
    let object = object_of(answer)?;
    let map = object.as_object().ok_or("the JSON is not an object")?;
    let summary = map
        .get("summary")
        .and_then(string_of)
        .ok_or("empty summary")?;
    let decisions = strings_of(map.get("decisions"));
    let action_items = map
        .get("action_items")
        .or_else(|| map.get("actionItems"))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(action_of).collect())
        .unwrap_or_default();
    Ok(Analysis {
        summary,
        decisions,
        action_items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_thinking_and_camel_case_are_read_and_an_empty_summary_is_refused() {
        let a = parse(
            "<think>hm</think>```json\n{\"summary\": \"Done.\", \"decisions\": [\"Ship\"], \"actionItems\": [{\"task\": \"Write\", \"assignee\": \"Mara\", \"dueDate\": \"Friday\"}, \"Call ops\"]}\n```",
        )
        .unwrap();
        assert_eq!(a.summary, "Done.");
        assert_eq!(a.decisions, ["Ship"]);
        assert_eq!(a.action_items[0].owner.as_deref(), Some("Mara"));
        assert_eq!(a.action_items[0].due.as_deref(), Some("Friday"));
        assert_eq!(a.action_items[1].text, "Call ops");
        assert_eq!(parse("").unwrap_err(), "empty output");
        assert_eq!(parse("Sure!").unwrap_err(), "no JSON object in the output");
        assert_eq!(parse("{\"summary\": \"\"}").unwrap_err(), "empty summary");
        assert!(
            parse("{\"summary\": \"x\",}")
                .unwrap_err()
                .starts_with("invalid JSON")
        );
        assert_eq!(parse("[1]").unwrap_err(), "no JSON object in the output");
    }

    #[test]
    fn an_object_the_budget_cut_short_is_told_from_a_bad_one() {
        assert!(cut_off("{\"summary\": \"The meeting agreed the"));
        assert!(cut_off(
            "<think>ok</think>{\"summary\": \"Agreed.\", \"action_items\": [{\"text\": \"Write\"}"
        ));
        assert!(cut_off(
            "{\"summary\": \"Agreed.\", \"decisions\": [\"a\", \"b"
        ));
        assert!(!cut_off("{\"summary\": \"Agreed.\", \"decisions\": []}"));
        assert!(!cut_off("{\"summary\": \"\"}"));
        assert!(!cut_off("Sure! Here is what I can do."));
        assert!(!cut_off(""));
        assert!(!cut_off("{\"summary\": \"x\",}"));
    }
}

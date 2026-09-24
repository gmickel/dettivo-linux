//! The reduce half of a chunked analysis. The model only ever combines
//! summaries, a few sentences in and a few sentences out, so the answer
//! fits the output budget whatever the meeting decided; the decisions
//! and the action items of the parts are joined here, in order, with
//! exact duplicates (after case and whitespace folding) removed once,
//! so a fact the model found is never lost to a merge that ran out of
//! tokens. `groups` bounds every merge call by a character limit while
//! still combining at least two summaries per call, so the reduction
//! always makes progress; a call that does not fit anyway lowers that
//! limit and retries in smaller groups (`analysis::Run::merge`).

use std::collections::HashSet;

use dettivo_proto::methods::meetings_notes::{ActionItem, Analysis};

/// Halves `text` for a second attempt after the model's answer was cut
/// off: on the middle line boundary, or in the middle of a single line
/// (at the nearest space before the middle when one is not too far
/// back); `None` when there is nothing left to halve.
pub fn halve(text: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() >= 2 {
        let cut = lines.len() / 2;
        return Some((lines[..cut].join("\n"), lines[cut..].join("\n")));
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 2 {
        return None;
    }
    let middle = chars.len() / 2;
    let cut = (1..middle)
        .rev()
        .find(|&i| chars[i].is_whitespace())
        .filter(|&i| i * 4 >= middle * 3)
        .unwrap_or(middle);
    Some((
        chars[..cut]
            .iter()
            .collect::<String>()
            .trim_end()
            .to_string(),
        chars[cut..]
            .iter()
            .collect::<String>()
            .trim_start()
            .to_string(),
    ))
}

/// The string as a duplicate key: whitespace folded, trailing sentence
/// punctuation dropped, lower-cased.
fn key(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(['.', '!', ';', ','])
        .to_lowercase()
}

/// The decisions and the action items of `parts`, in order, each
/// distinct one once.
pub fn combine(parts: &[Analysis]) -> (Vec<String>, Vec<ActionItem>) {
    let mut decisions = Vec::new();
    let mut seen_decisions = HashSet::new();
    let mut actions = Vec::new();
    let mut seen_actions = HashSet::new();
    for part in parts {
        for decision in &part.decisions {
            if seen_decisions.insert(key(decision)) {
                decisions.push(decision.clone());
            }
        }
        for item in &part.action_items {
            let k = format!(
                "{}\u{1f}{}\u{1f}{}",
                key(&item.text),
                item.owner.as_deref().map(key).unwrap_or_default(),
                item.due.as_deref().map(key).unwrap_or_default()
            );
            if seen_actions.insert(k) {
                actions.push(item.clone());
            }
        }
    }
    (decisions, actions)
}

/// Consecutive runs of `summaries` for one merge call each: a run takes
/// at least two summaries and grows while it stays under `limit`
/// characters; a summary left over at the end stands alone.
pub fn groups(summaries: &[String], limit: usize) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut i = 0;
    while i < summaries.len() {
        let mut group = vec![summaries[i].clone()];
        let mut chars = summaries[i].chars().count();
        i += 1;
        while i < summaries.len() {
            let next = summaries[i].chars().count();
            if group.len() >= 2 && chars + next > limit {
                break;
            }
            chars += next;
            group.push(summaries[i].clone());
            i += 1;
        }
        out.push(group);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(text: &str, owner: Option<&str>, due: Option<&str>) -> ActionItem {
        ActionItem {
            text: text.into(),
            owner: owner.map(Into::into),
            due: due.map(Into::into),
        }
    }

    #[test]
    fn halving_cuts_on_the_middle_line_then_inside_a_line_then_stops() {
        assert_eq!(
            halve("[a] one\n[b] two\n[a] three"),
            Some(("[a] one".into(), "[b] two\n[a] three".into()))
        );
        assert_eq!(
            halve("one two three four"),
            Some(("one two".into(), "three four".into()))
        );
        let (head, tail) = halve(&"x".repeat(50)).unwrap();
        assert_eq!(head.len() + tail.len(), 50);
        assert_eq!(halve("x"), None);
        assert_eq!(halve(""), None);
    }

    #[test]
    fn the_lists_join_in_order_and_a_repeat_is_kept_once() {
        let parts = [
            Analysis {
                summary: "a".into(),
                decisions: vec!["Ship on Thursday.".into(), "Hire two".into()],
                action_items: vec![item("Write the runbook", Some("Mara"), None)],
            },
            Analysis {
                summary: "b".into(),
                decisions: vec!["ship on  thursday".into(), "Renew the lease".into()],
                action_items: vec![
                    item("Write the runbook", Some("Mara"), Some("Wednesday")),
                    item("write the runbook.", Some("mara"), None),
                ],
            },
        ];
        let (decisions, actions) = combine(&parts);
        assert_eq!(
            decisions,
            ["Ship on Thursday.", "Hire two", "Renew the lease"]
        );
        assert_eq!(
            actions,
            [
                item("Write the runbook", Some("Mara"), None),
                item("Write the runbook", Some("Mara"), Some("Wednesday")),
            ]
        );
    }

    #[test]
    fn groups_take_at_least_two_and_stay_under_the_limit() {
        let s: Vec<String> = (0..5)
            .map(|i| format!("summary {i} {}", "x".repeat(90)))
            .collect();
        let g = groups(&s, 250);
        assert_eq!(g.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2, 1]);
        assert_eq!(groups(&s, 10_000).len(), 1);
        assert_eq!(groups(&s[..1], 10).len(), 1);
        assert!(groups(&[], 10).is_empty());
    }
}

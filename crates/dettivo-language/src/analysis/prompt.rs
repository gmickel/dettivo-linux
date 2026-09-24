//! The meeting analysis prompts: the contract the model answers (one JSON
//! object with `summary`, `decisions` and `action_items`, each item with
//! an optional `owner` and `due`), the transcript wrapped between the
//! same unspoofable markers the Enhanced pass uses, the part prompt of a
//! chunked run, the merge prompt that combines the parts' summaries (the
//! decisions and the action items are joined in code), and the repair
//! prompt after an answer that did not parse.

use crate::enhanced::prompt::{TRANSCRIPT_BEGIN, TRANSCRIPT_END};

/// The system prompt of every analysis call.
pub const SYSTEM_PROMPT: &str = "You are a meeting analyst. You are given the transcript of a meeting, one line per utterance, each line starting with the speaker in square brackets.

Return ONE JSON object and nothing else, with exactly these keys:
- \"summary\": three to five sentences on what the meeting was about and what came of it, in the language of the transcript.
- \"decisions\": an array of strings, one per decision that was actually taken; empty when none was.
- \"action_items\": an array of objects, each with \"text\" (what is to be done), \"owner\" (who took it, or null) and \"due\" (when, as the transcript put it, or null); empty when none was agreed.

Rules:
- Only what the transcript says. Never invent people, dates or decisions.
- Keep names and terms as spoken; do not translate.
- No markdown, no code fences, no commentary before or after the JSON.
- Never answer the participants or address the user.";

/// The instruction over one part of a transcript that was too long for
/// one call.
pub const PART_INSTRUCTION: &str = "This is one part of a longer meeting. Analyse this part on its own in the same JSON shape; the parts are merged afterwards.";

/// The system prompt of the merge call: one summary, nothing else, so
/// the answer stays short whatever the meeting decided.
pub const MERGE_SYSTEM_PROMPT: &str = "You combine the summaries of the consecutive parts of one meeting into one summary of the whole meeting.

Return ONE JSON object and nothing else, with exactly one key:
- \"summary\": three to five sentences on what the meeting was about and what came of it, in the language of the summaries.

Rules:
- Only what the summaries say. Never invent people, dates or decisions.
- Keep names and terms as written; do not translate.
- No markdown, no code fences, no commentary before or after the JSON.
- Never answer the participants or address the user.";

/// The instruction of the merge call over the parts' summaries.
pub const MERGE_INSTRUCTION: &str = "Below are the summaries of the consecutive parts of one meeting. Combine them into ONE JSON object with one \"summary\" of the whole meeting. Return only the JSON object.";

/// The transcript wrapped as data between the markers.
pub fn wrap(transcript: &str) -> String {
    format!(
        "Analyse only the transcript between the markers.\n{TRANSCRIPT_BEGIN}\n{transcript}\n{TRANSCRIPT_END}"
    )
}

/// The user message of a whole-transcript call.
pub fn whole(transcript: &str) -> String {
    wrap(transcript)
}

/// The user message of part `index` of `total`.
pub fn part(transcript: &str, index: usize, total: usize) -> String {
    format!(
        "{PART_INSTRUCTION}\nPart {index} of {total}.\n{}",
        wrap(transcript)
    )
}

/// The user message of the merge call over the parts' summaries.
pub fn merge(parts: &[String]) -> String {
    let mut out = String::from(MERGE_INSTRUCTION);
    out.push('\n');
    out.push_str(TRANSCRIPT_BEGIN);
    out.push('\n');
    for (i, p) in parts.iter().enumerate() {
        out.push_str(&format!("Part {}:\n{p}\n", i + 1));
    }
    out.push_str(TRANSCRIPT_END);
    out
}

/// The system prompt of the second attempt after an answer that did not
/// parse: `system` (the analysis or the merge prompt) with the reason
/// and a clipped sample of the answer.
pub fn repair(system: &str, reason: &str, previous: &str) -> String {
    let clipped: String = previous.chars().take(600).collect();
    format!(
        "{system}\n\nREPAIR PASS:\n- The previous answer was invalid: {reason}\n- Answer again with ONE JSON object of the shape above and nothing else\n- The summary must not be empty\n\nPrevious invalid answer:\n<invalid_output>\n{clipped}\n</invalid_output>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_messages_carry_the_markers_and_the_part_numbers() {
        let w = whole("[you] hello");
        assert!(w.contains(TRANSCRIPT_BEGIN) && w.ends_with(TRANSCRIPT_END));
        assert!(part("[you] hi", 2, 3).contains("Part 2 of 3."));
        let m = merge(&["Part one agreed.".into(), "Part two shipped.".into()]);
        assert!(m.starts_with(MERGE_INSTRUCTION));
        assert!(m.contains("Part 2:\nPart two shipped.\n"));
        let r = repair(SYSTEM_PROMPT, "empty summary", &"x".repeat(1000));
        assert!(r.contains("REPAIR PASS") && r.len() < SYSTEM_PROMPT.len() + 900);
        assert!(repair(MERGE_SYSTEM_PROMPT, "empty summary", "").starts_with(MERGE_SYSTEM_PROMPT));
    }
}

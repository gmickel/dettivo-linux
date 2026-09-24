//! The Markdown export of a meeting (ADR 0036): the title, the facts, a
//! Notes section under the precedence rule (the live editor's text, then
//! the saved notes), an Analysis section when one is ready (the summary,
//! the decisions, the action items with owner and due), and the
//! transcript with a speaker or side per segment and a gap marker where
//! capture was missing.

use crate::meeting_export::{Options, gap_line, label, notes_of, stamp, text_of, transcript_of};
use crate::meetings::MeetingRow;

fn facts(row: &MeetingRow) -> String {
    let mut out = format!("- id: `{}`\n- started: {}\n", row.id, row.started_at);
    if let Some(ended) = &row.ended_at {
        out.push_str(&format!("- ended: {ended}\n"));
    }
    out.push_str(&format!(
        "- duration: {} s\n- status: {}\n",
        row.duration_seconds(),
        row.status.as_str()
    ));
    if !row.language.is_empty() {
        out.push_str(&format!("- language: {}\n", row.language));
    }
    out.push_str(&format!(
        "- engine: {}/{}\n- source: {}\n",
        row.stt_provider, row.stt_model, row.source_kind
    ));
    let speakers = row.speaker_labels();
    if !speakers.is_empty() {
        out.push_str(&format!("- speakers: {}\n", speakers.join(", ")));
    }
    out
}

/// Renders the document.
pub fn render(row: &MeetingRow, options: &Options) -> String {
    let mut out = format!("# {}\n\n{}", row.title, facts(row));
    if let Some(notes) = notes_of(row, options) {
        out.push_str("\n## Notes\n\n");
        out.push_str(notes.trim_end());
        out.push('\n');
    }
    if let Some(analysis) = &row.analysis {
        out.push_str("\n## Analysis\n\n");
        out.push_str(analysis.summary.trim());
        out.push('\n');
        if !analysis.decisions.is_empty() {
            out.push_str("\n### Decisions\n\n");
            for d in &analysis.decisions {
                out.push_str(&format!("- {}\n", d.trim()));
            }
        }
        if !analysis.action_items.is_empty() {
            out.push_str("\n### Action items\n\n");
            for a in &analysis.action_items {
                let mut line = format!("- {}", a.text.trim());
                let tail: Vec<String> = [
                    a.owner.as_deref().map(|o| format!("owner: {o}")),
                    a.due.as_deref().map(|d| format!("due: {d}")),
                ]
                .into_iter()
                .flatten()
                .collect();
                if !tail.is_empty() {
                    line.push_str(&format!(" ({})", tail.join(", ")));
                }
                out.push_str(&line);
                out.push('\n');
            }
        }
    }
    out.push_str("\n## Transcript\n\n");
    for s in &row.segments {
        if let Some(gap) = gap_line(s) {
            out.push_str(&format!("- {gap}\n"));
        }
        out.push_str(&format!(
            "- [{}] {}: {}\n",
            stamp(s.start_ms, '.'),
            label(s),
            text_of(s, options)
        ));
    }
    if row.segments.is_empty() {
        out.push_str(&transcript_of(row, options));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_live_text_beats_the_saved_notes_and_both_sit_above_the_analysis() {
        let row = crate::seed_meetings::rich();
        let saved = render(&row, &Options::default());
        assert!(saved.contains("## Notes\n\n# Roadmap review\n\n- Budget for Q2 agreed"));
        assert!(saved.contains("- speakers: Gordon, Mara\n"));
        let notes_at = saved.find("## Notes").unwrap();
        let analysis_at = saved.find("## Analysis").unwrap();
        let transcript_at = saved.find("## Transcript").unwrap();
        assert!(notes_at < analysis_at && analysis_at < transcript_at);
        assert!(saved.contains("- Write the rollout runbook (owner: Gordon, due: Wednesday)\n"));
        assert!(saved.contains("- Ping ops about the café lease (owner: Mara)\n"));
        let live = render(
            &row,
            &Options {
                notes_override: Some("Live draft.".into()),
                raw: false,
            },
        );
        assert!(live.contains("## Notes\n\nLive draft.\n"));
        assert!(!live.contains("Budget for Q2 agreed"));
        let mut bare = row.clone();
        bare.notes_markdown.clear();
        let none = render(&bare, &Options::default());
        assert!(!none.contains("## Notes"), "{none}");
        assert!(none.contains("## Analysis"));
    }
}

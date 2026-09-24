//! `lint-deltas`: every method, reserved method, MCP tool, resource, export
//! format and event topic in the copied macOS contract has exactly one
//! disposition in `docs/api/linux-deltas.md`, and every disposition is one
//! of the fixed set.

use std::collections::BTreeMap;

use crate::{Outcome, ToolError};

/// The copied contract the register must cover.
pub const CONTRACT: &str = "docs/api/dettivo-ipc-v1.md";
/// The register.
pub const REGISTER: &str = "docs/api/linux-deltas.md";
/// The dispositions a row may carry.
pub const DISPOSITIONS: &[&str] = &["implemented", "covered-elsewhere", "deferred", "blocked"];

/// Runs the lint against the repository root found through git.
pub fn run() -> Result<Outcome, ToolError> {
    let root = crate::file_length::repo_root()?;
    let contract = std::fs::read_to_string(root.join(CONTRACT))
        .map_err(|e| ToolError(format!("{CONTRACT}: {e}")))?;
    let register = std::fs::read_to_string(root.join(REGISTER))
        .map_err(|e| ToolError(format!("{REGISTER}: {e}")))?;
    let found =
        violations(&contract, &register).map_err(|m| ToolError(format!("{REGISTER}: {m}")))?;
    for line in &found {
        println!("{line}");
    }
    if found.is_empty() {
        Ok(Outcome::Pass)
    } else {
        Ok(Outcome::Violations)
    }
}

/// Every violation, one line each, path first. An unparsable register is
/// a tool error rather than a violation.
pub fn violations(contract: &str, register: &str) -> Result<Vec<String>, String> {
    let rows = parse_register(register)?;
    let mut found = Vec::new();
    for (item, (kind, disposition)) in &rows {
        if !DISPOSITIONS.contains(&disposition.as_str()) {
            found.push(format!(
                "{REGISTER}: `{item}` ({kind}) has disposition {disposition:?}; expected one of {}",
                DISPOSITIONS.join(", ")
            ));
        }
    }
    for (kind, item) in contract_items(contract) {
        if !rows.contains_key(&item) {
            found.push(format!(
                "{REGISTER}: contract {kind} `{item}` has no disposition; add a row with one of {}",
                DISPOSITIONS.join(", ")
            ));
        }
    }
    Ok(found)
}

/// Rows between the register markers: item -> (kind, disposition). A
/// duplicate item or a missing marker is an error.
fn parse_register(register: &str) -> Result<BTreeMap<String, (String, String)>, String> {
    let start = register
        .find("<!-- register:start -->")
        .ok_or("missing `<!-- register:start -->` marker")?;
    let end = register
        .find("<!-- register:end -->")
        .ok_or("missing `<!-- register:end -->` marker")?;
    let mut rows = BTreeMap::new();
    for line in register[start..end].lines() {
        let cells: Vec<&str> = line
            .trim()
            .strip_prefix('|')
            .and_then(|l| l.strip_suffix('|'))
            .map(|l| l.split('|').map(str::trim).collect())
            .unwrap_or_default();
        if cells.len() < 3 || cells[0] == "Item" || cells[0].starts_with("---") {
            continue;
        }
        let item = cells[0].trim_matches('`').to_string();
        if rows
            .insert(item.clone(), (cells[1].to_string(), cells[2].to_string()))
            .is_some()
        {
            return Err(format!("`{item}` appears twice"));
        }
    }
    if rows.is_empty() {
        return Err("no rows between the register markers".into());
    }
    Ok(rows)
}

/// Every item the contract defines, as (kind, item).
fn contract_items(contract: &str) -> Vec<(&'static str, String)> {
    let mut items = Vec::new();
    let mut in_reserved = false;
    for line in contract.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed
            .strip_prefix("### `")
            .and_then(|r| r.strip_suffix('`'))
        {
            if name.contains('.') {
                items.push(("method", name.to_string()));
            }
        }
        if trimmed.starts_with("Reserved methods") {
            in_reserved = true;
            continue;
        }
        if in_reserved {
            if let Some(name) = trimmed
                .strip_prefix("- `")
                .and_then(|r| r.strip_suffix('`'))
            {
                items.push(("method", name.to_string()));
                continue;
            }
            if !trimmed.is_empty() {
                in_reserved = false;
            }
        }
        for prefix in ["- read:", "- control/write:", "- polish config writes:"] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                for tool in backticked(rest) {
                    items.push(("tool", tool));
                }
            }
        }
        for scope in ["\"dictation_export\":", "\"meeting_export\":"] {
            if let Some(rest) = trimmed.strip_prefix(scope) {
                for format in quoted(rest) {
                    items.push(("export-format", format));
                }
            }
        }
        if let Some(rest) = trimmed.strip_prefix("{ \"topics\":") {
            for topic in quoted(rest.split(']').next().unwrap_or("")) {
                items.push(("topic", topic));
            }
        }
        if trimmed.contains("`topic=events.overflow`") {
            items.push(("topic", "events.overflow".to_string()));
        }
    }
    for uri in [
        "status://current",
        "transcript://{id}",
        "meeting://{id}",
        "transcripts://search/{query}",
        "transcripts://latest/{kind}",
    ] {
        if contract.contains(uri) {
            items.push(("resource", uri.to_string()));
        }
    }
    items.sort();
    items.dedup();
    items
}

fn backticked(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .collect()
}

fn quoted(text: &str) -> Vec<String> {
    text.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &str = "\
## 6.2\n\
  \"formats\": {\n\
    \"dictation_export\": [\"txt\", \"md\"],\n\
    \"meeting_export\": [\"txt\", \"srt\"]\n\
  },\n\
### `system.ping`\n\
### `dictation.start`\n\
Reserved methods (shape protected now):\n\
- `knowledge.search`\n\
\n\
- read: `get_status`, `list_transcripts`\n\
{ \"topics\": [\"dictation.state\", \"job.progress\"], \"buffer\": 256 }\n\
- server emits `topic=events.overflow` with drop count\n\
  - `status://current`\n";

    fn register(rows: &str) -> String {
        format!(
            "intro\n<!-- register:start -->\n| Item | Kind | Disposition | Notes |\n|---|---|---|---|\n{rows}<!-- register:end -->\n"
        )
    }

    #[test]
    fn extracts_every_kind_of_item() {
        let items = contract_items(CONTRACT);
        let names: Vec<&str> = items.iter().map(|(_, i)| i.as_str()).collect();
        for expected in [
            "system.ping",
            "dictation.start",
            "knowledge.search",
            "get_status",
            "list_transcripts",
            "txt",
            "md",
            "srt",
            "dictation.state",
            "job.progress",
            "events.overflow",
            "status://current",
        ] {
            assert!(names.contains(&expected), "missing {expected}: {names:?}");
        }
    }

    #[test]
    fn passes_when_every_item_has_a_valid_disposition() {
        let rows = "| `system.ping` | method | implemented | |\n\
| `dictation.start` | method | implemented | |\n\
| `knowledge.search` | method | deferred | |\n\
| `get_status` | tool | implemented | |\n\
| `list_transcripts` | tool | implemented | |\n\
| `txt` | export-format | implemented | |\n\
| `md` | export-format | implemented | |\n\
| `srt` | export-format | implemented | |\n\
| `dictation.state` | topic | implemented | |\n\
| `job.progress` | topic | implemented | |\n\
| `events.overflow` | topic | implemented | |\n\
| `status://current` | resource | implemented | |\n";
        assert_eq!(
            violations(CONTRACT, &register(rows)).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn fails_on_a_missing_item_and_an_unknown_disposition() {
        let rows = "| `system.ping` | method | maybe | |\n";
        let text = violations(CONTRACT, &register(rows)).unwrap().join("\n");
        assert!(
            text.contains("`system.ping` (method) has disposition \"maybe\""),
            "{text}"
        );
        assert!(
            text.contains("contract method `dictation.start` has no disposition"),
            "{text}"
        );
    }

    #[test]
    fn duplicate_rows_and_missing_markers_are_errors() {
        let dup =
            "| `system.ping` | method | implemented | |\n| `system.ping` | method | deferred | |\n";
        assert!(
            violations(CONTRACT, &register(dup))
                .unwrap_err()
                .contains("appears twice")
        );
        assert!(
            violations(CONTRACT, "no markers")
                .unwrap_err()
                .contains("marker")
        );
    }
}

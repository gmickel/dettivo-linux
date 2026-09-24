//! The checklist as data: `docs/design/checklist.md` parsed into numbered
//! items, each with the command that proves it or `human`, and the
//! verdict of running every command once from the repository root. A
//! command that is not found fails by name; a command that exits non-zero
//! fails with its last lines.

use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// Where the checklist lives, relative to the repository root.
pub const CHECKLIST_PATH: &str = "docs/design/checklist.md";

/// One item of the checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// `C-01`.
    pub id: String,
    /// The section heading the item sits under.
    pub section: String,
    /// The rule, one sentence.
    pub text: String,
    /// The command, or `human`.
    pub check: String,
}

impl Item {
    /// True when a person decides.
    pub fn is_human(&self) -> bool {
        self.check == "human"
    }
}

/// The verdict of one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// The item.
    pub item: Item,
    /// `pass`, `fail` or `human`.
    pub outcome: String,
    /// The command's last lines, for a fail.
    pub detail: String,
}

/// Parses the checklist text: a heading `## Name` opens a section, an item
/// is `- **C-NN** text` followed by an indented `check: ...` line.
pub fn parse(text: &str) -> Result<Vec<Item>, String> {
    let mut items: Vec<Item> = Vec::new();
    let mut section = String::new();
    for (n, line) in text.lines().enumerate() {
        if let Some(rest) = line.strip_prefix("## ") {
            section = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("- **C-") {
            let (number, tail) = rest
                .split_once("**")
                .ok_or_else(|| format!("checklist line {}: unterminated item id", n + 1))?;
            items.push(Item {
                id: format!("C-{number}"),
                section: section.clone(),
                text: tail.trim().to_string(),
                check: String::new(),
            });
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("check:") {
            let Some(last) = items.last_mut() else {
                return Err(format!("checklist line {}: check before any item", n + 1));
            };
            if !last.check.is_empty() {
                return Err(format!(
                    "checklist line {}: {} has two checks",
                    n + 1,
                    last.id
                ));
            }
            last.check = rest.trim().to_string();
        }
    }
    if items.is_empty() {
        return Err("checklist: no items (expected `- **C-NN** ...` lines)".into());
    }
    for item in &items {
        if item.check.is_empty() {
            return Err(format!("checklist: {} names no check", item.id));
        }
    }
    let mut ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() != items.len() {
        return Err("checklist: an item id is listed twice".into());
    }
    Ok(items)
}

/// Reads and parses the repository's checklist.
pub fn load(repo: &Path) -> Result<Vec<Item>, String> {
    let path = repo.join(CHECKLIST_PATH);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text)
}

/// Runs every distinct command once and hands each item its verdict.
pub fn run_all(repo: &Path, items: &[Item]) -> Result<Vec<Verdict>, String> {
    let mut results: Vec<(String, (bool, String))> = Vec::new();
    let mut verdicts = Vec::with_capacity(items.len());
    for item in items {
        if item.is_human() {
            verdicts.push(Verdict {
                item: item.clone(),
                outcome: "human".into(),
                detail: String::new(),
            });
            continue;
        }
        let (ok, detail) = match results.iter().find(|(c, _)| *c == item.check) {
            Some((_, r)) => r.clone(),
            None => {
                let r = run_check(repo, &item.check)?;
                results.push((item.check.clone(), r.clone()));
                r
            }
        };
        verdicts.push(Verdict {
            item: item.clone(),
            outcome: if ok { "pass" } else { "fail" }.into(),
            detail,
        });
    }
    Ok(verdicts)
}

/// Runs one `check:` command from the repository root. The program is
/// resolved first so a missing one is named rather than reported as a
/// failed run.
pub fn run_check(repo: &Path, command: &str) -> Result<(bool, String), String> {
    let mut words = shell_words(command);
    if words.is_empty() {
        return Err("checklist: an empty check command".into());
    }
    let program = words.remove(0);
    let resolved = if program.contains('/') {
        let p = repo.join(&program);
        p.is_file().then_some(p)
    } else {
        std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|d| d.join(&program))
                .find(|p| p.is_file())
        })
    };
    let Some(resolved) = resolved else {
        return Err(format!("checklist: check command not found: {program}"));
    };
    // A script written a moment ago can still be open for writing in a
    // child another thread just forked; exec then fails with ETXTBSY until
    // that child execs, so a few short retries cover the window.
    let mut attempt = 0;
    let output = loop {
        let run = Command::new(&resolved)
            .args(&words)
            .current_dir(repo)
            .env_remove("DETTIVO_QA_MODE")
            .stdin(Stdio::null())
            .output();
        match run {
            Err(e) if e.raw_os_error() == Some(26) && attempt < 5 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => break other,
        }
    }
    .map_err(|e| format!("checklist: run {}: {e}", resolved.display()))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let tail: Vec<&str> = text.lines().rev().take(4).collect();
    let detail: Vec<&str> = tail.into_iter().rev().collect();
    Ok((output.status.success(), detail.join("\n")))
}

/// A whitespace split that keeps single- and double-quoted words whole.
fn shell_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in command.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => quote = Some(ch),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            (None, c) => current.push(c),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// `(pass, fail, human)` counts.
pub fn tally(verdicts: &[Verdict]) -> (usize, usize, usize) {
    let count = |o: &str| verdicts.iter().filter(|v| v.outcome == o).count();
    (count("pass"), count("fail"), count("human"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Design checklist\n\n## Tokens\n\n- **C-01** Colour from tokens.\n  check: scripts/lint-qml-tokens.sh\n- **C-02** One accent.\n  check: human\n\n## Copy\n\n- **C-03** Sentences end.\n  check: scripts/lint-copy.sh --self-test\n";

    #[test]
    fn the_checklist_parses_items_sections_and_checks() {
        let items = parse(SAMPLE).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id, "C-01");
        assert_eq!(items[0].section, "Tokens");
        assert_eq!(items[0].check, "scripts/lint-qml-tokens.sh");
        assert!(items[1].is_human());
        assert_eq!(items[2].section, "Copy");
        assert_eq!(items[2].check, "scripts/lint-copy.sh --self-test");
        let err = parse("## X\n- **C-01** no check\n").unwrap_err();
        assert!(err.contains("C-01 names no check"), "{err}");
        let twice = parse(&SAMPLE.replace("C-03", "C-01")).unwrap_err();
        assert!(twice.contains("twice"), "{twice}");
    }

    #[test]
    fn a_check_command_that_is_not_found_fails_by_name() {
        let repo = tempfile::tempdir().unwrap();
        let err = run_check(repo.path(), "scripts/no-such-lint.sh").unwrap_err();
        assert!(err.contains("not found: scripts/no-such-lint.sh"), "{err}");
        let err = run_check(repo.path(), "dettivo-no-such-tool --flag").unwrap_err();
        assert!(err.contains("not found: dettivo-no-such-tool"), "{err}");
    }

    #[test]
    fn verdicts_come_from_the_command_and_humans_stay_open() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join("scripts")).unwrap();
        let ok = repo.path().join("scripts/ok.sh");
        std::fs::write(&ok, "#!/bin/sh\necho fine\n").unwrap();
        let bad = repo.path().join("scripts/bad.sh");
        std::fs::write(&bad, "#!/bin/sh\necho 'lint: x.qml:3: rule' >&2\nexit 1\n").unwrap();
        for p in [&ok, &bad] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let items = parse(
            "## S\n- **C-01** a\n  check: scripts/ok.sh\n- **C-02** b\n  check: scripts/bad.sh\n- **C-03** c\n  check: human\n- **C-04** d\n  check: scripts/ok.sh\n",
        )
        .unwrap();
        let verdicts = run_all(repo.path(), &items).unwrap();
        assert_eq!(verdicts[0].outcome, "pass");
        assert_eq!(verdicts[1].outcome, "fail");
        assert!(verdicts[1].detail.contains("x.qml:3: rule"));
        assert_eq!(verdicts[2].outcome, "human");
        assert_eq!(verdicts[3].outcome, "pass");
        assert_eq!(tally(&verdicts), (2, 1, 1));
        assert_eq!(
            shell_words("a 'b c' \"d e\" f"),
            vec!["a", "b c", "d e", "f"]
        );
    }
}

//! `lint-file-length`: source files stay under a line limit unless listed in
//! `.file-length-allow` at the repo root (meant for generated code).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Outcome, ToolError};

/// Name of the allowlist file at the repo root.
pub const ALLOW_FILE: &str = ".file-length-allow";

/// Line limits by file extension.
const LIMITS: &[(&[&str], usize)] = &[
    (&["rs", "cpp", "cc", "cxx", "h", "hpp", "hh"], 500),
    (&["qml"], 300),
];

/// Top-level directories that are build output, never source, and may be
/// enumerated as untracked files before a `.gitignore` exists.
const BUILD_DIRS: &[&str] = &["target"];

/// Runs the lint against the current repository.
pub fn run() -> Result<Outcome, ToolError> {
    let root = repo_root()?;
    let allow = match std::fs::read_to_string(root.join(ALLOW_FILE)) {
        Ok(text) => parse_allow(&text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(ToolError(format!("{ALLOW_FILE}: {err}"))),
    };
    let mut violations = Vec::new();
    for rel in tracked_files(&root)? {
        let Some(limit) = limit_for(&rel) else {
            continue;
        };
        if is_build_output(&rel) || allow.iter().any(|p| glob_match(p, &rel)) {
            continue;
        }
        let bytes = std::fs::read(root.join(&rel))?;
        let lines = count_lines(&bytes);
        if lines > limit {
            violations.push(format!("{rel}: {lines} lines (limit {limit})"));
        }
    }
    for line in &violations {
        println!("{line}");
    }
    Ok(if violations.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Violations
    })
}

/// Line limit for a repo-relative path, or `None` if the file is not linted.
pub fn limit_for(path: &str) -> Option<usize> {
    let ext = Path::new(path).extension()?.to_str()?;
    LIMITS
        .iter()
        .find(|(exts, _)| exts.contains(&ext))
        .map(|(_, limit)| *limit)
}

/// Number of lines, counting a final line without a trailing newline.
pub fn count_lines(bytes: &[u8]) -> usize {
    let newlines = bytes.iter().filter(|b| **b == b'\n').count();
    match bytes.last() {
        None | Some(b'\n') => newlines,
        Some(_) => newlines + 1,
    }
}

/// Parses the allowlist: one pattern per line, `#` comments and blanks skipped.
pub fn parse_allow(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Matches a `/`-separated path against a pattern where `*` matches any run
/// of characters within one segment and a `**` segment matches any number
/// of whole segments (including none).
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let segs: Vec<&str> = path.split('/').collect();
    match_segments(&pat, &segs)
}

fn match_segments(pat: &[&str], segs: &[&str]) -> bool {
    match pat.split_first() {
        None => segs.is_empty(),
        Some((&"**", rest)) => (0..=segs.len()).any(|skip| match_segments(rest, &segs[skip..])),
        Some((first, rest)) => match segs.split_first() {
            Some((seg, seg_rest)) => match_segment(first, seg) && match_segments(rest, seg_rest),
            None => false,
        },
    }
}

fn match_segment(pat: &str, seg: &str) -> bool {
    match pat.split_once('*') {
        None => pat == seg,
        Some((head, tail)) => seg.strip_prefix(head).is_some_and(|rest| {
            (0..=rest.len()).any(|i| rest.is_char_boundary(i) && match_segment(tail, &rest[i..]))
        }),
    }
}

fn is_build_output(rel: &str) -> bool {
    rel.split('/')
        .next()
        .is_some_and(|first| BUILD_DIRS.contains(&first))
}

/// Resolves the repository root the way git does, from the crate's own
/// directory. Asking git (rather than probing for a `.git` entry) keeps the
/// lint correct in worktrees, in CI containers and wherever `safe.directory`
/// rules apply, and surfaces git's own message when it refuses.
pub(crate) fn repo_root() -> Result<PathBuf, ToolError> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let start = if manifest_dir.is_dir() {
        manifest_dir.clone()
    } else {
        std::env::current_dir()?
    };
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&start)
        .output()?;
    if !output.status.success() {
        return Err(ToolError(format!(
            "git rev-parse --show-toplevel failed from {}: {}",
            start.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err(ToolError(format!(
            "git reported no repository root above {}",
            start.display()
        )));
    }
    Ok(PathBuf::from(root))
}

fn tracked_files(root: &Path) -> Result<Vec<String>, ToolError> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(ToolError(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_by_extension() {
        let cases = [
            ("crates/a/src/lib.rs", Some(500)),
            ("qt/src/main.cpp", Some(500)),
            ("qt/src/a.cc", Some(500)),
            ("qt/src/a.cxx", Some(500)),
            ("qt/src/a.h", Some(500)),
            ("qt/src/a.hpp", Some(500)),
            ("qt/src/a.hh", Some(500)),
            ("qt/qml/Main.qml", Some(300)),
            ("README.md", None),
            ("Cargo.toml", None),
            ("noext", None),
        ];
        for (path, expected) in cases {
            assert_eq!(limit_for(path), expected, "{path}");
        }
    }

    #[test]
    fn line_counting() {
        let cases: &[(&[u8], usize)] = &[
            (b"", 0),
            (b"a", 1),
            (b"a\n", 1),
            (b"a\nb", 2),
            (b"a\nb\n", 2),
            (b"\n\n", 2),
        ];
        for (bytes, expected) in cases {
            assert_eq!(count_lines(bytes), *expected, "{bytes:?}");
        }
    }

    #[test]
    fn allow_parsing_skips_comments_and_blanks() {
        let text = "# header\n\n  crates/a/src/gen.rs  \n#x\nqt/**/*.qml\n";
        assert_eq!(
            parse_allow(text),
            vec!["crates/a/src/gen.rs", "qt/**/*.qml"]
        );
    }

    #[test]
    fn glob_table() {
        let cases = [
            ("crates/a/src/gen.rs", "crates/a/src/gen.rs", true),
            ("crates/a/src/gen.rs", "crates/a/src/gen.rs.bak", false),
            ("crates/a/src/*.rs", "crates/a/src/gen.rs", true),
            ("crates/a/src/*.rs", "crates/a/src/sub/gen.rs", false),
            ("crates/*/src/gen.rs", "crates/b/src/gen.rs", true),
            ("crates/gen_*.rs", "crates/gen_types.rs", true),
            ("crates/gen_*.rs", "crates/types.rs", false),
            ("**/*.qml", "qt/qml/Main.qml", true),
            ("**/*.qml", "Main.qml", true),
            ("qt/**/*.qml", "qt/Main.qml", true),
            ("qt/**/*.qml", "qt/a/b/Main.qml", true),
            ("qt/**/*.qml", "src/Main.qml", false),
            ("qt/**", "qt/a/b/c.cpp", true),
            ("qt/**", "qtx/a.cpp", false),
            ("a*b*c", "abc", true),
            ("a*b*c", "axxbyyc", true),
            ("a*b*c", "axxbyy", false),
            ("*", "file.rs", true),
            ("*", "dir/file.rs", false),
        ];
        for (pattern, path, expected) in cases {
            assert_eq!(glob_match(pattern, path), expected, "{pattern} vs {path}");
        }
    }

    #[test]
    fn build_output_is_skipped() {
        assert!(is_build_output("target/debug/build/x/out/gen.rs"));
        assert!(!is_build_output("crates/target/src/lib.rs"));
    }
}

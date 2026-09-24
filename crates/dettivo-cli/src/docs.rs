//! `dettivo docs cli-tree`: the command tree rendered from clap's own
//! command graph, so the agent guide's tree can never drift from the
//! binary. Every verb whose path the delta register lists as a Linux
//! addition (the table between the `cli-additions` markers in
//! `docs/api/linux-deltas.md`) carries a `*`, the way Appendix E of the
//! product plan marks them; `--json` prints the same tree as data.

use std::path::{Path, PathBuf};

use clap::{Command, CommandFactory, Subcommand};
use serde::Serialize;

use crate::Cli;
use crate::exit::{Exit, Failure};

/// The register the marks come from, relative to the repository root.
pub const REGISTER: &str = "docs/api/linux-deltas.md";
const BEGIN: &str = "<!-- cli-additions:start -->";
const END: &str = "<!-- cli-additions:end -->";

/// `dettivo docs <what>`.
#[derive(Debug, Subcommand)]
pub enum DocsCmd {
    /// Print the command tree with every Linux addition marked `*`.
    CliTree {
        /// The delta register that lists the Linux additions (default: docs/api/linux-deltas.md under the repository root found from the current directory).
        #[arg(long, value_name = "PATH")]
        register: Option<PathBuf>,
    },
}

/// One node of the tree.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Node {
    /// The verb.
    pub name: String,
    /// The full path, `dettivo <noun> <verb>`.
    pub path: String,
    /// True when the register lists the path (or a parent) as a Linux addition.
    pub linux_addition: bool,
    /// The first sentence of the verb's help.
    pub about: String,
    /// Positional arguments, as `<name>` or `[name]`.
    pub positionals: Vec<String>,
    /// Long options, as `--name`.
    pub options: Vec<String>,
    /// The subcommands, in declaration order.
    pub children: Vec<Node>,
}

/// The verb paths the register lists between its markers, one per row,
/// as `dettivo <noun> [<verb>]`.
pub fn additions(register: &str) -> Result<Vec<String>, String> {
    let start = register
        .find(BEGIN)
        .ok_or_else(|| format!("missing the {BEGIN} marker"))?;
    let end = register
        .find(END)
        .ok_or_else(|| format!("missing the {END} marker"))?;
    if end < start {
        return Err(format!("{END} comes before {BEGIN}"));
    }
    let mut rows = Vec::new();
    for line in register[start + BEGIN.len()..end].lines() {
        let cells: Vec<&str> = line
            .trim()
            .strip_prefix('|')
            .and_then(|l| l.strip_suffix('|'))
            .map(|l| l.split('|').map(str::trim).collect())
            .unwrap_or_default();
        if cells.len() < 2 || cells[0] == "Verb" || cells[0].starts_with("---") {
            continue;
        }
        let verb = cells[0].trim_matches('`').trim();
        if !verb.starts_with("dettivo") {
            return Err(format!("row {verb:?} does not start with `dettivo`"));
        }
        rows.push(verb.to_string());
    }
    Ok(rows)
}

fn first_sentence(text: &str) -> String {
    let text = text.lines().next().unwrap_or("").trim();
    match text.find(". ") {
        Some(i) => text[..=i].to_string(),
        None => text.to_string(),
    }
}

/// Builds the tree for `command` under `path`; `marked` are the register's rows.
pub fn node(command: &Command, path: &str, marked: &[String], inherited: bool) -> Node {
    let addition = inherited || marked.iter().any(|m| m == path);
    let mut positionals = Vec::new();
    let mut options = Vec::new();
    for arg in command.get_arguments() {
        if arg.is_global_set() || matches!(arg.get_id().as_str(), "help" | "version") {
            continue;
        }
        if arg.is_positional() {
            let name = arg.get_id().as_str().replace('_', "-");
            positionals.push(if arg.is_required_set() {
                format!("<{name}>")
            } else {
                format!("[{name}]")
            });
        } else if let Some(long) = arg.get_long() {
            options.push(format!("--{long}"));
        }
    }
    let children = command
        .get_subcommands()
        .map(|sub| {
            let child_path = format!("{path} {}", sub.get_name());
            node(sub, &child_path, marked, addition)
        })
        .collect();
    Node {
        name: command.get_name().to_string(),
        path: path.to_string(),
        linux_addition: addition,
        about: first_sentence(
            &command
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_default(),
        ),
        positionals,
        options,
        children,
    }
}

fn line(node: &Node, out: &mut String) {
    let mut head = node.name.clone();
    if node.linux_addition {
        head.push('*');
    }
    let mut args = node.positionals.clone();
    args.extend(node.options.iter().cloned());
    if !args.is_empty() {
        head.push(' ');
        head.push_str(&args.join(" "));
    }
    if node.about.is_empty() {
        out.push_str(&head);
    } else {
        out.push_str(&format!("{head}  {}", node.about));
    }
    out.push('\n');
}

fn render_children(node: &Node, prefix: &str, out: &mut String) {
    let last = node.children.len().saturating_sub(1);
    for (i, child) in node.children.iter().enumerate() {
        out.push_str(prefix);
        out.push_str(if i == last { "└─ " } else { "├─ " });
        line(child, out);
        let next = format!("{prefix}{}", if i == last { "   " } else { "│  " });
        render_children(child, &next, out);
    }
}

/// The text rendering: the tree, then the global options and the exit codes.
pub fn render(root: &Node, globals: &[String]) -> String {
    let mut out = String::new();
    line(root, &mut out);
    render_children(root, "", &mut out);
    out.push_str(&format!("\nglobal: {}\n", globals.join(" ")));
    out.push_str("env:    DETTIVO_IPC_SOCKET DETTIVO_IPC_TOKEN\n");
    out.push_str(
        "exit:   0 ok, 1 runtime, 2 daemon unavailable, 3 permission, 4 args, 5 timeout\n",
    );
    out.push_str("*       a Linux addition recorded in docs/api/linux-deltas.md\n");
    out
}

/// The global options of the root command, as `--name`.
pub fn globals(command: &Command) -> Vec<String> {
    command
        .get_arguments()
        .filter(|a| a.is_global_set())
        .filter_map(|a| a.get_long().map(|l| format!("--{l}")))
        .collect()
}

/// The register path: `--register`, else `docs/api/linux-deltas.md` under
/// the first ancestor of the current directory that holds it.
fn register_path(flag: Option<&Path>) -> Result<PathBuf, Failure> {
    if let Some(p) = flag {
        return Ok(p.to_path_buf());
    }
    let mut dir =
        std::env::current_dir().map_err(|e| Failure::new(Exit::Failure, e.to_string()))?;
    loop {
        let candidate = dir.join(REGISTER);
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !dir.pop() {
            return Err(Failure::new(
                Exit::InvalidArgs,
                format!("{REGISTER} not found above the current directory; pass --register <path>"),
            ));
        }
    }
}

/// Runs `dettivo docs <what>`; no daemon is touched.
pub fn run(cli: &Cli, what: &DocsCmd) -> Result<(), Failure> {
    match what {
        DocsCmd::CliTree { register } => {
            let path = register_path(register.as_deref())?;
            let text = std::fs::read_to_string(&path)
                .map_err(|e| Failure::new(Exit::InvalidArgs, format!("{}: {e}", path.display())))?;
            let marked = additions(&text)
                .map_err(|e| Failure::new(Exit::Failure, format!("{}: {e}", path.display())))?;
            let command = Cli::command();
            let root = node(&command, "dettivo", &marked, false);
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "tree": root,
                        "global": globals(&command),
                        "register": path.display().to_string(),
                    }))
                    .unwrap_or_default()
                );
            } else {
                print!("{}", render(&root, &globals(&command)));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTER_TEXT: &str = "intro\n<!-- cli-additions:start -->\n| Verb | Serves | Reason |\n|---|---|---|\n| `dettivo docs` | this binary | the tree |\n| `dettivo dictation toggle` | dictation.status then start or stop | a binding |\n<!-- cli-additions:end -->\n";

    #[test]
    fn the_register_rows_mark_the_verb_and_everything_beneath_it() {
        let marked = additions(REGISTER_TEXT).unwrap();
        assert_eq!(marked, ["dettivo docs", "dettivo dictation toggle"]);
        let root = node(&Cli::command(), "dettivo", &marked, false);
        let docs = root.children.iter().find(|c| c.name == "docs").unwrap();
        assert!(docs.linux_addition);
        assert!(docs.children.iter().all(|c| c.linux_addition));
        let dictation = root
            .children
            .iter()
            .find(|c| c.name == "dictation")
            .unwrap();
        assert!(!dictation.linux_addition);
        let toggle = dictation
            .children
            .iter()
            .find(|c| c.name == "toggle")
            .unwrap();
        assert!(toggle.linux_addition);
        let start = dictation
            .children
            .iter()
            .find(|c| c.name == "start")
            .unwrap();
        assert!(!start.linux_addition);
        assert!(start.options.contains(&"--language".to_string()));
        let text = render(&root, &globals(&Cli::command()));
        assert!(text.starts_with("dettivo  "), "{text}");
        assert!(text.contains("├─ docs*  "), "{text}");
        assert!(text.contains("│  ├─ toggle*  "), "{text}");
        assert!(
            text.contains("global: --socket --token --token-file --timeout-ms --json --quiet"),
            "{text}"
        );
        assert!(additions("no markers").is_err());
    }
}

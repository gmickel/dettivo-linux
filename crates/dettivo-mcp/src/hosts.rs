//! `dettivo mcp config`: the server entry each host expects, rendered to
//! print or merged into the host's file with every other server kept.
//! Claude Desktop and Cursor take the `mcpServers` JSON object, Claude
//! Code takes a `claude mcp add` line (or a project `.mcp.json`), Codex
//! takes an `[mcp_servers.<name>]` TOML table. `--hardened` adds the
//! `DETTIVO_IPC_TOKEN` placeholder the macOS helper adds.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// The host names, in the order the usage line shows them.
pub const HOSTS: &[&str] = &["claude-desktop", "claude-code", "cursor", "codex"];

/// A supported MCP host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// `~/.config/Claude/claude_desktop_config.json`.
    ClaudeDesktop,
    /// `claude mcp add`, or `.mcp.json` in the project.
    ClaudeCode,
    /// `~/.cursor/mcp.json`.
    Cursor,
    /// `$CODEX_HOME/config.toml`, else `~/.codex/config.toml`.
    Codex,
}

impl Host {
    /// The host for a name; the error names every host.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "claude-desktop" => Ok(Self::ClaudeDesktop),
            "claude-code" => Ok(Self::ClaudeCode),
            "cursor" => Ok(Self::Cursor),
            "codex" => Ok(Self::Codex),
            other => Err(format!("{other} is not a host ({})", HOSTS.join(", "))),
        }
    }

    /// The name on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeDesktop => "claude-desktop",
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
        }
    }

    /// The file `--write` merges into. `cwd` is the project directory
    /// for Claude Code's `.mcp.json`.
    pub fn config_path(self, env: impl Fn(&str) -> Option<OsString>, cwd: &Path) -> PathBuf {
        let home = env("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        match self {
            Self::ClaudeDesktop => {
                let config_home = match env("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
                    Some(v) => PathBuf::from(v),
                    None => home.join(".config"),
                };
                config_home
                    .join("Claude")
                    .join("claude_desktop_config.json")
            }
            Self::ClaudeCode => cwd.join(".mcp.json"),
            Self::Cursor => home.join(".cursor").join("mcp.json"),
            Self::Codex => match env("CODEX_HOME").filter(|v| !v.is_empty()) {
                Some(v) => PathBuf::from(v).join("config.toml"),
                None => home.join(".codex").join("config.toml"),
            },
        }
    }
}

/// What to render.
#[derive(Debug, Clone)]
pub struct Options {
    /// The host.
    pub host: Host,
    /// The server name inside the host's file (`dettivo`).
    pub name: String,
    /// The `dettivo` command path.
    pub command: String,
    /// A socket override, written as `DETTIVO_IPC_SOCKET`.
    pub socket: Option<String>,
    /// Add the `DETTIVO_IPC_TOKEN` placeholder.
    pub hardened: bool,
}

/// The placeholder a hardened entry carries until the user sets the token.
pub const TOKEN_PLACEHOLDER: &str = "<set-me>";

/// The server entry: `{type, command, args[, env]}`.
pub fn server_entry(opts: &Options) -> Value {
    let mut env = Map::new();
    if let Some(socket) = opts.socket.as_deref().filter(|s| !s.is_empty()) {
        env.insert("DETTIVO_IPC_SOCKET".into(), json!(socket));
    }
    if opts.hardened {
        env.insert("DETTIVO_IPC_TOKEN".into(), json!(TOKEN_PLACEHOLDER));
    }
    let mut server = json!({"type": "stdio", "command": opts.command, "args": ["mcp", "serve"]});
    if !env.is_empty() {
        server["env"] = Value::Object(env);
    }
    server
}

/// The text `dettivo mcp config` prints for `opts`.
pub fn render(opts: &Options) -> Result<String, String> {
    let server = server_entry(opts);
    match opts.host {
        Host::ClaudeDesktop | Host::Cursor => merged_json("", &opts.name, &server),
        Host::ClaudeCode => Ok(claude_code_line(opts, &server)),
        Host::Codex => merged_codex("", &opts.name, &server),
    }
}

fn claude_code_line(opts: &Options, server: &Value) -> String {
    let mut line = format!(
        "claude mcp add --scope user --transport stdio {}",
        sh_quote(&opts.name)
    );
    if let Some(env) = server["env"].as_object() {
        for (k, v) in env {
            line.push_str(&format!(
                " --env {}",
                sh_quote(&format!("{k}={}", v.as_str().unwrap_or("")))
            ));
        }
    }
    line.push_str(&format!(" -- {} mcp serve\n", sh_quote(&opts.command)));
    line
}

/// A word a POSIX shell reads back verbatim: bare when every byte is
/// safe, single-quoted otherwise (a quote inside becomes `'\''`).
fn sh_quote(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-./=:@%+,".contains(&b));
    if safe {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The host file after merging `server` under `name`, other servers kept.
pub fn merged(host: Host, existing: &str, name: &str, server: &Value) -> Result<String, String> {
    match host {
        Host::Codex => merged_codex(existing, name, server),
        _ => merged_json(existing, name, server),
    }
}

fn merged_json(existing: &str, name: &str, server: &Value) -> Result<String, String> {
    let mut root: Value = if existing.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(existing).map_err(|e| format!("existing file is not JSON: {e}"))?
    };
    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| "existing file is not a JSON object".to_string())?;
    let servers = root_obj.entry("mcpServers").or_insert_with(|| json!({}));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| "mcpServers is not an object".to_string())?;
    servers.insert(name.to_string(), server.clone());
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?
    ))
}

fn merged_codex(existing: &str, name: &str, server: &Value) -> Result<String, String> {
    use toml_edit::{Array, DocumentMut, Item, Table, value};
    let mut doc: DocumentMut = existing
        .parse()
        .map_err(|e| format!("existing file is not TOML: {e}"))?;
    let mut entry = Table::new();
    entry["command"] = value(server["command"].as_str().unwrap_or(""));
    let mut args = Array::new();
    for a in server["args"].as_array().into_iter().flatten() {
        args.push(a.as_str().unwrap_or(""));
    }
    entry["args"] = value(args);
    if let Some(env) = server["env"].as_object() {
        let mut env_table = Table::new();
        for (k, v) in env {
            env_table[k] = value(v.as_str().unwrap_or(""));
        }
        entry["env"] = Item::Table(env_table);
    }
    // An inline `mcp_servers = { other = {...} }` is a table too: it is
    // lifted to a regular one so `other` stays; a value of any other kind
    // is refused rather than replaced.
    match doc.get("mcp_servers") {
        None => {
            let mut servers = Table::new();
            servers.set_implicit(true);
            doc["mcp_servers"] = Item::Table(servers);
        }
        Some(item) if item.is_table() => {}
        Some(item) => {
            let inline = item
                .as_inline_table()
                .ok_or_else(|| "mcp_servers is not a table".to_string())?
                .clone();
            doc["mcp_servers"] = Item::Table(inline.into_table());
        }
    }
    let servers = doc["mcp_servers"]
        .as_table_mut()
        .ok_or_else(|| "mcp_servers is not a table".to_string())?;
    servers[name] = Item::Table(entry);
    Ok(doc.to_string())
}

/// The command path: `override`, then `~/.local/bin/dettivo` when it is
/// executable, then the running executable when it sits under a stable
/// prefix, then the running executable as it is.
pub fn resolve_command(override_path: Option<&str>, exe: &Path, home: &Path) -> String {
    if let Some(p) = override_path.filter(|p| !p.is_empty()) {
        return p.to_string();
    }
    let local = home.join(".local/bin/dettivo");
    if is_executable(&local) {
        return local.to_string_lossy().into_owned();
    }
    let text = exe.to_string_lossy().into_owned();
    let stable = [
        home.join(".local/bin/").to_string_lossy().into_owned(),
        "/usr/local/bin/".to_string(),
        "/usr/bin/".to_string(),
    ];
    if stable.iter().any(|p| text.starts_with(p.as_str())) && is_executable(exe) {
        return text;
    }
    text
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Merges `opts` into `path`, creating the directory and the file. Only a
/// file that does not exist counts as empty; a file that cannot be read
/// is never overwritten. The merged text lands through a rename, so an
/// interrupted write leaves the file as it was.
pub fn write(opts: &Options, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let text = merged(opts.host, &existing, &opts.name, &server_entry(opts))?;
    let tmp = path.with_extension("dettivo-tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot replace {}: {e}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(host: Host, hardened: bool) -> Options {
        Options {
            host,
            name: "dettivo".into(),
            command: "/usr/bin/dettivo".into(),
            socket: None,
            hardened,
        }
    }

    #[test]
    fn unknown_hosts_name_the_four_and_paths_follow_the_environment() {
        assert_eq!(
            Host::parse("vscode").unwrap_err(),
            "vscode is not a host (claude-desktop, claude-code, cursor, codex)"
        );
        let env = |k: &str| match k {
            "HOME" => Some(OsString::from("/home/u")),
            "CODEX_HOME" => Some(OsString::from("/cx")),
            _ => None,
        };
        let cwd = Path::new("/proj");
        assert_eq!(
            Host::ClaudeDesktop.config_path(env, cwd),
            PathBuf::from("/home/u/.config/Claude/claude_desktop_config.json")
        );
        assert_eq!(
            Host::ClaudeCode.config_path(env, cwd),
            PathBuf::from("/proj/.mcp.json")
        );
        assert_eq!(
            Host::Cursor.config_path(env, cwd),
            PathBuf::from("/home/u/.cursor/mcp.json")
        );
        assert_eq!(
            Host::Codex.config_path(env, cwd),
            PathBuf::from("/cx/config.toml")
        );
        let plain = |k: &str| (k == "HOME").then(|| OsString::from("/home/u"));
        assert_eq!(
            Host::Codex.config_path(plain, cwd),
            PathBuf::from("/home/u/.codex/config.toml")
        );
    }

    #[test]
    fn the_entry_carries_env_only_when_needed() {
        let plain = server_entry(&opts(Host::Cursor, false));
        assert_eq!(
            plain,
            json!({"type": "stdio", "command": "/usr/bin/dettivo", "args": ["mcp", "serve"]})
        );
        let mut o = opts(Host::Cursor, true);
        o.socket = Some("/run/x.sock".into());
        let hardened = server_entry(&o);
        assert_eq!(hardened["env"]["DETTIVO_IPC_TOKEN"], TOKEN_PLACEHOLDER);
        assert_eq!(hardened["env"]["DETTIVO_IPC_SOCKET"], "/run/x.sock");
        o.host = Host::ClaudeCode;
        let line = render(&o).unwrap();
        assert_eq!(
            line,
            "claude mcp add --scope user --transport stdio dettivo --env DETTIVO_IPC_SOCKET=/run/x.sock --env 'DETTIVO_IPC_TOKEN=<set-me>' -- /usr/bin/dettivo mcp serve\n"
        );
    }

    #[test]
    fn merging_keeps_other_servers_in_json_and_toml() {
        let existing = "{\"mcpServers\": {\"other\": {\"command\": \"x\"}}, \"theme\": \"dark\"}";
        let out = merged(
            Host::Cursor,
            existing,
            "dettivo",
            &server_entry(&opts(Host::Cursor, false)),
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        assert_eq!(v["mcpServers"]["dettivo"]["command"], "/usr/bin/dettivo");
        assert_eq!(v["theme"], "dark");
        assert!(merged(Host::Cursor, "not json", "dettivo", &json!({})).is_err());

        let existing = "model = \"o3\"\n\n[mcp_servers.other]\ncommand = \"x\"\n\n[mcp_servers.dettivo]\ncommand = \"old\"\nargs = []\n";
        let out = merged(
            Host::Codex,
            existing,
            "dettivo",
            &server_entry(&opts(Host::Codex, true)),
        )
        .unwrap();
        assert!(out.starts_with("model = \"o3\"\n"), "{out}");
        assert!(
            out.contains("[mcp_servers.other]\ncommand = \"x\"\n"),
            "{out}"
        );
        assert!(out.contains("[mcp_servers.dettivo]\ncommand = \"/usr/bin/dettivo\"\nargs = [\"mcp\", \"serve\"]\n"), "{out}");
        assert!(
            out.contains("[mcp_servers.dettivo.env]\nDETTIVO_IPC_TOKEN = \"<set-me>\"\n"),
            "{out}"
        );
        assert!(!out.contains("old"), "{out}");
    }

    #[test]
    fn an_inline_servers_table_keeps_its_other_servers_and_odd_values_are_refused() {
        let existing = "model = \"o3\"\nmcp_servers = { other = { command = \"x\", args = [] } }\n";
        let out = merged(
            Host::Codex,
            existing,
            "dettivo",
            &server_entry(&opts(Host::Codex, false)),
        )
        .unwrap();
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x"));
        assert_eq!(
            doc["mcp_servers"]["dettivo"]["command"].as_str(),
            Some("/usr/bin/dettivo")
        );
        assert_eq!(doc["model"].as_str(), Some("o3"));
        let err = merged(
            Host::Codex,
            "mcp_servers = 3\n",
            "dettivo",
            &server_entry(&opts(Host::Codex, false)),
        )
        .unwrap_err();
        assert_eq!(err, "mcp_servers is not a table");
    }

    #[test]
    fn a_file_that_cannot_be_read_is_never_overwritten_and_the_merge_lands_by_rename() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("codex/config.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let existing = "[mcp_servers.other]\ncommand = \"x\"\n";
        std::fs::write(&path, existing).unwrap();
        std::fs::set_permissions(&path, PermissionsExt::from_mode(0o000)).unwrap();
        let err = write(&opts(Host::Codex, false), &path).unwrap_err();
        assert!(err.starts_with("cannot read"), "{err}");
        std::fs::set_permissions(&path, PermissionsExt::from_mode(0o600)).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), existing);

        write(&opts(Host::Codex, false), &path).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert!(
            out.contains("[mcp_servers.other]\ncommand = \"x\"\n"),
            "{out}"
        );
        assert!(out.contains("[mcp_servers.dettivo]"), "{out}");
        assert!(!path.with_extension("dettivo-tmp").exists());
        // A missing file is still created.
        let fresh = dir.path().join("cursor/mcp.json");
        write(&opts(Host::Cursor, false), &fresh).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&fresh).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["dettivo"]["command"], "/usr/bin/dettivo");
    }

    #[test]
    fn command_resolution_prefers_local_bin_then_stable_prefixes() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let exe = Path::new("/opt/build/target/debug/dettivo");
        assert_eq!(resolve_command(Some("/x/dettivo"), exe, home), "/x/dettivo");
        assert_eq!(
            resolve_command(None, exe, home),
            "/opt/build/target/debug/dettivo"
        );
        std::fs::create_dir_all(home.join(".local/bin")).unwrap();
        let local = home.join(".local/bin/dettivo");
        std::fs::write(&local, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&local, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        assert_eq!(resolve_command(None, exe, home), local.to_string_lossy());
    }

    #[test]
    fn shell_words_are_bare_when_safe_and_single_quoted_otherwise() {
        assert_eq!(sh_quote("dettivo"), "dettivo");
        assert_eq!(sh_quote("/usr/bin/dettivo"), "/usr/bin/dettivo");
        assert_eq!(
            sh_quote("DETTIVO_IPC_TOKEN=<set-me>"),
            "'DETTIVO_IPC_TOKEN=<set-me>'"
        );
        assert_eq!(sh_quote("my server"), "'my server'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(sh_quote(""), "''");
    }
}

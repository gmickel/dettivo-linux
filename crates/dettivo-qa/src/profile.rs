//! Isolated profiles (R3): every scenario gets its own HOME and XDG tree
//! under one short root, a private models directory with only the named
//! test models hard-linked in (`profile_models`, never a link to the
//! user's directory), the QA environment set, and a leak check at the end
//! that names what was left behind: a process the scenario started that
//! is still alive, or a socket under the root's runtime directory.

use std::collections::BTreeMap;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub use crate::profile_models::{ModelSource, ModelTree, TEST_MODELS};

/// One scenario's private tree.
#[derive(Debug)]
pub struct Profile {
    /// The root; everything the scenario writes lands beneath it.
    pub root: PathBuf,
    /// Processes the scenario started, checked at the end.
    children: Vec<(String, u32)>,
    keep: bool,
    /// Variables added to every process's environment (`extend_env`).
    extra_env: BTreeMap<String, String>,
    /// The private models tree `data/dettivo/models` points at.
    models: Option<ModelTree>,
}

/// What a leak check found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Leaks {
    /// `name (pid)` of processes still alive.
    pub processes: Vec<String>,
    /// Sockets left under the runtime directory.
    pub sockets: Vec<PathBuf>,
}

impl Leaks {
    /// True when nothing leaked.
    pub fn is_empty(&self) -> bool {
        self.processes.is_empty() && self.sockets.is_empty()
    }
}

impl std::fmt::Display for Leaks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for p in &self.processes {
            writeln!(f, "process still alive: {p}")?;
        }
        for s in &self.sockets {
            writeln!(f, "socket left behind: {}", s.display())?;
        }
        Ok(())
    }
}

impl Profile {
    /// Creates the tree under `/tmp` (Unix socket paths are short there)
    /// with a private models directory: the test models hard-linked in
    /// from `models` when it names the real directory, empty otherwise.
    pub fn create(scenario: &str, models: Option<&ModelSource>) -> std::io::Result<Self> {
        let short: String = scenario
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(8)
            .collect();
        let root = tempfile::Builder::new()
            .prefix(&format!("dq{short}-"))
            .tempdir_in("/tmp")?
            .keep();
        if let Err(why) = guard(&root, |k| std::env::var_os(k)) {
            let _ = std::fs::remove_dir_all(&root);
            return Err(std::io::Error::other(why));
        }
        for sub in ["cfg", "state", "data", "cache", "tmp", "run", "home"] {
            std::fs::create_dir_all(root.join(sub))?;
        }
        // The rig drives the daemon through the CLI and the socket; the
        // desktop's portal must never bind shortcuts for a scenario's
        // daemon (a dialog on KDE, a refusal elsewhere). A scenario that
        // wants a backend writes its own configuration.
        std::fs::create_dir_all(root.join("cfg/dettivo"))?;
        std::fs::write(
            root.join("cfg/dettivo/config.toml"),
            "[hotkeys]\nbackend = \"none\"\n",
        )?;
        let tree = match ModelTree::create(&root, models.filter(|m| m.real.is_dir())) {
            Ok(tree) => tree,
            Err(why) => {
                let _ = std::fs::remove_dir_all(&root);
                return Err(why);
            }
        };
        Ok(Self {
            root,
            children: Vec::new(),
            keep: false,
            extra_env: BTreeMap::new(),
            models: Some(tree),
        })
    }

    /// Links one more model by name (`diarize/diarization`,
    /// `whisper/large-v3-turbo`, `llm/qwen3-1.7b`) into the profile's
    /// private tree before the daemon starts; a model the real directory
    /// does not carry is refused by name with the command that fetches it.
    pub fn link_model(&mut self, name: &str) -> Result<PathBuf, String> {
        self.models
            .as_mut()
            .ok_or_else(|| format!("the profile has no models tree to link `{name}` into"))?
            .link(name)
    }

    /// The profile's models directory as the daemon sees it.
    pub fn models_dir(&self) -> PathBuf {
        self.root.join("data/dettivo/models")
    }

    /// Keeps the tree after the run (for inspection).
    pub fn keep(&mut self) {
        self.keep = true;
    }

    /// The socket the scenario's daemon listens on.
    pub fn socket(&self) -> PathBuf {
        self.root.join("run/dettivo.sock")
    }

    /// The environment every process in the scenario runs with: isolated
    /// locations, QA mode with mock mode and mock insertion on (no real
    /// microphone, no real paste), the X11 platform and accessibility on.
    /// `XDG_RUNTIME_DIR` is inherited on purpose: the session bus, the
    /// accessibility bus, the display and PipeWire all live there, and the
    /// daemon socket is isolated through `DETTIVO_IPC_SOCKET` instead.
    pub fn env(&self) -> BTreeMap<String, String> {
        let r = |s: &str| self.root.join(s).to_string_lossy().into_owned();
        let mut env = BTreeMap::new();
        env.insert("HOME".into(), r("home"));
        env.insert("XDG_CONFIG_HOME".into(), r("cfg"));
        env.insert("XDG_STATE_HOME".into(), r("state"));
        env.insert("XDG_DATA_HOME".into(), r("data"));
        env.insert("XDG_CACHE_HOME".into(), r("cache"));
        env.insert("TMPDIR".into(), r("tmp"));
        // Host-specific overrides otherwise escape the isolated HOME when
        // the Agents page reads or writes an MCP client configuration.
        env.insert("CODEX_HOME".into(), r("home/.codex"));
        env.insert("CLAUDE_CONFIG_DIR".into(), r("home/.claude"));
        env.insert(
            "DETTIVO_IPC_SOCKET".into(),
            self.socket().to_string_lossy().into_owned(),
        );
        env.insert("DETTIVO_QA_MODE".into(), "1".into());
        env.insert("DETTIVO_QA_ALLOW_RELEASE".into(), "1".into());
        env.insert("DETTIVO_MOCK_MODE".into(), "1".into());
        env.insert("DETTIVO_MOCK_INSERT".into(), "1".into());
        env.insert("QT_QPA_PLATFORM".into(), "xcb".into());
        env.insert("QT_LINUX_ACCESSIBILITY_ALWAYS_ON".into(), "1".into());
        env.extend(self.extra_env.clone());
        env
    }

    /// Variables every process of the scenario also gets (a pack's
    /// `--cpu` sets `DETTIVO_FORCE_CPU=1` for every daemon it drives).
    pub fn extend_env(&mut self, extra: &BTreeMap<String, String>) {
        self.extra_env.extend(extra.clone());
    }

    /// Records a process the scenario started.
    pub fn track(&mut self, name: &str, child: &Child) {
        self.children.push((name.to_string(), child.id()));
    }

    /// Records a pid a driver started on the scenario's behalf.
    pub fn track_pid(&mut self, name: &str, pid: u32) {
        self.children.push((name.to_string(), pid));
    }

    /// The leak check: tracked processes still alive, sockets under `run`.
    pub fn leaks(&self) -> Leaks {
        let mut leaks = Leaks::default();
        for (name, pid) in &self.children {
            if Path::new(&format!("/proc/{pid}")).exists() && !is_zombie(*pid) {
                leaks.processes.push(format!("{name} ({pid})"));
            }
        }
        for path in walk(&self.root.join("run")) {
            if std::fs::symlink_metadata(&path)
                .map(|m| m.file_type().is_socket())
                .unwrap_or(false)
            {
                leaks.sockets.push(path);
            }
        }
        leaks
    }
}

/// Builds a scenario process from its complete profile environment. Only
/// desktop connection variables and the locale cross from the caller;
/// the supplied map has final say, including deliberate scenario overrides.
pub fn command(program: impl AsRef<std::ffi::OsStr>, env: &BTreeMap<String, String>) -> Command {
    const SESSION: &[&str] = &[
        "PATH",
        "DISPLAY",
        "XAUTHORITY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "DBUS_SESSION_BUS_ADDRESS",
        "AT_SPI_BUS_ADDRESS",
        "CUA_DRIVER_DELIVERY_MODE",
        "HYPRLAND_INSTANCE_SIGNATURE",
        "PIPEWIRE_REMOTE",
        "PIPEWIRE_RUNTIME_DIR",
        "PULSE_SERVER",
        "PULSE_RUNTIME_PATH",
        "PULSE_COOKIE",
        "LANG",
        "LC_ALL",
        "RUST_BACKTRACE",
    ];
    let mut command = Command::new(program);
    command
        .env_clear()
        .envs(
            SESSION
                .iter()
                .filter_map(|key| std::env::var_os(key).map(|value| (key, value))),
        )
        .envs(env);
    command
}

impl Drop for Profile {
    fn drop(&mut self) {
        if !self.keep {
            if let Some(models) = &self.models {
                models.remove();
            }
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

/// The variables whose directories a drive must never resolve into.
pub const GUARDED: &[&str] = &[
    "HOME",
    "XDG_CONFIG_HOME",
    "XDG_STATE_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
];

/// The daemon configuration every scenario starts from: no daemon hotkey
/// backend, so the desktop's portal never binds shortcuts for a
/// scenario's daemon and no drive registers a real shortcut by accident.
pub const BASELINE: &str = "[hotkeys]\nbackend = \"none\"\n";

/// A scenario's daemon configuration over the profile's baseline: the
/// scenario's own settings first, then `BASELINE` unless the scenario
/// names a `[hotkeys]` table itself. Choosing a real backend is explicit:
/// a scenario writes the table, nothing restores the product's `auto`.
pub fn with_baseline(scenario: &str) -> String {
    if scenario.lines().any(|l| l.trim() == "[hotkeys]") {
        return scenario.to_string();
    }
    let mut text = scenario.to_string();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(BASELINE);
    text
}

/// The profile preflight (fn-35 R1): a scenario root that resolves under
/// the caller's home or a real XDG directory is refused by the variable's
/// name, so a first-run drive never sees the developer's snippet, models
/// or state. `env` answers the caller's environment.
pub fn guard(root: &Path, env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Result<(), String> {
    let resolved = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    for var in GUARDED {
        let Some(value) = env(var).filter(|v| !v.is_empty()) else {
            continue;
        };
        let dir = PathBuf::from(&value);
        let dir_resolved = std::fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
        if resolved.starts_with(&dir_resolved) || root.starts_with(&dir) {
            return Err(format!(
                "the scenario root {} is under {var}={}; a drive never links a real user directory",
                root.display(),
                dir.display()
            ));
        }
    }
    Ok(())
}

fn is_zombie(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .map(|s| {
            s.lines()
                .any(|l| l.starts_with("State:") && l.contains('Z'))
        })
        .unwrap_or(true)
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// qa-rig/F4 (fn-43): a scenario configuration without a `[hotkeys]`
    /// table parses with the profile's `none` backend, and a scenario's
    /// own table stays as written.
    #[test]
    fn the_baseline_keeps_the_hotkey_backend_off_unless_the_scenario_names_one() {
        let plain = "[engines]\ndirectory = \"/b\"\n[speech]\nmodel = \"tiny.en\"\n";
        let parsed: toml::Value = toml::from_str(&with_baseline(plain)).unwrap();
        assert_eq!(parsed["hotkeys"]["backend"].as_str(), Some("none"));
        assert_eq!(parsed["speech"]["model"].as_str(), Some("tiny.en"));
        let own = "[hotkeys]\nbackend = \"none\"\nhold = \"F7\"\n";
        assert_eq!(with_baseline(own), own);
        let real = "[speech]\nmodel = \"tiny.en\"\n[hotkeys]\nbackend = \"evdev\"\n";
        let parsed: toml::Value = toml::from_str(&with_baseline(real)).unwrap();
        assert_eq!(parsed["hotkeys"]["backend"].as_str(), Some("evdev"));
        let parsed: toml::Value = toml::from_str(&with_baseline("")).unwrap();
        assert_eq!(parsed["hotkeys"]["backend"].as_str(), Some("none"));
    }

    #[test]
    fn profile_isolates_locations_and_detects_leaks() {
        let mut profile = Profile::create("placeholder_window", None).unwrap();
        let env = profile.env();
        assert!(env["HOME"].starts_with(profile.root.to_str().unwrap()));
        assert!(env["CODEX_HOME"].starts_with(profile.root.to_str().unwrap()));
        assert!(env["CLAUDE_CONFIG_DIR"].starts_with(profile.root.to_str().unwrap()));
        assert!(!env.contains_key("XDG_RUNTIME_DIR"));
        assert_eq!(env["DETTIVO_QA_MODE"], "1");
        assert_eq!(env["QT_QPA_PLATFORM"], "xcb");
        assert!(profile.leaks().is_empty());

        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        profile.track("sleep", &child);
        let run = profile.root.join("run");
        std::fs::create_dir_all(&run).unwrap();
        let _listener = std::os::unix::net::UnixListener::bind(run.join("stale.sock")).unwrap();
        let leaks = profile.leaks();
        assert_eq!(leaks.processes.len(), 1);
        assert!(leaks.processes[0].starts_with("sleep ("));
        assert_eq!(leaks.sockets.len(), 1);
        let _ = child.kill();
        let _ = child.wait();
        assert!(profile.leaks().processes.is_empty());
        let root = profile.root.clone();
        drop(profile);
        assert!(!root.exists(), "tree removed on drop");
    }

    #[test]
    fn a_profile_carries_a_private_models_tree_and_links_by_name() {
        let real = tempfile::Builder::new()
            .prefix("dq-real-")
            .tempdir_in("/tmp")
            .unwrap();
        let store = tempfile::Builder::new()
            .prefix("dq-store-")
            .tempdir_in("/tmp")
            .unwrap();
        for file in [
            "whisper/tiny.en/ggml-tiny.en.bin",
            "fixtures/jfk.wav",
            "diarize/diarization/segmentation.onnx",
        ] {
            let path = real.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"x").unwrap();
        }
        let source = ModelSource {
            real: real.path().to_path_buf(),
            store: store.path().to_path_buf(),
        };
        let mut profile = Profile::create("history_seeded", Some(&source)).unwrap();
        let models = profile.models_dir();
        let resolved = std::fs::canonicalize(&models).unwrap();
        assert!(
            resolved.starts_with(store.path().canonicalize().unwrap()),
            "{resolved:?}"
        );
        assert!(models.join("whisper/tiny.en/ggml-tiny.en.bin").is_file());
        assert!(!models.join("diarize").exists());
        profile.link_model("diarize/diarization").unwrap();
        assert!(
            models
                .join("diarize/diarization/segmentation.onnx")
                .is_file()
        );
        let refused = profile.link_model("llm/qwen3-1.7b").unwrap_err();
        assert!(refused.contains("`llm/qwen3-1.7b`"), "{refused}");
        drop(profile);
        assert_eq!(
            std::fs::read_dir(store.path()).unwrap().count(),
            0,
            "the tree goes with the profile"
        );
        assert!(
            real.path()
                .join("whisper/tiny.en/ggml-tiny.en.bin")
                .is_file()
        );

        // A store inside the real directory never gets a profile.
        let inside = ModelSource {
            real: real.path().to_path_buf(),
            store: real.path().join("qa"),
        };
        let err = Profile::create("history_seeded", Some(&inside)).unwrap_err();
        assert!(err.to_string().contains("real models directory"), "{err}");
    }

    #[test]
    fn a_root_under_the_callers_home_or_xdg_directory_is_refused_by_name() {
        let env = |k: &str| match k {
            "HOME" => Some(std::ffi::OsString::from("/tmp/dq-home")),
            "XDG_DATA_HOME" => Some(std::ffi::OsString::from("/tmp/dq-data/share")),
            _ => None,
        };
        let refused = guard(Path::new("/tmp/dq-home/.cache/x"), env).unwrap_err();
        assert!(
            refused.contains("HOME=/tmp/dq-home") && refused.contains("/tmp/dq-home/.cache/x"),
            "{refused}"
        );
        let refused = guard(Path::new("/tmp/dq-data/share/dettivo"), env).unwrap_err();
        assert!(refused.contains("XDG_DATA_HOME="), "{refused}");
        assert!(guard(Path::new("/tmp/dqplace-abc"), env).is_ok());
        // An unset or empty variable guards nothing.
        assert!(guard(Path::new("/tmp/dq-home/x"), |_| None).is_ok());
    }
}

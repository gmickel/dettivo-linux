//! The binding snippets `dettivo setup <compositor>` writes: press and
//! release bindings for Hyprland (classic `.conf` and the Lua config that
//! Hyprland 0.56 and Omarchy use) and Sway, toggle-only for Niri, which
//! has no release bindings. Every snippet drives the `dettivo` command,
//! so the compositor needs no daemon-side backend.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::Keys;

/// Which compositor a snippet is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compositor {
    /// Hyprland, classic `hyprland.conf`.
    HyprlandConf,
    /// Hyprland with the Lua configuration (`hyprland.lua`, Omarchy).
    HyprlandLua,
    /// Sway (and other `bindsym` compositors).
    Sway,
    /// Niri.
    Niri,
}

/// The names `dettivo setup` accepts.
pub const SUPPORTED: &[&str] = &["hyprland", "sway", "niri"];

/// An unsupported compositor name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCompositor(pub String);

impl fmt::Display for UnknownCompositor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown compositor {:?}; supported: {}",
            self.0,
            SUPPORTED.join(", ")
        )
    }
}

impl std::error::Error for UnknownCompositor {}

impl FromStr for Compositor {
    type Err = UnknownCompositor;

    /// `hyprland` picks the Lua flavour when `hyprland.lua` exists in the
    /// configuration directory and `hyprland.conf` does not; `hyprland-lua`
    /// and `hyprland-conf` name a flavour explicitly.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "hyprland" | "hyprland-conf" => Ok(Self::HyprlandConf),
            "hyprland-lua" => Ok(Self::HyprlandLua),
            "sway" => Ok(Self::Sway),
            "niri" => Ok(Self::Niri),
            _ => Err(UnknownCompositor(s.to_string())),
        }
    }
}

impl Compositor {
    /// The compositor for a name under one configuration home: `hyprland`
    /// becomes the Lua flavour when `hypr/hyprland.lua` exists there
    /// (Hyprland 0.56 and Omarchy), the classic one otherwise;
    /// `hyprland-lua` and `hyprland-conf` name a flavour by hand.
    pub fn resolve(name: &str, config_home: &Path) -> Result<Self, UnknownCompositor> {
        let compositor: Self = name.parse()?;
        Ok(match compositor {
            Self::HyprlandConf
                if name.eq_ignore_ascii_case("hyprland")
                    && config_home.join("hypr/hyprland.lua").is_file() =>
            {
                Self::HyprlandLua
            }
            other => other,
        })
    }

    /// The compositor of a running session from the platform's name
    /// (`Hyprland`, `sway`, `niri`); `None` for a desktop the snippets do
    /// not cover, which binds through the portal instead.
    pub fn detect(platform_compositor: Option<&str>, config_home: &Path) -> Option<Self> {
        let name = platform_compositor?.to_ascii_lowercase();
        Self::resolve(&name, config_home).ok()
    }

    /// The snippet's path under `$XDG_CONFIG_HOME`.
    pub fn snippet_path(self) -> &'static str {
        match self {
            Self::HyprlandConf => "hypr/dettivo.conf",
            Self::HyprlandLua => "hypr/dettivo.lua",
            Self::Sway => "sway/dettivo",
            Self::Niri => "niri/dettivo.kdl",
        }
    }

    /// The user's main configuration under `$XDG_CONFIG_HOME`, the file
    /// the include line belongs in.
    pub fn main_config(self) -> &'static str {
        match self {
            Self::HyprlandConf => "hypr/hyprland.conf",
            Self::HyprlandLua => "hypr/hyprland.lua",
            Self::Sway => "sway/config",
            Self::Niri => "niri/config.kdl",
        }
    }

    /// The line that loads the snippet from the main configuration.
    pub fn include_line(self) -> &'static str {
        match self {
            Self::HyprlandConf => "source = ~/.config/hypr/dettivo.conf",
            Self::HyprlandLua => "dofile(os.getenv(\"HOME\") .. \"/.config/hypr/dettivo.lua\")",
            Self::Sway => "include ~/.config/sway/dettivo",
            Self::Niri => "include \"dettivo.kdl\"",
        }
    }

    /// The name `dettivo setup` prints.
    pub fn name(self) -> &'static str {
        match self {
            Self::HyprlandConf | Self::HyprlandLua => "hyprland",
            Self::Sway => "sway",
            Self::Niri => "niri",
        }
    }

    /// True when a non-comment line of the main configuration loads the
    /// snippet.
    pub fn is_sourced(self, main_config: &str) -> bool {
        let (comment, needle) = match self {
            Self::HyprlandConf => ("#", "dettivo.conf"),
            Self::HyprlandLua => ("--", "dettivo.lua"),
            Self::Sway => ("#", "sway/dettivo"),
            Self::Niri => ("//", "dettivo.kdl"),
        };
        main_config
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with(comment))
            .any(|l| {
                let code = l.split(comment).next().unwrap_or("").trim();
                code.contains(needle) && (self != Self::HyprlandLua || code.starts_with("dofile("))
            })
    }
}

/// A rendered snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// The compositor.
    pub compositor: Compositor,
    /// The file text.
    pub text: String,
    /// What to tell the user beyond the include line.
    pub notes: Vec<String>,
}

const OMARCHY_NOTE: &str = "Omarchy binds F9 and SUPER + CTRL + X to Voxtype when it is installed (default/hypr/bindings/voxtype.lua); the snippet takes those keys over with unbind lines, so load it after Omarchy's defaults.";

/// Renders the snippet for `compositor` with `keys`.
pub fn render(compositor: Compositor, keys: &Keys) -> Snippet {
    let cmd = |verb: &str| format!("dettivo --quiet dictation {verb}");
    let text = match compositor {
        Compositor::HyprlandConf => format!(
            "# Dettivo dictation bindings. Generated by `dettivo setup hyprland`; change the\n\
             # keys in ~/.config/dettivo/config.toml ([hotkeys]) and run it again.\n\
             # Load it from hyprland.conf: {include}\n\
             # Omarchy binds the same keys to Voxtype; unbind takes them over.\n\
             unbind = {hold}\n\
             unbind = {toggle}\n\
             # Hold to talk: press starts, release stops.\n\
             bind = {hold}, exec, {start}\n\
             bindr = {hold}, exec, {stop}\n\
             # Toggle: one press starts, the next stops.\n\
             bind = {toggle}, exec, {toggle_cmd}\n\
             bind = {cancel}, exec, {cancel_cmd}\n\
             bind = {reinsert}, exec, {reinsert_cmd}\n",
            include = compositor.include_line(),
            hold = keys.hold.hyprland_conf(),
            toggle = keys.toggle.hyprland_conf(),
            cancel = keys.cancel.hyprland_conf(),
            reinsert = keys.reinsert.hyprland_conf(),
            start = cmd("start"),
            stop = cmd("stop"),
            toggle_cmd = cmd("toggle"),
            cancel_cmd = cmd("cancel"),
            reinsert_cmd = cmd("reinsert-last"),
        ),
        Compositor::HyprlandLua => format!(
            "-- Dettivo dictation bindings. Generated by `dettivo setup hyprland`; change the\n\
             -- keys in ~/.config/dettivo/config.toml ([hotkeys]) and run it again.\n\
             -- Load it from hyprland.lua after Omarchy's defaults: {include}\n\
             -- Omarchy binds the same keys to Voxtype; unbind takes them over.\n\
             hl.unbind(\"{hold}\")\n\
             hl.unbind(\"{toggle}\")\n\
             -- Hold to talk: press starts, release stops.\n\
             hl.bind(\"{hold}\", hl.dsp.exec_cmd(\"{start}\"), {{ description = \"Dettivo: hold to talk\" }})\n\
             hl.bind(\"{hold}\", hl.dsp.exec_cmd(\"{stop}\"), {{ description = \"Dettivo: hold to talk (release)\", release = true }})\n\
             -- Toggle: one press starts, the next stops.\n\
             hl.bind(\"{toggle}\", hl.dsp.exec_cmd(\"{toggle_cmd}\"), {{ description = \"Dettivo: toggle dictation\" }})\n\
             hl.bind(\"{cancel}\", hl.dsp.exec_cmd(\"{cancel_cmd}\"), {{ description = \"Dettivo: cancel dictation\" }})\n\
             hl.bind(\"{reinsert}\", hl.dsp.exec_cmd(\"{reinsert_cmd}\"), {{ description = \"Dettivo: insert the last transcript again\" }})\n",
            include = compositor.include_line(),
            hold = keys.hold.hyprland_lua(),
            toggle = keys.toggle.hyprland_lua(),
            cancel = keys.cancel.hyprland_lua(),
            reinsert = keys.reinsert.hyprland_lua(),
            start = cmd("start"),
            stop = cmd("stop"),
            toggle_cmd = cmd("toggle"),
            cancel_cmd = cmd("cancel"),
            reinsert_cmd = cmd("reinsert-last"),
        ),
        Compositor::Sway => format!(
            "# Dettivo dictation bindings. Generated by `dettivo setup sway`; change the\n\
             # keys in ~/.config/dettivo/config.toml ([hotkeys]) and run it again.\n\
             # Load it from ~/.config/sway/config: {include}\n\
             # Hold to talk: press starts, release stops.\n\
             bindsym {hold} exec {start}\n\
             bindsym --release {hold} exec {stop}\n\
             # Toggle: one press starts, the next stops.\n\
             bindsym {toggle} exec {toggle_cmd}\n\
             bindsym {cancel} exec {cancel_cmd}\n\
             bindsym {reinsert} exec {reinsert_cmd}\n",
            include = compositor.include_line(),
            hold = keys.hold.sway(),
            toggle = keys.toggle.sway(),
            cancel = keys.cancel.sway(),
            reinsert = keys.reinsert.sway(),
            start = cmd("start"),
            stop = cmd("stop"),
            toggle_cmd = cmd("toggle"),
            cancel_cmd = cmd("cancel"),
            reinsert_cmd = cmd("reinsert-last"),
        ),
        Compositor::Niri => format!(
            "// Dettivo dictation bindings. Generated by `dettivo setup niri`; change the\n\
             // keys in ~/.config/dettivo/config.toml ([hotkeys]) and run it again.\n\
             // Load it from ~/.config/niri/config.kdl: {include}\n\
             // Niri has no release bindings, so hold to talk is absent here: {toggle}\n\
             // toggles instead (one press starts, the next stops). The hold chord\n\
             // ({hold}) still works through the portal backend where a portal exists.\n\
             binds {{\n\
             \x20   {toggle} {{ spawn {toggle_cmd}; }}\n\
             \x20   {cancel} {{ spawn {cancel_cmd}; }}\n\
             \x20   {reinsert} {{ spawn {reinsert_cmd}; }}\n\
             }}\n",
            include = compositor.include_line(),
            hold = keys.hold.niri(),
            toggle = keys.toggle.niri(),
            cancel = keys.cancel.niri(),
            reinsert = keys.reinsert.niri(),
            toggle_cmd = kdl_args("toggle"),
            cancel_cmd = kdl_args("cancel"),
            reinsert_cmd = kdl_args("reinsert-last"),
        ),
    };
    let mut notes = vec![format!(
        "Add this line to ~/.config/{}: {}",
        compositor.main_config(),
        compositor.include_line()
    )];
    match compositor {
        Compositor::HyprlandConf | Compositor::HyprlandLua => {
            notes.push(OMARCHY_NOTE.to_string());
            notes.push(
                "Hyprland fires the release binding when the key of the chord comes up; release the modifiers last."
                    .to_string(),
            );
        }
        Compositor::Sway => notes.push(
            "Sway fires `--release` bindings when the key comes up; release the modifiers last."
                .to_string(),
        ),
        Compositor::Niri => notes.push(format!(
            "Niri has no release bindings: hold to talk ({}) is absent and {} toggles.",
            keys.hold.niri(),
            keys.toggle.niri()
        )),
    }
    Snippet {
        compositor,
        text,
        notes,
    }
}

/// `$XDG_CONFIG_HOME`, or `$HOME/.config`: where the snippets and the
/// main configurations live.
pub fn config_home(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> PathBuf {
    match env("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => env("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"))
            .join(".config"),
    }
}

/// What `dettivo setup --check`, `hotkeys.setup` and `dettivo doctor`
/// report about a compositor's snippet under one configuration home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// The compositor.
    pub compositor: Compositor,
    /// The snippet file.
    pub snippet_path: PathBuf,
    /// True when the snippet file exists.
    pub written: bool,
    /// The user's main configuration file.
    pub main_config: PathBuf,
    /// True when the main configuration exists.
    pub main_exists: bool,
    /// True when a non-comment line of the main configuration loads the
    /// snippet.
    pub sourced: bool,
}

/// Reads the snippet's state under `config_home` (`$XDG_CONFIG_HOME`).
pub fn check(compositor: Compositor, config_home: &Path) -> Check {
    let snippet_path = config_home.join(compositor.snippet_path());
    let main_config = config_home.join(compositor.main_config());
    let sourced = snippet_path.is_file()
        && std::fs::read_to_string(&main_config)
            .map(|text| compositor.is_sourced(&text))
            .unwrap_or(false);
    Check {
        compositor,
        written: snippet_path.is_file(),
        main_exists: main_config.is_file(),
        snippet_path,
        main_config,
        sourced,
    }
}

/// Writes the snippet for `compositor` with `keys` under `config_home`,
/// creating its directory; a file that already holds the same text is
/// left untouched, so a second write changes nothing on disk. The user's
/// Lua main configuration gets its missing include. Returns the rendered snippet and
/// the check after the write.
pub fn write(
    compositor: Compositor,
    config_home: &Path,
    keys: &Keys,
) -> Result<(Snippet, Check), String> {
    let rendered = render(compositor, keys);
    let path = config_home.join(compositor.snippet_path());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let same = std::fs::read_to_string(&path).is_ok_and(|t| t == rendered.text);
    if !same {
        std::fs::write(&path, &rendered.text)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    if compositor == Compositor::HyprlandLua {
        crate::install::ensure_include(config_home)?;
    }
    Ok((rendered, check(compositor, config_home)))
}

fn kdl_args(verb: &str) -> String {
    format!("\"dettivo\" \"--quiet\" \"dictation\" \"{verb}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_write_activates_missing_include() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("hypr")).unwrap();
        std::fs::write(
            dir.path().join("hypr/hyprland.lua"),
            "require('defaults')\n",
        )
        .unwrap();
        let (_, state) = write(Compositor::HyprlandLua, dir.path(), &Keys::defaults()).unwrap();
        assert!(state.sourced, "setup must activate the missing include");
    }

    #[test]
    fn names_parse_and_unknown_ones_list_the_supported_set() {
        assert_eq!(
            "hyprland".parse::<Compositor>().unwrap(),
            Compositor::HyprlandConf
        );
        assert_eq!(
            "hyprland-lua".parse::<Compositor>().unwrap(),
            Compositor::HyprlandLua
        );
        assert_eq!("Sway".parse::<Compositor>().unwrap(), Compositor::Sway);
        assert_eq!("niri".parse::<Compositor>().unwrap(), Compositor::Niri);
        let err = "gnome".parse::<Compositor>().unwrap_err();
        assert_eq!(
            err.to_string(),
            "unknown compositor \"gnome\"; supported: hyprland, sway, niri"
        );
    }

    #[test]
    fn press_and_release_pairs_and_the_niri_note() {
        let keys = Keys::defaults();
        let conf = render(Compositor::HyprlandConf, &keys).text;
        assert!(conf.contains("bind = , F9, exec, dettivo --quiet dictation start\n"));
        assert!(conf.contains("bindr = , F9, exec, dettivo --quiet dictation stop\n"));
        assert!(conf.contains("bind = SUPER CTRL, X, exec, dettivo --quiet dictation toggle\n"));
        let lua = render(Compositor::HyprlandLua, &keys).text;
        assert!(lua.contains("hl.bind(\"F9\", hl.dsp.exec_cmd(\"dettivo --quiet dictation stop\"), { description = \"Dettivo: hold to talk (release)\", release = true })"));
        assert!(lua.contains("hl.unbind(\"SUPER + CTRL + X\")"));
        let sway = render(Compositor::Sway, &keys).text;
        assert!(sway.contains("bindsym F9 exec dettivo --quiet dictation start\n"));
        assert!(sway.contains("bindsym --release F9 exec dettivo --quiet dictation stop\n"));
        let niri = render(Compositor::Niri, &keys);
        assert!(
            niri.text
                .contains("Mod+Ctrl+X { spawn \"dettivo\" \"--quiet\" \"dictation\" \"toggle\"; }")
        );
        assert!(!niri.text.contains("dictation start"));
        assert!(niri.text.contains("no release bindings"));
        assert!(niri.notes.iter().any(|n| n.contains("no release bindings")));
    }

    #[test]
    fn write_is_idempotent_and_check_reports_the_include_state() {
        let dir = tempfile::tempdir().unwrap();
        let keys = Keys::defaults();
        let before = check(Compositor::Sway, dir.path());
        assert!(!before.written && !before.main_exists && !before.sourced);
        let (snippet, after) = write(Compositor::Sway, dir.path(), &keys).unwrap();
        assert!(after.written && !after.sourced);
        assert_eq!(
            std::fs::read_to_string(&after.snippet_path).unwrap(),
            snippet.text
        );
        let first = std::fs::metadata(&after.snippet_path)
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let (_, again) = write(Compositor::Sway, dir.path(), &keys).unwrap();
        assert_eq!(
            std::fs::metadata(&again.snippet_path)
                .unwrap()
                .modified()
                .unwrap(),
            first
        );
        std::fs::write(&after.main_config, "include ~/.config/sway/dettivo\n").unwrap();
        let sourced = check(Compositor::Sway, dir.path());
        assert!(sourced.main_exists && sourced.sourced);
        assert_eq!(sourced.main_config, dir.path().join("sway/config"));
    }

    #[test]
    fn sourced_ignores_comments() {
        assert!(Compositor::HyprlandConf.is_sourced("source = ~/.config/hypr/dettivo.conf\n"));
        assert!(!Compositor::HyprlandConf.is_sourced("# source = ~/.config/hypr/dettivo.conf\n"));
        assert!(
            Compositor::HyprlandLua
                .is_sourced("dofile(os.getenv(\"HOME\") .. \"/.config/hypr/dettivo.lua\")\n")
        );
        assert!(!Compositor::HyprlandLua.is_sourced("-- dofile(\"dettivo.lua\")\n"));
        assert!(Compositor::Sway.is_sourced("include ~/.config/sway/dettivo\n"));
        assert!(Compositor::Niri.is_sourced("include \"dettivo.kdl\"\n"));
        assert!(!Compositor::Niri.is_sourced("// include \"dettivo.kdl\"\n"));
    }
}

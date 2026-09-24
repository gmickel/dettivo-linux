//! The `[insert]` settings the chain reads, mirrored from `config.toml`
//! by the daemon (this crate does not read the file itself).

use std::collections::BTreeMap;

/// The backend names of the chain in FR-I1 order.
pub const BACKEND_NAMES: &[&str] = &[
    "virtual_keyboard",
    "libei",
    "ydotool",
    "xdotool",
    "clipboard_paste",
    "clipboard",
];

/// The paste keystroke for the clipboard path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteKeys {
    /// `ctrl+v`, the desktop default.
    CtrlV,
    /// `ctrl+shift+v`, the terminal convention.
    CtrlShiftV,
    /// `shift+insert`, the X11 primary-selection convention.
    ShiftInsert,
}

impl PasteKeys {
    /// Parses `ctrl+v`, `ctrl+shift+v` or `shift+insert` (any case, any
    /// modifier order).
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts: Vec<String> = text
            .split('+')
            .map(|p| p.trim().to_ascii_lowercase())
            .filter(|p| !p.is_empty())
            .collect();
        parts.sort();
        match parts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["ctrl", "v"] | ["control", "v"] => Some(Self::CtrlV),
            ["ctrl", "shift", "v"] | ["control", "shift", "v"] => Some(Self::CtrlShiftV),
            ["insert", "shift"] => Some(Self::ShiftInsert),
            _ => None,
        }
    }

    /// The configuration spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CtrlV => "ctrl+v",
            Self::CtrlShiftV => "ctrl+shift+v",
            Self::ShiftInsert => "shift+insert",
        }
    }
}

/// Everything the chain is configured with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `auto`, or one of [`BACKEND_NAMES`] to pin.
    pub backend: String,
    /// Paste keystroke per app id.
    pub paste_keys: BTreeMap<String, String>,
    /// App ids that are terminals.
    pub terminal_app_ids: Vec<String>,
    /// App ids of Dettivo's own windows.
    pub self_app_ids: Vec<String>,
    /// Pause between typed keys.
    pub inter_key_delay_ms: u64,
    /// Put the previous clipboard back after a clipboard insertion.
    pub restore_clipboard: bool,
    /// How long the restore waits for the target to take the paste.
    pub clipboard_restore_delay_ms: u64,
    /// How long after an insertion `insert.undo` may take it back.
    pub undo_window_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        let text = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Self {
            backend: "auto".to_string(),
            paste_keys: BTreeMap::new(),
            terminal_app_ids: text(&[
                "foot",
                "footclient",
                "com.mitchellh.ghostty",
                "ghostty",
                "Alacritty",
                "alacritty",
                "kitty",
                "org.wezfurlong.wezterm",
                "wezterm",
                "xterm",
                "org.gnome.Console",
                "org.gnome.Terminal",
                "org.kde.konsole",
            ]),
            self_app_ids: text(&["dettivo", "dettivo-app", "dettivo-osd", "dettivo-sheet"]),
            inter_key_delay_ms: 2,
            restore_clipboard: true,
            clipboard_restore_delay_ms: 300,
            undo_window_ms: 5000,
        }
    }
}

impl Settings {
    /// The paste keystroke for an app id: the configured one, else the
    /// terminal convention for a terminal, else `ctrl+v`.
    pub fn paste_keys_for(&self, app_id: &str) -> PasteKeys {
        if let Some(keys) = self
            .paste_keys
            .get(app_id)
            .and_then(|k| PasteKeys::parse(k))
        {
            return keys;
        }
        if self.is_terminal(app_id) {
            PasteKeys::CtrlShiftV
        } else {
            PasteKeys::CtrlV
        }
    }

    /// True for a configured terminal app id (case-insensitive).
    pub fn is_terminal(&self, app_id: &str) -> bool {
        self.terminal_app_ids
            .iter()
            .any(|t| t.eq_ignore_ascii_case(app_id))
    }

    /// True for one of Dettivo's own app ids (case-insensitive).
    pub fn is_self(&self, app_id: &str) -> bool {
        self.self_app_ids
            .iter()
            .any(|t| t.eq_ignore_ascii_case(app_id))
    }

    /// The pinned backend name, `None` for `auto`.
    pub fn pin(&self) -> Option<&str> {
        let name = self.backend.trim();
        (!name.is_empty() && name != "auto").then_some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_keys_resolve_from_config_then_terminal_list_then_default() {
        let mut settings = Settings::default();
        assert_eq!(settings.paste_keys_for("foot"), PasteKeys::CtrlShiftV);
        assert_eq!(
            settings.paste_keys_for("org.mozilla.firefox"),
            PasteKeys::CtrlV
        );
        settings
            .paste_keys
            .insert("foot".into(), "shift+insert".into());
        assert_eq!(settings.paste_keys_for("foot"), PasteKeys::ShiftInsert);
        settings.paste_keys.insert("foot".into(), "bogus".into());
        assert_eq!(settings.paste_keys_for("foot"), PasteKeys::CtrlShiftV);
        assert_eq!(
            PasteKeys::parse("Shift+Ctrl+V"),
            Some(PasteKeys::CtrlShiftV)
        );
        assert!(settings.is_self("Dettivo-App"));
        assert_eq!(settings.pin(), None);
        settings.backend = "xdotool".into();
        assert_eq!(settings.pin(), Some("xdotool"));
    }
}

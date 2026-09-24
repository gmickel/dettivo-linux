//! Chords as `[hotkeys]` writes them (`SUPER CTRL, X`, `F9`, or the Lua
//! spelling `SUPER + CTRL + X`), parsed once and rendered for every
//! consumer: a Hyprland `bind` line or `hl.bind` call, a Sway `bindsym`,
//! a Niri `binds` entry, a portal `preferred_trigger`, and the evdev codes
//! the evdev backend matches.

use std::fmt;

/// A modifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Modifier {
    /// Super, Logo, Mod4, Win.
    Super,
    /// Control.
    Ctrl,
    /// Alt, Mod1.
    Alt,
    /// Shift.
    Shift,
}

impl Modifier {
    /// Every modifier in canonical order.
    pub const ALL: [Modifier; 4] = [Self::Super, Self::Ctrl, Self::Alt, Self::Shift];

    fn parse(word: &str) -> Option<Self> {
        match word.to_ascii_uppercase().as_str() {
            "SUPER" | "LOGO" | "WIN" | "MOD4" | "MOD" | "META" => Some(Self::Super),
            "CTRL" | "CONTROL" => Some(Self::Ctrl),
            "ALT" | "MOD1" => Some(Self::Alt),
            "SHIFT" => Some(Self::Shift),
            _ => None,
        }
    }

    /// The evdev key codes that count as this modifier (left and right).
    pub fn evdev_codes(self) -> [u16; 2] {
        match self {
            Self::Super => [125, 126],
            Self::Ctrl => [29, 97],
            Self::Alt => [56, 100],
            Self::Shift => [42, 54],
        }
    }
}

/// The key of a chord: an xkb keysym name with its evdev code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    name: &'static str,
    evdev: u16,
}

impl Key {
    /// The canonical xkb keysym name (`F9`, `Escape`, `x`).
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The evdev `KEY_*` code.
    pub fn evdev_code(&self) -> u16 {
        self.evdev
    }

    fn is_letter(&self) -> bool {
        self.name.len() == 1 && self.name.as_bytes()[0].is_ascii_lowercase()
    }
}

/// A chord: modifiers plus one key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    /// The modifiers in canonical order, without duplicates.
    pub modifiers: Vec<Modifier>,
    /// The key.
    pub key: Key,
}

/// Why a chord did not parse; names the config key when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordError {
    /// The config key (`hotkeys.hold`) when the chord came from the file.
    pub key: Option<String>,
    /// The chord text as given.
    pub chord: String,
    /// What is wrong with it.
    pub message: String,
}

impl ChordError {
    pub(crate) fn for_key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }
}

impl fmt::Display for ChordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(key) = &self.key {
            write!(f, "{key}: ")?;
        }
        write!(
            f,
            "cannot parse chord {:?}: {}; the notation is `MODIFIERS, KEY` (Hyprland style, e.g. `SUPER CTRL, X` or `F9`; `SUPER + CTRL + X` is accepted too), modifiers SUPER, CTRL, ALT, SHIFT",
            self.chord, self.message
        )
    }
}

impl std::error::Error for ChordError {}

impl Chord {
    /// Parses `SUPER CTRL, X`, `F9` or `SUPER + CTRL + X`.
    pub fn parse(text: &str) -> Result<Self, ChordError> {
        let err = |message: String| ChordError {
            key: None,
            chord: text.to_string(),
            message,
        };
        let (mods, key) = match text.split_once(',') {
            Some((mods, key)) => (mods.to_string(), key.trim().to_string()),
            None => {
                let mut words: Vec<&str> = text
                    .split(['+', ' '])
                    .map(str::trim)
                    .filter(|w| !w.is_empty())
                    .collect();
                let key = words.pop().map(str::to_string).unwrap_or_default();
                (words.join(" "), key)
            }
        };
        if key.is_empty() {
            return Err(err("no key after the modifiers".into()));
        }
        if key.contains(',') || key.contains(' ') {
            return Err(err(format!("`{key}` is not one key")));
        }
        let mut modifiers = Vec::new();
        for word in mods
            .split(['+', ' '])
            .map(str::trim)
            .filter(|w| !w.is_empty())
        {
            let m =
                Modifier::parse(word).ok_or_else(|| err(format!("unknown modifier `{word}`")))?;
            if !modifiers.contains(&m) {
                modifiers.push(m);
            }
        }
        modifiers.sort();
        let key = lookup(&key).ok_or_else(|| err(format!("unknown key `{key}`")))?;
        Ok(Self { modifiers, key })
    }

    fn has(&self, m: Modifier) -> bool {
        self.modifiers.contains(&m)
    }

    /// Hyprland `bind = MODS, KEY` spelling: `SUPER CTRL, X`.
    pub fn hyprland_conf(&self) -> String {
        let mods = self
            .modifiers
            .iter()
            .map(|m| match m {
                Modifier::Super => "SUPER",
                Modifier::Ctrl => "CTRL",
                Modifier::Alt => "ALT",
                Modifier::Shift => "SHIFT",
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("{mods}, {}", self.key_upper())
    }

    /// Hyprland Lua `hl.bind` spelling: `SUPER + CTRL + X`.
    pub fn hyprland_lua(&self) -> String {
        let mut parts: Vec<String> = self
            .modifiers
            .iter()
            .map(|m| {
                match m {
                    Modifier::Super => "SUPER",
                    Modifier::Ctrl => "CTRL",
                    Modifier::Alt => "ALT",
                    Modifier::Shift => "SHIFT",
                }
                .to_string()
            })
            .collect();
        parts.push(self.key_upper().to_ascii_uppercase());
        parts.join(" + ")
    }

    /// Sway `bindsym` spelling: `Mod4+Ctrl+x`.
    pub fn sway(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for m in &self.modifiers {
            parts.push(match m {
                Modifier::Super => "Mod4",
                Modifier::Ctrl => "Ctrl",
                Modifier::Alt => "Alt",
                Modifier::Shift => "Shift",
            });
        }
        parts.push(self.key.name);
        parts.join("+")
    }

    /// Niri spelling: `Mod+Ctrl+X`.
    pub fn niri(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for m in &self.modifiers {
            parts.push(
                match m {
                    Modifier::Super => "Mod",
                    Modifier::Ctrl => "Ctrl",
                    Modifier::Alt => "Alt",
                    Modifier::Shift => "Shift",
                }
                .to_string(),
            );
        }
        parts.push(self.key_upper());
        parts.join("+")
    }

    /// The portal's `preferred_trigger` (XDG shortcuts): `LOGO+CTRL+x`.
    pub fn portal_trigger(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for m in &self.modifiers {
            parts.push(match m {
                Modifier::Super => "LOGO",
                Modifier::Ctrl => "CTRL",
                Modifier::Alt => "ALT",
                Modifier::Shift => "SHIFT",
            });
        }
        parts.push(self.key.name);
        parts.join("+")
    }

    /// True when `pressed_modifiers` is exactly this chord's set.
    pub fn modifiers_match(&self, pressed: &[Modifier]) -> bool {
        Modifier::ALL
            .iter()
            .all(|m| self.has(*m) == pressed.contains(m))
    }

    fn key_upper(&self) -> String {
        if self.key.is_letter() {
            self.key.name.to_ascii_uppercase()
        } else {
            self.key.name.to_string()
        }
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.is_empty() {
            f.write_str(&self.key_upper())
        } else {
            f.write_str(&self.hyprland_conf())
        }
    }
}

/// `(canonical xkb name, evdev code, aliases)`.
const KEYS: &[(&str, u16, &[&str])] = &[
    ("Escape", 1, &["esc"]),
    ("Return", 28, &["enter"]),
    ("space", 57, &[]),
    ("Tab", 15, &[]),
    ("BackSpace", 14, &["backspace"]),
    ("Delete", 111, &["del"]),
    ("Insert", 110, &["ins"]),
    ("Home", 102, &[]),
    ("End", 107, &[]),
    ("Prior", 104, &["pageup", "page_up"]),
    ("Next", 109, &["pagedown", "page_down"]),
    ("Up", 103, &[]),
    ("Down", 108, &[]),
    ("Left", 105, &[]),
    ("Right", 106, &[]),
    ("Print", 99, &["printscreen"]),
    ("Pause", 119, &[]),
    ("Menu", 139, &[]),
    ("Caps_Lock", 58, &["capslock"]),
    ("Num_Lock", 69, &["numlock"]),
    ("Scroll_Lock", 70, &["scrolllock"]),
    ("comma", 51, &[]),
    ("period", 52, &[]),
    ("minus", 12, &[]),
    ("equal", 13, &[]),
    ("slash", 53, &[]),
    ("backslash", 43, &[]),
    ("semicolon", 39, &[]),
    ("apostrophe", 40, &[]),
    ("grave", 41, &[]),
    ("bracketleft", 26, &[]),
    ("bracketright", 27, &[]),
    ("XF86AudioMute", 113, &[]),
    ("XF86AudioLowerVolume", 114, &[]),
    ("XF86AudioRaiseVolume", 115, &[]),
    ("XF86AudioMicMute", 248, &[]),
    ("XF86AudioPlay", 164, &[]),
    ("XF86AudioNext", 163, &[]),
    ("XF86AudioPrev", 165, &[]),
];

const LETTERS: &[(&str, u16)] = &[
    ("a", 30),
    ("b", 48),
    ("c", 46),
    ("d", 32),
    ("e", 18),
    ("f", 33),
    ("g", 34),
    ("h", 35),
    ("i", 23),
    ("j", 36),
    ("k", 37),
    ("l", 38),
    ("m", 50),
    ("n", 49),
    ("o", 24),
    ("p", 25),
    ("q", 16),
    ("r", 19),
    ("s", 31),
    ("t", 20),
    ("u", 22),
    ("v", 47),
    ("w", 17),
    ("x", 45),
    ("y", 21),
    ("z", 44),
];

const DIGITS: &[(&str, u16)] = &[
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
];

/// `F1` to `F12`. The evdev codes for F13 and up reach compositors as
/// `XF86Tools` and friends under the standard xkb rules, so a chord names
/// them by those keysyms rather than by `F13`.
const FUNCTION: &[(&str, u16)] = &[
    ("F1", 59),
    ("F2", 60),
    ("F3", 61),
    ("F4", 62),
    ("F5", 63),
    ("F6", 64),
    ("F7", 65),
    ("F8", 66),
    ("F9", 67),
    ("F10", 68),
    ("F11", 87),
    ("F12", 88),
];

fn lookup(text: &str) -> Option<Key> {
    let lower = text.to_ascii_lowercase();
    let found = LETTERS
        .iter()
        .chain(DIGITS)
        .chain(FUNCTION)
        .find(|(n, _)| n.eq_ignore_ascii_case(text))
        .map(|(name, code)| Key { name, evdev: *code });
    if found.is_some() {
        return found;
    }
    KEYS.iter()
        .find(|(name, _, aliases)| {
            name.eq_ignore_ascii_case(text) || aliases.contains(&lower.as_str())
        })
        .map(|(name, code, _)| Key { name, evdev: *code })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_notation_parses_to_the_same_chord() {
        let a = Chord::parse("SUPER CTRL, X").unwrap();
        let b = Chord::parse("SUPER + CTRL + X").unwrap();
        let c = Chord::parse("ctrl super, x").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a.modifiers, [Modifier::Super, Modifier::Ctrl]);
        assert_eq!(a.key.name(), "x");
        assert_eq!(a.key.evdev_code(), 45);
        let f9 = Chord::parse("F9").unwrap();
        assert!(f9.modifiers.is_empty());
        assert_eq!(f9.key.evdev_code(), 67);
        assert_eq!(Chord::parse(", F9").unwrap(), f9);
        assert_eq!(Chord::parse("esc").unwrap().key.name(), "Escape");
        assert_eq!(Chord::parse("SUPER, Page_Up").unwrap().key.name(), "Prior");
    }

    #[test]
    fn renderings_per_consumer() {
        let c = Chord::parse("SUPER CTRL SHIFT, x").unwrap();
        assert_eq!(c.hyprland_conf(), "SUPER CTRL SHIFT, X");
        assert_eq!(c.hyprland_lua(), "SUPER + CTRL + SHIFT + X");
        assert_eq!(c.sway(), "Mod4+Ctrl+Shift+x");
        assert_eq!(c.niri(), "Mod+Ctrl+Shift+X");
        assert_eq!(c.portal_trigger(), "LOGO+CTRL+SHIFT+x");
        assert_eq!(c.to_string(), "SUPER CTRL SHIFT, X");
        let f9 = Chord::parse("F9").unwrap();
        assert_eq!(f9.hyprland_conf(), ", F9");
        assert_eq!(f9.hyprland_lua(), "F9");
        assert_eq!(f9.sway(), "F9");
        assert_eq!(f9.niri(), "F9");
        assert_eq!(f9.portal_trigger(), "F9");
        assert_eq!(f9.to_string(), "F9");
        let esc = Chord::parse("SUPER CTRL, Escape").unwrap();
        assert_eq!(esc.hyprland_lua(), "SUPER + CTRL + ESCAPE");
        assert_eq!(esc.sway(), "Mod4+Ctrl+Escape");
    }

    #[test]
    fn errors_name_the_key_and_the_notation() {
        for (text, fragment) in [
            ("SUPER,", "no key"),
            ("HYPER, X", "unknown modifier `HYPER`"),
            ("SUPER, Frob", "unknown key `Frob`"),
            ("SUPER, X Y", "not one key"),
            ("", "no key"),
        ] {
            let err = Chord::parse(text).unwrap_err();
            assert!(err.message.contains(fragment), "{text}: {err}");
            assert!(err.to_string().contains("SUPER CTRL, X"), "{err}");
        }
    }

    #[test]
    fn modifier_matching_is_exact() {
        let c = Chord::parse("SUPER CTRL, X").unwrap();
        assert!(c.modifiers_match(&[Modifier::Ctrl, Modifier::Super]));
        assert!(!c.modifiers_match(&[Modifier::Super]));
        assert!(!c.modifiers_match(&[Modifier::Super, Modifier::Ctrl, Modifier::Shift]));
        assert!(Chord::parse("F9").unwrap().modifiers_match(&[]));
    }
}

//! The chords the Hyprland hotkey scenario owns: Ctrl+Alt+Shift with
//! four function keys reserved from what `hyprctl binds -j` shows free,
//! so neither the snippet's own unbind nor the cleanup touches a binding
//! of the user's, and the cleanup is checked against the compositor.

use evdev::KeyCode;
use serde_json::Value;

/// Hyprland's modmask for Ctrl+Alt+Shift (SHIFT 1, CTRL 4, ALT 8).
const MODMASK: u64 = 13;

/// The function keys a chord may sit on, with their evdev codes.
const FUNCTION_KEYS: [(u8, KeyCode); 12] = [
    (1, KeyCode::KEY_F1),
    (2, KeyCode::KEY_F2),
    (3, KeyCode::KEY_F3),
    (4, KeyCode::KEY_F4),
    (5, KeyCode::KEY_F5),
    (6, KeyCode::KEY_F6),
    (7, KeyCode::KEY_F7),
    (8, KeyCode::KEY_F8),
    (9, KeyCode::KEY_F9),
    (10, KeyCode::KEY_F10),
    (11, KeyCode::KEY_F11),
    (12, KeyCode::KEY_F12),
];

/// The four chords the scenario owns: Ctrl+Alt+Shift with function keys
/// that carried no binding when the scenario looked, so loading the
/// snippet (which unbinds its hold and toggle chords first) and the
/// cleanup afterwards touch nothing of the user's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chords {
    /// Hold to talk.
    pub hold: u8,
    /// Toggle.
    pub toggle: u8,
    /// Cancel.
    pub cancel: u8,
    /// Re-insert the last transcript.
    pub reinsert: u8,
}

impl Chords {
    /// Reserves four function keys free of a Ctrl+Alt+Shift binding in
    /// `binds_json` (`hyprctl binds -j`); an error names the shortage.
    pub fn reserve(binds_json: &str) -> Result<Self, String> {
        let taken = bound_function_keys(binds_json)?;
        let free: Vec<u8> = FUNCTION_KEYS
            .iter()
            .map(|(n, _)| *n)
            .filter(|n| !taken.contains(n))
            .collect();
        match free.as_slice() {
            [hold, toggle, cancel, reinsert, ..] => Ok(Self {
                hold: *hold,
                toggle: *toggle,
                cancel: *cancel,
                reinsert: *reinsert,
            }),
            _ => Err(format!(
                "fewer than four Ctrl+Alt+Shift function keys are free of a binding (taken: {taken:?})"
            )),
        }
    }

    /// The four function keys, hold first.
    pub fn keys(&self) -> [u8; 4] {
        [self.hold, self.toggle, self.cancel, self.reinsert]
    }

    /// The `[hotkeys]` section that binds the chords.
    pub fn config(&self) -> String {
        format!(
            "[hotkeys]\nbackend = \"none\"\nhold = \"CTRL ALT SHIFT, F{}\"\ntoggle = \"CTRL ALT SHIFT, F{}\"\ncancel = \"CTRL ALT SHIFT, F{}\"\nreinsert = \"CTRL ALT SHIFT, F{}\"\n",
            self.hold, self.toggle, self.cancel, self.reinsert
        )
    }

    /// The `hl.unbind` calls that take the chords back out.
    pub fn unbind(&self) -> String {
        self.keys()
            .iter()
            .map(|n| format!("hl.unbind(\"CTRL + ALT + SHIFT + F{n}\")"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The chords still bound in `binds_json`, empty once the cleanup
    /// took them all back.
    pub fn still_bound(&self, binds_json: &str) -> Result<Vec<u8>, String> {
        let taken = bound_function_keys(binds_json)?;
        Ok(self
            .keys()
            .iter()
            .copied()
            .filter(|n| taken.contains(n))
            .collect())
    }

    /// The evdev code of function key `n`.
    pub fn code(n: u8) -> KeyCode {
        FUNCTION_KEYS
            .iter()
            .find(|(k, _)| *k == n)
            .map(|(_, code)| *code)
            .expect("a reserved chord is a function key")
    }
}

/// The function keys that carry a Ctrl+Alt+Shift binding in
/// `hyprctl binds -j`: every entry's `modmask` and `key`, the key read
/// case-insensitively.
fn bound_function_keys(binds_json: &str) -> Result<Vec<u8>, String> {
    let binds: Vec<Value> =
        serde_json::from_str(binds_json).map_err(|e| format!("hyprctl binds -j: {e}"))?;
    let mut taken = Vec::new();
    for bind in &binds {
        if bind["modmask"].as_u64() != Some(MODMASK) {
            continue;
        }
        let key = bind["key"].as_str().unwrap_or("").to_ascii_uppercase();
        if let Some(n) = key.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
            if FUNCTION_KEYS.iter().any(|(k, _)| *k == n) && !taken.contains(&n) {
                taken.push(n);
            }
        }
    }
    taken.sort_unstable();
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bind(modmask: u64, key: &str) -> Value {
        json!({"locked": false, "mouse": false, "release": false, "repeat": false, "modmask": modmask, "submap": "", "key": key, "keycode": 0, "catch_all": false, "description": "", "dispatcher": "exec", "arg": "true"})
    }

    /// A chord the user already binds is never reserved, so neither the
    /// snippet's own unbind nor the cleanup can remove it; a compositor
    /// with fewer than four free chords refuses with the ones taken.
    #[test]
    fn reserved_chords_skip_existing_ctrl_alt_shift_bindings() {
        let binds = Value::Array(vec![
            bind(13, "F1"),
            bind(13, "f3"),
            bind(64, "F2"),
            bind(13, "Return"),
        ])
        .to_string();
        let chords = Chords::reserve(&binds).unwrap();
        assert_eq!(
            chords,
            Chords {
                hold: 2,
                toggle: 4,
                cancel: 5,
                reinsert: 6
            }
        );
        assert!(chords.config().contains("hold = \"CTRL ALT SHIFT, F2\""));
        assert!(
            chords
                .unbind()
                .contains("hl.unbind(\"CTRL + ALT + SHIFT + F6\")")
        );
        assert!(!chords.unbind().contains("F1\"") && !chords.unbind().contains("F3\""));
        let crowded =
            Value::Array((1..=9).map(|n| bind(13, &format!("F{n}"))).collect()).to_string();
        let err = Chords::reserve(&crowded).unwrap_err();
        assert!(err.contains("fewer than four"), "{err}");
        assert!(Chords::reserve("not json").is_err());
    }

    /// The cleanup is checked against the compositor: a chord that is
    /// still bound afterwards is named.
    #[test]
    fn still_bound_names_the_chords_the_cleanup_left_behind() {
        let chords = Chords {
            hold: 10,
            toggle: 11,
            cancel: 12,
            reinsert: 9,
        };
        let before = Value::Array(vec![
            bind(13, "F10"),
            bind(13, "F11"),
            bind(13, "F12"),
            bind(13, "F9"),
        ])
        .to_string();
        assert_eq!(chords.still_bound(&before).unwrap(), vec![10, 11, 12, 9]);
        let after = Value::Array(vec![bind(13, "F11"), bind(64, "F10")]).to_string();
        assert_eq!(chords.still_bound(&after).unwrap(), vec![11]);
        assert!(chords.still_bound("[]").unwrap().is_empty());
    }
}

//! Keymap generation for the virtual keyboard and libei backends (ADR
//! 0007, the `wtype` approach): every distinct character of a text gets
//! one keycode whose only keysym is that character, so typing never
//! depends on the user's layout. A text with more distinct characters
//! than the keycode budget is split into batches, each with a fresh
//! keymap. The generated text compiles with xkbcommon (checked in tests
//! and again when a backend uploads it).

use std::collections::BTreeMap;

/// The offset between an evdev code and its xkb keycode.
pub const MIN_KEYCODE: u32 = 8;

/// The evdev codes a generated keymap uses, in allocation order: the 48
/// printable keys of a US keyboard first, because an application that
/// looks at the physical key (a terminal deciding what Backspace or Tab
/// means) treats those as text keys, then the rest of the 8 to 255
/// range with the modifier, editing, navigation, function and keypad
/// codes left out for the same reason.
pub fn evdev_codes() -> Vec<u32> {
    let printable: Vec<u32> = (2..=13)
        .chain(16..=27)
        .chain(30..=41)
        .chain(43..=53)
        .chain(std::iter::once(57))
        .collect();
    let reserved: &[u32] = &[
        1, 14, 15, 28, 29, 42, 54, 55, 56, 58, 69, 70, 74, 78, 79, 80, 81, 82, 83, 96, 97, 98, 99,
        100, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 113, 114, 115, 117, 119, 125, 126,
        127, 139, 142,
    ];
    let mut codes = printable.clone();
    codes.extend((1..=247u32).filter(|c| {
        !printable.contains(c)
            && !reserved.contains(c)
            && !(59..=68).contains(c)
            && !(71..=73).contains(c)
            && !(75..=77).contains(c)
            && !(87..=88).contains(c)
            && !(183..=194).contains(c)
    }));
    codes
}

/// Distinct characters one keymap may carry: one per usable evdev code.
pub const BUDGET: usize = 160;

/// A character the keymap cannot express: a control character with no
/// keysym. Reported by index so the caller can name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmappable {
    /// Character index in the text.
    pub index: usize,
    /// The character.
    pub ch: char,
}

impl std::fmt::Display for Unmappable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "character {} (U+{:04X}) has no keysym",
            self.index, self.ch as u32
        )
    }
}

/// One key of a generated keymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// The xkb keycode (`MIN_KEYCODE` upwards).
    pub keycode: u32,
    /// The keysym name in the keymap text (`U00E9`, `Return`, `Shift_L`).
    pub keysym: String,
}

impl Key {
    /// The evdev code the Wayland and libei protocols carry.
    pub fn evdev(&self) -> u32 {
        self.keycode - MIN_KEYCODE
    }
}

/// A keymap plus the key sequence to type with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    /// The keys, in keycode order.
    pub keys: Vec<Key>,
    /// The characters of this batch, each resolved to its key index.
    pub sequence: Vec<usize>,
    /// The xkb keymap text (format `XKB_V1`).
    pub text: String,
}

impl Batch {
    /// The key for a character of the sequence.
    pub fn key(&self, position: usize) -> &Key {
        &self.keys[self.sequence[position]]
    }
}

/// The keysym name for a character, or `None` for a control character
/// the protocols cannot type.
pub fn keysym_name(ch: char) -> Option<String> {
    match ch {
        '\n' | '\r' => Some("Return".to_string()),
        '\t' => Some("Tab".to_string()),
        c if (c as u32) < 0x20 || (0x7F..0xA0).contains(&(c as u32)) => None,
        c => Some(format!("U{:04X}", c as u32)),
    }
}

/// Splits `text` into batches of at most `budget` distinct characters.
/// The first unmappable character stops the split and is reported.
pub fn batches(text: &str, budget: usize) -> Result<Vec<Batch>, Unmappable> {
    let budget = budget.max(1);
    let mut out = Vec::new();
    let mut names: BTreeMap<char, String> = BTreeMap::new();
    let mut chars: Vec<char> = Vec::new();
    for (index, ch) in text.chars().enumerate() {
        let name = keysym_name(ch).ok_or(Unmappable { index, ch })?;
        if !names.contains_key(&ch) && names.len() == budget {
            out.push(build(&names, &chars));
            names.clear();
            chars.clear();
        }
        names.entry(ch).or_insert(name);
        chars.push(ch);
    }
    if !chars.is_empty() {
        out.push(build(&names, &chars));
    }
    Ok(out)
}

/// A keymap of named keys on their physical evdev codes, for keystrokes
/// (`Control_L`, `v`) and undo (`Shift_L`, `Left`, `Delete`); the
/// sequence is empty.
pub fn named(keys: &[(&str, u32)]) -> Batch {
    let mut keys: Vec<Key> = keys
        .iter()
        .map(|(name, evdev)| Key {
            keycode: MIN_KEYCODE + evdev,
            keysym: (*name).to_string(),
        })
        .collect();
    keys.sort_by_key(|k| k.keycode);
    let text = render(&keys);
    Batch {
        keys,
        sequence: Vec::new(),
        text,
    }
}

fn build(names: &BTreeMap<char, String>, chars: &[char]) -> Batch {
    let codes = evdev_codes();
    let keys: Vec<Key> = names
        .values()
        .enumerate()
        .map(|(i, name)| Key {
            keycode: MIN_KEYCODE + codes[i],
            keysym: name.clone(),
        })
        .collect();
    let index_of: BTreeMap<&char, usize> = names.keys().enumerate().map(|(i, c)| (c, i)).collect();
    let sequence = chars.iter().map(|c| index_of[c]).collect();
    let text = render(&keys);
    Batch {
        keys,
        sequence,
        text,
    }
}

/// The xkb text for a key list.
pub fn render(keys: &[Key]) -> String {
    let max = keys
        .iter()
        .map(|k| k.keycode)
        .max()
        .unwrap_or(MIN_KEYCODE)
        .max(MIN_KEYCODE + 1);
    let mut out = String::new();
    out.push_str("xkb_keymap {\n");
    out.push_str("xkb_keycodes \"(unnamed)\" {\n");
    out.push_str(&format!(
        "    minimum = {MIN_KEYCODE};\n    maximum = {max};\n"
    ));
    for key in keys {
        out.push_str(&format!("    <K{}> = {};\n", key.keycode, key.keycode));
    }
    out.push_str("};\n");
    out.push_str("xkb_types \"(unnamed)\" { include \"complete\" };\n");
    out.push_str("xkb_compatibility \"(unnamed)\" { include \"complete\" };\n");
    out.push_str("xkb_symbols \"(unnamed)\" {\n");
    for key in keys {
        out.push_str(&format!(
            "    key <K{}> {{[{}]}};\n",
            key.keycode, key.keysym
        ));
    }
    out.push_str("};\n};\n");
    out
}

/// Compiles a keymap text with xkbcommon, returning the keymap.
pub fn compile(text: &str) -> Result<xkbcommon::xkb::Keymap, String> {
    use xkbcommon::xkb;
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    xkb::Keymap::new_from_string(
        &context,
        text.to_string(),
        xkb::KEYMAP_FORMAT_TEXT_V1,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or_else(|| "xkbcommon refused the generated keymap".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use xkbcommon::xkb;

    fn typed(batch: &Batch) -> String {
        let keymap = compile(&batch.text).unwrap();
        (0..batch.sequence.len())
            .map(|i| {
                let key = batch.key(i);
                let syms = keymap.key_get_syms_by_level(xkb::Keycode::new(key.keycode), 0, 0);
                assert_eq!(syms.len(), 1, "{}", key.keysym);
                xkb::keysym_to_utf8(syms[0])
                    .trim_end_matches('\0')
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn every_character_maps_to_exactly_one_keycode_and_compiles() {
        let cases = [
            "Hello, world! 123",
            "café crème brûlée ñ ß Ø",
            "日本語のテキスト 中文 한국어",
            "emoji 😀🚀 and 👨‍👩‍👧 family",
            "mixed: Ünïcödé 漢字 🎉 ASCII\ttab\nline",
        ];
        for text in cases {
            let batches = batches(text, BUDGET).unwrap();
            assert_eq!(batches.len(), 1, "{text}");
            let batch = &batches[0];
            let distinct: std::collections::BTreeSet<char> = text.chars().collect();
            assert_eq!(batch.keys.len(), distinct.len(), "{text}");
            assert_eq!(batch.sequence.len(), text.chars().count(), "{text}");
            assert_eq!(typed(batch).replace('\r', "\n"), text, "{text}");
        }
    }

    #[test]
    fn batches_split_above_the_keycode_budget_and_keep_order() {
        let text: String = (0..300u32)
            .map(|i| char::from_u32(0x4E00 + i).unwrap())
            .collect();
        let batches = batches(&text, BUDGET).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].keys.len(), BUDGET);
        assert_eq!(batches[1].keys.len(), 300 - BUDGET);
        let back: String = batches.iter().map(typed).collect();
        assert_eq!(back, text);
        let small = super::batches("abcabcabc", 2).unwrap();
        assert_eq!(small.len(), 5);
        assert_eq!(small.iter().map(typed).collect::<String>(), "abcabcabc");
    }

    #[test]
    fn unmappable_code_points_are_reported_by_index() {
        let err = batches("ok\u{7}bell", BUDGET).unwrap_err();
        assert_eq!(
            err,
            Unmappable {
                index: 2,
                ch: '\u{7}'
            }
        );
        assert!(err.to_string().contains("U+0007"));
        assert!(batches("", BUDGET).unwrap().is_empty());
    }

    #[test]
    fn named_keymaps_carry_modifier_and_editing_keys_on_their_physical_codes() {
        let batch = named(&[("Control_L", 29), ("Left", 105), ("v", 47)]);
        let keymap = compile(&batch.text).unwrap();
        assert_eq!(batch.keys[0].evdev(), 29);
        let left = batch.keys.iter().find(|k| k.keysym == "Left").unwrap();
        let syms = keymap.key_get_syms_by_level(xkb::Keycode::new(left.keycode), 0, 0);
        assert_eq!(syms, &[xkb::Keysym::Left]);
        assert!(keymap.mod_get_index("Control") != xkb::MOD_INVALID);
    }

    #[test]
    fn generated_keys_avoid_physical_editing_and_modifier_codes() {
        let codes = evdev_codes();
        assert!(codes.len() >= BUDGET, "{}", codes.len());
        assert_eq!(&codes[..12], &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
        for reserved in [1, 14, 15, 28, 29, 42, 56, 59, 71, 103, 105, 111, 125] {
            assert!(!codes.contains(&reserved), "{reserved}");
        }
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len(), "no duplicates");
        let batch = batches("ab", BUDGET).unwrap().remove(0);
        assert_eq!(batch.keys[0].evdev(), 2);
        assert_eq!(batch.keys[1].evdev(), 3);
    }
}

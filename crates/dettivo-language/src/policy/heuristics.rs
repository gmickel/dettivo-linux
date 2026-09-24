//! The terminal heuristics of the macOS `PolishPolicyResolver`: prose
//! dictated into a terminal window is not code, so the resolver drops the
//! `code` preset for `generic` when the transcript reads like a sentence
//! rather than a command, and turns `@path` insertion back on when the
//! transcript still names a recoverable file.

use crate::polish::Preset;
use crate::polish::text::{is_match, re};

/// Wayland app ids and X11 classes of the terminals Omarchy ships or a
/// user is likely to run.
const TERMINALS: &[&str] = &[
    "Alacritty",
    "alacritty",
    "com.mitchellh.ghostty",
    "ghostty",
    "foot",
    "footclient",
    "kitty",
    "org.wezfurlong.wezterm",
    "org.kde.konsole",
    "konsole",
    "org.gnome.Console",
    "org.gnome.Terminal",
    "xterm",
    "URxvt",
    "st",
    "dev.warp.Warp",
    "rio",
    "wezterm",
];

/// True when `app_id` names a terminal emulator.
pub(crate) fn is_terminal_app(app_id: &str) -> bool {
    TERMINALS.contains(&app_id)
}

/// The preset a terminal window's transcript overrides to, or `None` when
/// the transcript reads like a command or a profile named the preset.
pub(crate) fn dynamic_preset_override(
    app_id: Option<&str>,
    raw_transcript: Option<&str>,
    has_explicit_override: bool,
) -> Option<Preset> {
    if has_explicit_override || !app_id.is_some_and(is_terminal_app) {
        return None;
    }
    let transcript = raw_transcript?;
    if is_likely_terminal_mixed_prose(transcript) || is_likely_terminal_agent_prose(transcript) {
        Some(Preset::Generic)
    } else {
        None
    }
}

/// The prose signals the macOS resolver counts, each padded with spaces.
const PROSE_SIGNALS: &[&str] = &[
    " i ",
    " i'm ",
    " we ",
    " we need ",
    " you ",
    " this ",
    " that ",
    " it ",
    " is ",
    " are ",
    " was ",
    " were ",
    " should ",
    " could ",
    " before ",
    " after ",
    " because ",
    " if ",
    " but ",
    " then ",
    " let's ",
    " please ",
];

/// Spoken separators a dictation uses when it means code.
const SPOKEN_TECHNICAL_MARKERS: &[&str] = &[
    " dot ",
    " slash ",
    " dash dash ",
    " colon ",
    " point ",
    " at sign ",
    " backtick ",
    " open paren ",
    " close paren ",
];

/// Shell commands a dictation into a terminal starts with.
const COMMAND_PREFIXES: &[&str] = &[
    "git ",
    "npm ",
    "pnpm ",
    "bun ",
    "yarn ",
    "npx ",
    "uv ",
    "pip ",
    "python ",
    "python3 ",
    "cargo ",
    "go ",
    "rustc ",
    "just ",
    "make ",
    "ls ",
    "cd ",
    "mkdir ",
    "rm ",
    "mv ",
    "cp ",
    "cat ",
    "sed ",
    "rg ",
    "grep ",
    "find ",
    "chmod ",
    "chown ",
    "docker ",
    "kubectl ",
    "systemctl ",
    "pacman ",
    "yay ",
    "hyprctl ",
];

fn is_likely_terminal_agent_prose(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || contains_explicit_command_markers(trimmed)
        || contains_spoken_technical_markers(trimmed)
    {
        return false;
    }
    let words = trimmed.split_whitespace().count();
    if words < 6 {
        return false;
    }
    let padded = format!(" {} ", trimmed.to_lowercase());
    let score = PROSE_SIGNALS
        .iter()
        .filter(|s| padded.contains(**s))
        .count();
    let sentence_like = trimmed.contains('.') || trimmed.contains('?') || trimmed.contains('!');
    score >= 2 || (score >= 1 && (sentence_like || words >= 10))
}

fn contains_spoken_technical_markers(text: &str) -> bool {
    let padded = format!(" {} ", text.to_lowercase());
    SPOKEN_TECHNICAL_MARKERS.iter().any(|m| padded.contains(*m))
}

fn contains_explicit_command_markers(text: &str) -> bool {
    let lowered = text.to_lowercase();
    if COMMAND_PREFIXES.iter().any(|p| lowered.starts_with(*p)) {
        return true;
    }
    [
        re!(r"\s--[a-z0-9-]+"),
        re!(r"\b[a-z0-9_./-]+\.(?:swift|rs|toml|ts|tsx|js|jsx|py|md|json|yaml|yml|sh|zsh|plist)\b"),
        re!(r"[{}\[\]();`]"),
    ]
    .iter()
    .any(|re| is_match(re, &lowered))
}

/// True when the transcript still names a file the `@path` post-processor
/// could recover.
pub(crate) fn transcript_likely_contains_recoverable_path(text: Option<&str>) -> bool {
    let Some(text) = text else { return false };
    let lowered = text.to_lowercase();
    [
        re!(
            r"\b(?:readme|changelog|settings ?view|design ?guide|index|release|plist|polish rewrite engine)\b"
        ),
        re!(r"\b(?:dot md|dot swift|dot rs|dot ts|dot json|dot toml|dot plist)\b"),
        re!(r"\b(?:slash|docslash)\b"),
        re!(r"\bdash dash\b"),
    ]
    .iter()
    .any(|re| is_match(re, &lowered))
}

/// The prose markers the mixed-prose check counts once a recoverable path
/// is present.
const MIXED_PROSE_MARKERS: &[&str] = &[
    " please ",
    " before ",
    " after ",
    " tomorrow",
    " today",
    " review ",
    " recap ",
    " memo ",
    " roadmap ",
    " need ",
];

fn is_likely_terminal_mixed_prose(text: &str) -> bool {
    if !transcript_likely_contains_recoverable_path(Some(text)) {
        return false;
    }
    let padded = format!(" {} ", text.to_lowercase());
    MIXED_PROSE_MARKERS
        .iter()
        .filter(|m| padded.contains(**m))
        .count()
        >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_prose_overrides_to_generic_and_commands_do_not() {
        assert!(is_terminal_app("Alacritty"));
        assert!(!is_terminal_app("code"));
        assert_eq!(
            dynamic_preset_override(
                Some("Alacritty"),
                Some("i think we should review this before we ship it tomorrow"),
                false
            ),
            Some(Preset::Generic)
        );
        assert_eq!(
            dynamic_preset_override(Some("Alacritty"), Some("cargo test --workspace"), false),
            None
        );
        assert_eq!(
            dynamic_preset_override(
                Some("Alacritty"),
                Some("i think we should review this before we ship it tomorrow"),
                true
            ),
            None
        );
        assert_eq!(
            dynamic_preset_override(Some("code"), Some("i think we should review this"), false),
            None
        );
    }

    #[test]
    fn mixed_prose_needs_a_recoverable_path_and_two_prose_markers() {
        assert!(is_likely_terminal_mixed_prose(
            "please review the readme before the recap"
        ));
        assert!(!is_likely_terminal_mixed_prose("readme"));
        assert!(transcript_likely_contains_recoverable_path(Some(
            "open the changelog"
        )));
        assert!(!transcript_likely_contains_recoverable_path(None));
    }
}

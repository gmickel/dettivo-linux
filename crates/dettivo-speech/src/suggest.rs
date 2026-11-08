//! Sound-alikes for a vocabulary term (ADR 0104): two Piper voices speak
//! the term alone and inside a short sentence, and the speech engine
//! transcribes each clip twice, with the vocabulary in force and with none.
//! In a clip the vocabulary did not fix, the words the engine wrote with
//! none where the term should be are a spelling it mishears the term as. Piper is
//! an outside program, run with its noise turned off so the same voice
//! says the same text the same way every time.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dettivo_engine_proto::sound::is_term_mark;

/// The Piper voices, in the order their clips are heard. Both speak at
/// 16 kHz, the rate every speech engine takes.
pub const VOICES: [&str; 2] = ["en_US-amy-low", "en_US-ryan-low"];
/// Where the voices come from.
pub const VOICES_URL: &str = "https://huggingface.co/rhasspy/piper-voices/tree/main/en/en_US";
/// The directory under the models directory that holds the voices.
pub const VOICES_DIR: &str = "piper";
/// The sentence a term is also said in: `{}` is the term. A term said
/// mid-sentence runs into its neighbours as it does in dictation.
const CARRIER: (&str, &str) = ("I think", "is great.");
/// The sample rate the voices must speak at.
const RATE: u64 = 16_000;

/// The words of `text` as the term-recall bench reads them: lower case,
/// split at anything that is not a letter or a digit.
pub fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// What a voice says for `term`: the term alone, then in the sentence.
pub fn sayings(term: &str) -> [String; 2] {
    [
        term.to_string(),
        format!("{} {term} {}", CARRIER.0, CARRIER.1),
    ]
}

/// Whether `heard`, the clip as dictation hears it, holds `term` spelt
/// right, and when it does not, the words `unprimed` (the clip heard with
/// no vocabulary) holds for it. `in_sentence` says the clip was the
/// sentence: the term's stretch is what lies between the sentence's own
/// words, and nothing is proposed when they did not come back. A stretch
/// more than two words longer than the term is not a spelling, and neither
/// is the term itself.
pub fn hear(term: &str, heard: &str, unprimed: &str, in_sentence: bool) -> (bool, Option<String>) {
    // A term spelt with marks (`C#`, `C++`) is right only where the text
    // keeps them: its words alone are `c`, which any lone `C` matches.
    let marks = term.chars().any(is_term_mark);
    let spelt = |text: &str| !marks || text.to_lowercase().contains(&term.to_lowercase());
    let term_words = words(term);
    let holds = |text: &[String]| {
        !term_words.is_empty()
            && text
                .windows(term_words.len())
                .any(|w| w == term_words.as_slice())
    };
    let (heard_words, unprimed_words) = (words(heard), words(unprimed));
    // A sentence clip is judged only between the sentence's own words; one
    // whose sentence did not come back is neither right nor a spelling.
    if region(&heard_words, in_sentence).is_some_and(holds) && spelt(heard) {
        return (true, None);
    }
    let Some(found) =
        region(&unprimed_words, in_sentence).filter(|s| !(holds(s) && spelt(unprimed)))
    else {
        return (false, None);
    };
    let spelling =
        (!found.is_empty() && found.len() <= term_words.len().max(1) + 2).then(|| found.join(" "));
    (false, spelling)
}

/// The words a clip is judged on: all of them, or for a sentence clip
/// those between the sentence's own (`None` when it did not come back).
fn region(text: &[String], in_sentence: bool) -> Option<&[String]> {
    if in_sentence {
        stretch(text)
    } else {
        Some(text)
    }
}

/// The words of a sentence clip between the sentence's own; `None` when
/// the sentence around the term did not come back.
fn stretch(words_heard: &[String]) -> Option<&[String]> {
    let (before, after) = (words(CARRIER.0), words(CARRIER.1));
    let fits = words_heard.len() > before.len() + after.len()
        && words_heard.starts_with(&before)
        && words_heard.ends_with(&after);
    fits.then(|| &words_heard[before.len()..words_heard.len() - after.len()])
}

/// Whether every clip heard the term right, and the spellings to propose:
/// each misheard spelling once, in clip order, without those `listed`.
pub fn propose(heard: &[(bool, Option<String>)], listed: &[String]) -> (bool, Vec<String>) {
    let recognised = heard.iter().all(|(right, _)| *right);
    let mut known: Vec<String> = listed.iter().map(|s| words(s).join(" ")).collect();
    let mut out = Vec::new();
    for spelling in heard.iter().filter_map(|(_, s)| s.as_ref()) {
        if !known.contains(spelling) {
            known.push(spelling.clone());
            out.push(spelling.clone());
        }
    }
    (recognised, out)
}

/// The Piper program and its voices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piper {
    /// The `piper` executable.
    pub binary: PathBuf,
    /// Each voice of [`VOICES`] with its model file.
    pub voices: Vec<(String, PathBuf)>,
}

impl Piper {
    /// Piper from `PATH` (or `~/.local/bin`, where `pipx` installs it) and
    /// the voices under `<models_dir>/piper`; what is missing, and how to
    /// get it, otherwise.
    pub fn find(
        models_dir: &Path,
        path: Option<&std::ffi::OsStr>,
        home: Option<&Path>,
    ) -> Result<Self, String> {
        let binary = path
            .into_iter()
            .flat_map(std::env::split_paths)
            .chain(home.map(|h| h.join(".local/bin")))
            .map(|dir| dir.join("piper"))
            .find(|p| {
                use std::os::unix::fs::PermissionsExt;
                p.metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            });
        let dir = models_dir.join(VOICES_DIR);
        let mut voices = Vec::new();
        let mut missing = Vec::new();
        for voice in VOICES {
            let model = dir.join(format!("{voice}.onnx"));
            match voice_rate(&model) {
                Ok(RATE) => voices.push((voice.to_string(), model)),
                Ok(rate) => missing.push(format!(
                    "{voice} speaks at {rate} Hz; the low-quality voice speaks at 16 kHz"
                )),
                Err(_) => missing.push(format!(
                    "{voice}.onnx and {voice}.onnx.json in {}",
                    dir.display()
                )),
            }
        }
        if binary.is_none() {
            missing.insert(0, "the piper command (`pipx install piper-tts`), on the daemon's PATH or in ~/.local/bin".into());
        }
        match binary {
            Some(binary) if missing.is_empty() => Ok(Self { binary, voices }),
            _ => Err(format!(
                "hearing a term needs Piper and two of its voices ({VOICES_URL}); missing:\n  {}",
                missing.join("\n  ")
            )),
        }
    }

    /// `text` spoken by the voice model at `model`: 16 kHz mono samples.
    pub fn speak(&self, model: &Path, text: &str) -> Result<Vec<i16>, String> {
        let mut child = Command::new(&self.binary)
            .arg("-m")
            .arg(model)
            .args(["--noise-scale", "0", "--noise-w-scale", "0", "--output-raw"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: {e}", self.binary.display()))?;
        if let Some(mut stdin) = child.stdin.take() {
            writeln!(stdin, "{text}").map_err(|e| format!("piper: {e}"))?;
        }
        let out = child
            .wait_with_output()
            .map_err(|e| format!("piper: {e}"))?;
        if !out.status.success() || out.stdout.len() < 2 {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let tail = stderr.lines().last().unwrap_or("no audio");
            return Err(format!("piper could not speak {text:?}: {tail}"));
        }
        Ok(out
            .stdout
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect())
    }
}

/// The sample rate the voice config beside `model` names.
fn voice_rate(model: &Path) -> Result<u64, String> {
    if !model.is_file() {
        return Err(format!("{} is missing", model.display()));
    }
    let config = model.with_extension("onnx.json");
    let text =
        std::fs::read_to_string(&config).map_err(|e| format!("{}: {e}", config.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", config.display()))?;
    value["audio"]["sample_rate"]
        .as_u64()
        .ok_or_else(|| format!("{}: no audio.sample_rate", config.display()))
}

#[cfg(test)]
#[path = "suggest_tests.rs"]
mod tests;

//! The sideloaded polish fine-tune (ADR 0032): a manifest in the macOS
//! shape (`id`, `displayName`, `relativeModelPath`) under the experiments
//! directory names a directory holding exactly one GGUF, either a fused
//! model or a LoRA adapter over a catalogue base. `resolve` turns the
//! manifest `[llm] polish_experiment` names into the file the engine
//! loads, and every way that can fail is one named error so
//! `llm.models.status` and `dettivo doctor` can say which.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The two forms a sideloaded fine-tune takes on Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// One fused GGUF that loads on its own.
    #[default]
    Gguf,
    /// One GGUF LoRA adapter applied over `baseModel` at load.
    GgufLora,
}

impl Format {
    /// The wire spelling (`gguf`, `gguf-lora`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gguf => "gguf",
            Self::GgufLora => "gguf-lora",
        }
    }
}

/// The manifest file: the three macOS fields plus the two Linux
/// additions, unknown fields kept out of the way so a manifest the macOS
/// packaging script writes reads as is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// The experiment id (`qwen3-1.7b-private-best`).
    pub id: String,
    /// The name to show.
    pub display_name: String,
    /// The model directory, relative to the experiments directory.
    pub relative_model_path: String,
    /// `gguf` (the default) or `gguf-lora`.
    #[serde(default)]
    pub format: Format,
    /// The catalogue model a `gguf-lora` adapter applies over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_model: Option<String>,
}

/// A manifest resolved to the file the engine loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sideload {
    /// The manifest name (`current`).
    pub name: String,
    /// The manifest file.
    pub manifest: PathBuf,
    /// The experiment id.
    pub id: String,
    /// The name to show.
    pub display_name: String,
    /// The form.
    pub format: Format,
    /// The one GGUF in the model directory: the fused model, or the
    /// adapter.
    pub gguf: PathBuf,
    /// Its size.
    pub size_bytes: u64,
    /// The catalogue base a `gguf-lora` adapter applies over.
    pub base_model: Option<String>,
}

/// Why a manifest did not resolve, worded for `manifest_error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideloadError {
    /// The manifest name that was asked for.
    pub name: String,
    /// One line naming the file or directory at fault.
    pub message: String,
}

impl std::fmt::Display for SideloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "polish experiment {}: {}", self.name, self.message)
    }
}

impl std::error::Error for SideloadError {}

/// The manifest file `name` stands for under `dir`: `current` is
/// `current.json`; a name that already ends in `.json` is taken as is.
pub fn manifest_path(dir: &Path, name: &str) -> PathBuf {
    if name.ends_with(".json") {
        dir.join(name)
    } else {
        dir.join(format!("{name}.json"))
    }
}

fn fail(name: &str, message: impl Into<String>) -> SideloadError {
    SideloadError {
        name: name.to_string(),
        message: message.into(),
    }
}

/// A relative path that stays inside the experiments directory: no
/// absolute path, no parent segment.
fn relative_inside(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && p.is_relative()
        && p.components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// Resolves the manifest `name` under `experiments_dir`.
pub fn resolve(experiments_dir: &Path, name: &str) -> Result<Sideload, SideloadError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(fail(name, "no manifest name"));
    }
    let manifest = manifest_path(experiments_dir, name);
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| fail(name, format!("{} cannot be read ({e})", manifest.display())))?;
    let parsed: Manifest = serde_json::from_str(&text).map_err(|e| {
        fail(
            name,
            format!("{} is not a manifest ({e})", manifest.display()),
        )
    })?;
    if parsed.id.trim().is_empty() {
        return Err(fail(
            name,
            format!("{} has an empty id", manifest.display()),
        ));
    }
    if !relative_inside(&parsed.relative_model_path) {
        return Err(fail(
            name,
            format!(
                "relativeModelPath {:?} must be a relative path inside {}",
                parsed.relative_model_path,
                experiments_dir.display()
            ),
        ));
    }
    let dir = experiments_dir.join(&parsed.relative_model_path);
    if !dir.is_dir() {
        return Err(fail(name, format!("{} is not a directory", dir.display())));
    }
    let mut ggufs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| fail(name, format!("{} cannot be listed ({e})", dir.display())))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "gguf"))
        .collect();
    ggufs.sort();
    let gguf = match ggufs.len() {
        1 => ggufs.remove(0),
        0 => {
            return Err(fail(name, format!("{} holds no .gguf file", dir.display())));
        }
        n => {
            return Err(fail(
                name,
                format!(
                    "{} holds {n} .gguf files; a sideload directory holds exactly one",
                    dir.display()
                ),
            ));
        }
    };
    let base_model = match parsed.format {
        Format::Gguf => None,
        Format::GgufLora => match parsed.base_model.as_deref().map(str::trim) {
            Some(base) if !base.is_empty() => Some(base.to_string()),
            _ => {
                return Err(fail(
                    name,
                    format!(
                        "{} is gguf-lora but names no baseModel (a catalogue id)",
                        manifest.display()
                    ),
                ));
            }
        },
    };
    let size_bytes = std::fs::metadata(&gguf).map(|m| m.len()).unwrap_or(0);
    Ok(Sideload {
        name: name.to_string(),
        manifest,
        id: parsed.id.trim().to_string(),
        display_name: parsed.display_name,
        format: parsed.format,
        gguf,
        size_bytes,
        base_model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("dtv-sideload")
            .tempdir_in("/tmp")
            .unwrap()
    }

    fn write_model(root: &Path, rel: &str, files: &[&str]) {
        let d = root.join(rel);
        std::fs::create_dir_all(&d).unwrap();
        for f in files {
            std::fs::write(d.join(f), b"GGUF").unwrap();
        }
    }

    #[test]
    fn the_macos_manifest_shape_resolves_to_a_fused_gguf() {
        let t = dir();
        write_model(
            t.path(),
            "qwen3-1.7b-private-best",
            &["qwen3-1.7b-private-best-Q4_K_M.gguf", "config.json"],
        );
        std::fs::write(
            t.path().join("current.json"),
            r#"{"id":"qwen3-1.7b-private-best","displayName":"Current 1.7B Experiment","relativeModelPath":"qwen3-1.7b-private-best"}"#,
        )
        .unwrap();
        let s = resolve(t.path(), "current").unwrap();
        assert_eq!(s.id, "qwen3-1.7b-private-best");
        assert_eq!(s.display_name, "Current 1.7B Experiment");
        assert_eq!(s.format, Format::Gguf);
        assert_eq!(s.base_model, None);
        assert_eq!(s.size_bytes, 4);
        assert!(
            s.gguf
                .ends_with("qwen3-1.7b-private-best/qwen3-1.7b-private-best-Q4_K_M.gguf")
        );
        assert_eq!(s.manifest, t.path().join("current.json"));
    }

    #[test]
    fn the_lora_form_names_its_base_and_needs_one() {
        let t = dir();
        write_model(t.path(), "tuned/adapter", &["adapter.gguf"]);
        std::fs::write(
            t.path().join("lora.json"),
            r#"{"id":"tuned","displayName":"Tuned","relativeModelPath":"tuned/adapter","format":"gguf-lora","baseModel":"qwen3-1.7b"}"#,
        )
        .unwrap();
        let s = resolve(t.path(), "lora").unwrap();
        assert_eq!(s.format, Format::GgufLora);
        assert_eq!(s.base_model.as_deref(), Some("qwen3-1.7b"));
        assert!(s.gguf.ends_with("tuned/adapter/adapter.gguf"));

        std::fs::write(
            t.path().join("nobase.json"),
            r#"{"id":"tuned","displayName":"Tuned","relativeModelPath":"tuned/adapter","format":"gguf-lora"}"#,
        )
        .unwrap();
        let e = resolve(t.path(), "nobase").unwrap_err();
        assert!(e.message.contains("baseModel"), "{e}");
        assert_eq!(e.name, "nobase");
    }

    #[test]
    fn every_failure_names_what_is_wrong() {
        let t = dir();
        let missing = resolve(t.path(), "current").unwrap_err();
        assert!(missing.message.contains("current.json"), "{missing}");
        assert!(
            missing
                .to_string()
                .starts_with("polish experiment current:")
        );

        std::fs::write(t.path().join("bad.json"), "{").unwrap();
        assert!(
            resolve(t.path(), "bad")
                .unwrap_err()
                .message
                .contains("is not a manifest")
        );

        std::fs::write(
            t.path().join("nodir.json"),
            r#"{"id":"x","displayName":"X","relativeModelPath":"absent"}"#,
        )
        .unwrap();
        assert!(
            resolve(t.path(), "nodir")
                .unwrap_err()
                .message
                .contains("is not a directory")
        );

        write_model(t.path(), "empty", &["notes.txt"]);
        std::fs::write(
            t.path().join("empty.json"),
            r#"{"id":"x","displayName":"X","relativeModelPath":"empty"}"#,
        )
        .unwrap();
        assert!(
            resolve(t.path(), "empty")
                .unwrap_err()
                .message
                .contains("holds no .gguf")
        );

        write_model(t.path(), "two", &["a.gguf", "b.gguf"]);
        std::fs::write(
            t.path().join("two.json"),
            r#"{"id":"x","displayName":"X","relativeModelPath":"two"}"#,
        )
        .unwrap();
        let two = resolve(t.path(), "two").unwrap_err();
        assert!(two.message.contains("holds 2 .gguf files"), "{two}");

        std::fs::write(
            t.path().join("escape.json"),
            r#"{"id":"x","displayName":"X","relativeModelPath":"../two"}"#,
        )
        .unwrap();
        assert!(
            resolve(t.path(), "escape")
                .unwrap_err()
                .message
                .contains("must be a relative path inside")
        );
        assert!(
            resolve(t.path(), "")
                .unwrap_err()
                .message
                .contains("no manifest name")
        );
    }

    #[test]
    fn a_name_with_the_extension_is_taken_as_is() {
        assert_eq!(
            manifest_path(Path::new("/x"), "current"),
            PathBuf::from("/x/current.json")
        );
        assert_eq!(
            manifest_path(Path::new("/x"), "trial.json"),
            PathBuf::from("/x/trial.json")
        );
    }
}

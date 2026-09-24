//! The model catalogue (FR-E4): a versioned manifest compiled into the
//! crate, one entry per model with its source, checksum, size, license and
//! redistribution note. A test keeps every entry complete and every source
//! on an allowlisted host.

use serde::{Deserialize, Serialize};

/// The catalogue shipped with this build.
pub const CATALOGUE_V1: &str = include_str!("../catalogue/v1.toml");

/// Hosts a catalogue entry may download from.
pub const ALLOWED_HOSTS: &[&str] = &["huggingface.co", "github.com"];

/// What a model is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Speech to text.
    Stt,
    /// Voice activity detection.
    Vad,
    /// Speaker segmentation or embedding.
    Speaker,
    /// A language model in GGUF for the Enhanced rewrite and analysis.
    Llm,
    /// A speaker diarization model set (segmentation plus embedding).
    Diarization,
}

/// An archive member a downloaded file is unpacked to: the download is
/// verified against the entry's checksum as it arrived, then `member` is
/// extracted to the entry's `file_name` and verified against `sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unpack {
    /// The archive format; `tar.bz2` is the one supported.
    pub format: String,
    /// The member path inside the archive.
    pub member: String,
    /// The checksum of the extracted member.
    pub sha256: String,
    /// The size of the extracted member.
    pub size_bytes: u64,
}

/// One file of a model set: the entry's own fields describe the first,
/// `files` the rest, and `ModelEntry::files` lists them all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    /// The file name inside the model directory.
    pub file_name: String,
    /// The size of the download.
    pub size_bytes: u64,
    /// Where the file comes from.
    pub url: String,
    /// The checksum the download is verified against.
    pub sha256: String,
    /// The license identifier.
    pub license: String,
    /// The archive member to unpack, for a download that is an archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unpack: Option<Unpack>,
}

impl ModelFile {
    /// The checksum the file on disk must have (the member's for an
    /// unpacked archive, the download's otherwise).
    pub fn disk_sha256(&self) -> &str {
        self.unpack
            .as_ref()
            .map(|u| u.sha256.as_str())
            .unwrap_or(&self.sha256)
    }
}

/// One catalogue entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    /// The provider directory under the models directory.
    pub provider: String,
    /// The model id, portable across the ports.
    pub id: String,
    /// The name the GUI and the CLI show.
    pub display_name: String,
    /// What the model is for.
    pub kind: ModelKind,
    /// The file name inside `<models>/<provider>/<id>/`.
    pub file_name: String,
    /// The file size, for progress and free-space checks.
    pub size_bytes: u64,
    /// Where the file comes from.
    pub url: String,
    /// The checksum the download is verified against.
    pub sha256: String,
    /// The license identifier.
    pub license: String,
    /// How the weights may be redistributed.
    pub redistribution: String,
    /// `["en"]`, or `["multilingual"]`.
    pub languages: Vec<String>,
    /// English-only weights.
    #[serde(default)]
    pub english_only: bool,
    /// The quantization, when not the source precision.
    #[serde(default)]
    pub quantization: Option<String>,
    /// What a language model is meant for (`polish`, `analysis`), so a
    /// client can offer the fast, quality and analysis options by role.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    /// The provider's default model.
    #[serde(default)]
    pub default: bool,
    /// Offered for download; `false` while the engine that reads it is
    /// not shipped yet.
    #[serde(default = "yes")]
    pub available: bool,
    /// One line on who the model suits; the models first run offers
    /// carry one (`docs/app.md`), the rest none.
    #[serde(default)]
    pub recommended_for: Option<String>,
    /// The archive member the first file is unpacked to, when the
    /// download is an archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unpack: Option<Unpack>,
    /// The further files of a multi-file model set (the diarization
    /// model set: the embedding model beside the segmentation model).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<ModelFile>,
}

impl ModelEntry {
    /// Every file of the model set, the entry's own first.
    pub fn files(&self) -> Vec<ModelFile> {
        let mut out = vec![ModelFile {
            file_name: self.file_name.clone(),
            size_bytes: self.size_bytes,
            url: self.url.clone(),
            sha256: self.sha256.clone(),
            license: self.license.clone(),
            unpack: self.unpack.clone(),
        }];
        out.extend(self.files.iter().cloned());
        out
    }

    /// Bytes every download of the set transfers.
    pub fn download_bytes(&self) -> u64 {
        self.files().iter().map(|f| f.size_bytes).sum()
    }

    /// The engine loads a directory rather than one file (a model set).
    pub fn loads_directory(&self) -> bool {
        self.kind == ModelKind::Diarization
    }
}

fn yes() -> bool {
    true
}

/// The catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// The manifest version.
    pub version: u32,
    /// Every model.
    pub models: Vec<ModelEntry>,
}

impl Catalogue {
    /// Parses a catalogue document.
    pub fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        c.validate()?;
        Ok(c)
    }

    /// The catalogue compiled into this build.
    pub fn builtin() -> Self {
        Self::parse(CATALOGUE_V1).expect("the built-in catalogue is valid (tested)")
    }

    /// Every entry has every field, a 64-hex checksum, a size and a source
    /// on an allowlisted host; provider and id pairs are unique and every
    /// provider has exactly one default.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        let mut defaults = std::collections::BTreeMap::<&str, u32>::new();
        for m in &self.models {
            let name = format!("{}/{}", m.provider, m.id);
            for (field, value) in [
                ("provider", &m.provider),
                ("id", &m.id),
                ("display_name", &m.display_name),
                ("redistribution", &m.redistribution),
            ] {
                if value.trim().is_empty() {
                    return Err(format!("{name}: {field} is empty"));
                }
            }
            if m.languages.is_empty() {
                return Err(format!("{name}: languages is empty"));
            }
            if m.provider.contains('/') || m.id.contains('/') {
                return Err(format!("{name}: names may not contain '/'"));
            }
            let mut file_names = std::collections::BTreeSet::new();
            for f in m.files() {
                validate_file(&name, &f)?;
                if !file_names.insert(f.file_name.clone()) {
                    return Err(format!("{name}: file {} listed twice", f.file_name));
                }
            }
            if !seen.insert(name.clone()) {
                return Err(format!("{name}: listed twice"));
            }
            if m.default {
                *defaults.entry(m.provider.as_str()).or_default() += 1;
            }
        }
        for provider in self.providers() {
            if defaults.get(provider.as_str()).copied().unwrap_or(0) != 1 {
                return Err(format!("{provider}: exactly one default model is required"));
            }
        }
        Ok(())
    }

    /// The providers with a model the `speech.models.*` methods may fetch
    /// (everything but the language models, which `llm.models.*` serve).
    pub fn download_providers(&self) -> Vec<String> {
        self.providers()
            .into_iter()
            .filter(|p| {
                self.models
                    .iter()
                    .any(|m| &m.provider == p && m.kind != ModelKind::Llm)
            })
            .collect()
    }

    /// One entry by provider and id.
    pub fn find(&self, provider: &str, id: &str) -> Option<&ModelEntry> {
        self.models
            .iter()
            .find(|m| m.provider == provider && m.id == id)
    }

    /// The providers, in catalogue order.
    pub fn providers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for m in &self.models {
            if !out.contains(&m.provider) {
                out.push(m.provider.clone());
            }
        }
        out
    }

    /// The providers with a speech-to-text model, in catalogue order (the
    /// `speech.*` methods and `[speech] provider` know these only).
    pub fn stt_providers(&self) -> Vec<String> {
        self.providers()
            .into_iter()
            .filter(|p| {
                self.models
                    .iter()
                    .any(|m| &m.provider == p && m.kind == ModelKind::Stt)
            })
            .collect()
    }

    /// The provider's default model.
    pub fn default_for(&self, provider: &str) -> Option<&ModelEntry> {
        self.models
            .iter()
            .find(|m| m.provider == provider && m.default)
    }

    /// The entries of one provider.
    pub fn models_of(&self, provider: &str) -> Vec<&ModelEntry> {
        self.models
            .iter()
            .filter(|m| m.provider == provider)
            .collect()
    }
}

/// One file's fields: a name without a slash, a 64-hex checksum, a size,
/// a license and a source on an allowlisted host (loopback over plain
/// HTTP is allowed for fixture servers and local mirrors); an unpack
/// step names a supported format and a member with its own checksum.
fn validate_file(name: &str, f: &ModelFile) -> Result<(), String> {
    let name = format!("{name} ({})", f.file_name);
    for (field, value) in [
        ("file_name", &f.file_name),
        ("url", &f.url),
        ("sha256", &f.sha256),
        ("license", &f.license),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{name}: {field} is empty"));
        }
    }
    let hex = |s: &str| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
    if !hex(&f.sha256) {
        return Err(format!("{name}: sha256 is not 64 hex characters"));
    }
    if f.size_bytes == 0 {
        return Err(format!("{name}: size_bytes is zero"));
    }
    let loopback = f.url.starts_with("http://127.0.0.1:") || f.url.starts_with("http://[::1]:");
    let host = f
        .url
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");
    if !loopback && !ALLOWED_HOSTS.contains(&host) {
        return Err(format!("{name}: url host {host} is not allowlisted"));
    }
    if f.file_name.contains('/') {
        return Err(format!("{name}: names may not contain '/'"));
    }
    if let Some(u) = &f.unpack {
        if u.format != "tar.bz2" {
            return Err(format!("{name}: unpack format {} is not tar.bz2", u.format));
        }
        if u.member.trim().is_empty() || u.member.starts_with('/') || u.member.contains("..") {
            return Err(format!("{name}: unpack member is not a plain archive path"));
        }
        if !hex(&u.sha256) || u.size_bytes == 0 {
            return Err(format!("{name}: unpack needs a 64-hex sha256 and a size"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibrated_diarization_has_its_own_model_identity() {
        let catalogue = Catalogue::builtin();
        let model = catalogue.find("diarize", "diarization-en").unwrap();
        let legacy = catalogue.find("diarize", "diarization").unwrap();
        assert!(model.default && !legacy.default);
        assert_eq!(model.files().len(), 2);
        assert_eq!(
            model.files()[0].disk_sha256(),
            legacy.files()[0].disk_sha256()
        );
        assert_eq!(
            model.files()[1].sha256,
            "c59158379255ad66e161679cca6af8d52d51e389e3224ab7d7a7baae295c2db5"
        );
        assert_eq!(
            legacy.files()[1].sha256,
            "1a331345f04805badbb495c775a6ddffcdd1a732567d5ec8b3d5749e3c7a5e4b"
        );
    }

    #[test]
    fn the_diarization_model_set_lists_both_files() {
        let c = Catalogue::builtin();
        let d = c.find("diarize", "diarization").unwrap();
        assert_eq!(d.kind, ModelKind::Diarization);
        assert!(d.loads_directory() && !d.default && d.available);
        let files = d.files();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].file_name, "segmentation.onnx");
        assert_eq!(
            files[0].unpack.as_ref().unwrap().member,
            "sherpa-onnx-pyannote-segmentation-3-0/model.onnx"
        );
        assert_ne!(files[0].disk_sha256(), files[0].sha256);
        assert_eq!(files[1].file_name, "embedding.onnx");
        assert_eq!(files[1].disk_sha256(), files[1].sha256);
        assert_eq!(files[1].license, "Apache-2.0");
        assert_eq!(
            d.download_bytes(),
            files[0].size_bytes + files[1].size_bytes
        );
        assert_eq!(c.download_providers(), ["whisper", "parakeet", "diarize"]);
        let mut c = Catalogue::builtin();
        let i = c.models.iter().position(|m| m.id == "diarization").unwrap();
        c.models[i].unpack.as_mut().unwrap().format = "zip".into();
        assert!(c.validate().unwrap_err().contains("not tar.bz2"));
        let mut c = Catalogue::builtin();
        c.models[i].files[0].file_name = "segmentation.onnx".into();
        assert!(c.validate().unwrap_err().contains("listed twice"));
    }

    #[test]
    fn the_builtin_catalogue_is_complete() {
        let c = Catalogue::builtin();
        assert_eq!(c.version, 1);
        let ids: Vec<&str> = c
            .models_of("whisper")
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        for want in [
            "tiny",
            "tiny.en",
            "base",
            "base.en",
            "small",
            "small.en",
            "medium",
            "medium.en",
            "large-v3",
            "large-v3-turbo",
            "silero-vad",
        ] {
            assert!(ids.contains(&want), "{want} missing");
        }
        assert_eq!(c.default_for("whisper").unwrap().id, "large-v3-turbo");
        assert_eq!(c.default_for("parakeet").unwrap().id, "parakeet-v3");
        assert!(c.models_of("parakeet").iter().all(|m| m.available));
        let v2 = c.find("parakeet", "parakeet-v2").unwrap();
        assert!(v2.english_only && v2.file_name.ends_with(".gguf"));
        assert_eq!(c.providers(), ["whisper", "parakeet", "llm", "diarize"]);
        assert_eq!(c.stt_providers(), ["whisper", "parakeet"]);
        assert!(c.find("whisper", "tiny.en").unwrap().english_only);
    }

    #[test]
    fn the_llm_catalogue_lists_the_macos_models_in_gguf() {
        let c = Catalogue::builtin();
        let ids: Vec<&str> = c.models_of("llm").iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "qwen3-4b-instruct-2507",
                "qwen3-1.7b",
                "qwen3-4b",
                "qwen3-8b"
            ]
        );
        assert_eq!(c.default_for("llm").unwrap().id, "qwen3-4b-instruct-2507");
        for m in c.models_of("llm") {
            assert_eq!(m.kind, ModelKind::Llm, "{}", m.id);
            assert!(m.file_name.ends_with(".gguf"), "{}", m.id);
            assert_eq!(m.quantization.as_deref(), Some("Q4_K_M"), "{}", m.id);
            assert!(!m.roles.is_empty(), "{} names its roles", m.id);
            assert!(m.available, "{}", m.id);
        }
        assert!(
            c.find("llm", "qwen3-8b")
                .unwrap()
                .roles
                .contains(&"analysis".to_string())
        );
    }

    #[test]
    fn a_malformed_entry_names_itself() {
        let mut c = Catalogue::builtin();
        c.models[0].sha256 = "abc".into();
        let err = c.validate().unwrap_err();
        assert!(err.starts_with("whisper/tiny (ggml-tiny.bin):"), "{err}");
        let mut c = Catalogue::builtin();
        c.models[1].url = "https://example.com/x.bin".into();
        assert!(c.validate().unwrap_err().contains("not allowlisted"));
        let mut c = Catalogue::builtin();
        c.models[2].license.clear();
        assert!(c.validate().unwrap_err().contains("license is empty"));
        let mut c = Catalogue::builtin();
        c.models[3].default = true;
        assert!(c.validate().unwrap_err().contains("exactly one default"));
        assert!(Catalogue::parse("version = 1\n[[models]]\nprovider = \"x\"\n").is_err());
    }
}

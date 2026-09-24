//! What the report records about the machine and the inputs: the host
//! (CPU model, the GPU from the Vulkan enumeration, memory, kernel), the
//! git commit, the engine binaries, the models resolved per provider from
//! the catalogue with their checksums, and the fixtures with their hashes.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::scenarios;

/// The catalogue the daemon ships, relative to the repository root.
pub const CATALOGUE: &str = "crates/dettivo-speech/catalogue/v1.toml";

/// The GPU as `vulkaninfo --summary` names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gpu {
    /// The device name.
    pub name: String,
    /// The driver name.
    pub driver: String,
    /// The driver version string.
    pub driver_info: String,
    /// `discrete`, `integrated` or the raw type.
    pub kind: String,
}

/// The machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    /// The host name.
    pub hostname: String,
    /// The CPU model.
    pub cpu: String,
    /// Physical memory in bytes.
    pub memory_bytes: u64,
    /// The kernel release.
    pub kernel: String,
    /// The GPU the Vulkan enumeration lists first among discrete devices.
    pub gpu: Option<Gpu>,
    /// Why no GPU is listed.
    pub gpu_reason: Option<String>,
}

impl Host {
    /// Reads the machine.
    pub fn detect() -> Self {
        let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
        let cpu = read("/proc/cpuinfo")
            .lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()))
            .unwrap_or_else(|| "unknown".into());
        let memory_bytes = read("/proc/meminfo")
            .lines()
            .find(|l| l.starts_with("MemTotal:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<u64>().ok())
            .map(|kb| kb * 1024)
            .unwrap_or(0);
        let (gpu, gpu_reason) = match Command::new("vulkaninfo").arg("--summary").output() {
            Ok(o) if o.status.success() => {
                let gpu = vulkan_gpu(&String::from_utf8_lossy(&o.stdout));
                let reason = gpu
                    .is_none()
                    .then(|| "vulkaninfo lists no device".to_string());
                (gpu, reason)
            }
            Ok(o) => (
                None,
                Some(format!(
                    "vulkaninfo failed: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                )),
            ),
            Err(e) => (None, Some(format!("vulkaninfo: {e}"))),
        };
        Self {
            hostname: read("/etc/hostname").trim().to_string(),
            cpu,
            memory_bytes,
            kernel: read("/proc/sys/kernel/osrelease").trim().to_string(),
            gpu,
            gpu_reason,
        }
    }
}

/// The first discrete device in `vulkaninfo --summary` output, or the
/// first device.
pub fn vulkan_gpu(summary: &str) -> Option<Gpu> {
    let mut devices: Vec<Gpu> = Vec::new();
    let mut current: Option<Gpu> = None;
    let field = |line: &str, key: &str| -> Option<String> {
        let (k, v) = line.trim().split_once('=')?;
        (k.trim() == key).then(|| v.trim().to_string())
    };
    for line in summary.lines() {
        if let Some(kind) = field(line, "deviceType") {
            if let Some(done) = current.take() {
                devices.push(done);
            }
            current = Some(Gpu {
                name: String::new(),
                driver: String::new(),
                driver_info: String::new(),
                kind: match kind.as_str() {
                    "PHYSICAL_DEVICE_TYPE_DISCRETE_GPU" => "discrete".into(),
                    "PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU" => "integrated".into(),
                    "PHYSICAL_DEVICE_TYPE_CPU" => "cpu".into(),
                    other => other.to_lowercase(),
                },
            });
        } else if let Some(gpu) = current.as_mut() {
            if let Some(v) = field(line, "deviceName") {
                gpu.name = v;
            } else if let Some(v) = field(line, "driverName") {
                gpu.driver = v;
            } else if let Some(v) = field(line, "driverInfo") {
                gpu.driver_info = v;
            }
        }
    }
    if let Some(done) = current.take() {
        devices.push(done);
    }
    devices
        .iter()
        .find(|d| d.kind == "discrete")
        .or_else(|| devices.first())
        .cloned()
}

/// The commit the tree is at (`unknown` outside git).
pub fn git_sha(repo_root: &Path) -> String {
    Command::new("git")
        .args(["-C", &repo_root.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// One engine binary measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineBinary {
    /// The binary name.
    pub binary: String,
    /// Its path.
    pub path: PathBuf,
}

/// The three engine binaries, from `engines_dir` or the workspace build;
/// a binary that is not built is left out (the steps skip naming it).
pub fn engine_binaries(repo_root: &Path, engines_dir: Option<&Path>) -> Vec<EngineBinary> {
    [
        "dettivo-engine-whisper",
        "dettivo-engine-parakeet",
        "dettivo-engine-llm",
    ]
    .iter()
    .filter_map(|name| {
        let path = match engines_dir {
            Some(dir) => Some(dir.join(name)).filter(|p| p.is_file()),
            None => scenarios::binary(repo_root, name).ok(),
        }?;
        Some(EngineBinary {
            binary: (*name).to_string(),
            path,
        })
    })
    .collect()
}

/// One model measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    /// The provider.
    pub provider: String,
    /// The catalogue id.
    pub id: String,
    /// The file.
    pub path: PathBuf,
    /// The catalogue's SHA-256.
    pub sha256: String,
}

impl ModelRef {
    /// The command that fetches this model.
    pub fn download_command(provider: &str, id: &str) -> String {
        match provider {
            "whisper" => format!("dettivo speech download --model {id}"),
            "llm" => format!("dettivo llm download --model {id}"),
            other => format!("dettivo speech download --provider {other} --model {id}"),
        }
    }
}

/// The model per provider the suite measures: the product's default
/// when it is on disk, the test model otherwise, in this order.
pub const PREFERRED: &[(&str, &[&str])] = &[
    ("whisper", &["large-v3-turbo", "tiny.en"]),
    ("parakeet", &["parakeet-v3", "parakeet-v2"]),
    ("llm", &["qwen3-4b-instruct-2507", "qwen3-1.7b"]),
];

/// The models resolved per provider.
#[derive(Debug, Clone, Default)]
pub struct Models {
    /// The provider and the model found, or the download that would
    /// fetch the preferred one.
    pub found: Vec<(String, Result<ModelRef, String>)>,
}

impl Models {
    /// Resolves every provider from the catalogue and `models_dir`.
    pub fn resolve(repo_root: &Path, models_dir: Option<&Path>) -> Result<Self, String> {
        let catalogue_path = repo_root.join(CATALOGUE);
        let text = std::fs::read_to_string(&catalogue_path)
            .map_err(|e| format!("{}: {e}", catalogue_path.display()))?;
        let catalogue: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
        let entries = catalogue["models"].as_array().cloned().unwrap_or_default();
        let mut found = Vec::new();
        for (provider, ids) in PREFERRED {
            let hit = ids.iter().find_map(|id| {
                let entry = entries.iter().find(|m| {
                    m["provider"].as_str() == Some(provider) && m["id"].as_str() == Some(id)
                })?;
                let path = models_dir?
                    .join(provider)
                    .join(id)
                    .join(entry["file_name"].as_str()?);
                path.is_file().then(|| ModelRef {
                    provider: (*provider).to_string(),
                    id: (*id).to_string(),
                    path,
                    sha256: entry["sha256"].as_str().unwrap_or_default().to_string(),
                })
            });
            found.push((
                (*provider).to_string(),
                hit.ok_or_else(|| {
                    format!(
                        "no {provider} model under the model directory; run `{}`",
                        ModelRef::download_command(provider, ids[0])
                    )
                }),
            ));
        }
        Ok(Self { found })
    }

    /// The model for `provider`, or the download that fetches it.
    pub fn get(&self, provider: &str) -> Result<&ModelRef, String> {
        self.found
            .iter()
            .find(|(p, _)| p == provider)
            .map(|(_, r)| r.as_ref().map_err(Clone::clone))
            .unwrap_or_else(|| Err(format!("no provider {provider}")))
    }

    /// Every model found, for the report.
    pub fn refs(&self) -> Vec<ModelRef> {
        self.found
            .iter()
            .filter_map(|(_, r)| r.as_ref().ok().cloned())
            .collect()
    }
}

/// One fixture and its hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureRef {
    /// The fixture's name.
    pub name: String,
    /// Its path.
    pub path: PathBuf,
    /// SHA-256 of the file.
    pub sha256: String,
    /// Size in bytes.
    pub bytes: u64,
}

/// Hashes `path`.
pub fn fixture_ref(name: &str, path: &Path) -> Option<FixtureRef> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(FixtureRef {
        name: name.into(),
        path: path.to_path_buf(),
        sha256: format!("{:x}", hasher.finalize()),
        bytes: bytes.len() as u64,
    })
}

/// The fixtures the run used: the speech clip and the long WAV built
/// from it, when they exist.
pub fn fixtures(bench: &super::Bench) -> Vec<FixtureRef> {
    [
        ("jfk.wav", bench.fixture.clone()),
        ("long.wav", bench.run_dir.join("long.wav")),
        (
            "polish-goldens",
            bench.opts.repo_root.join(super::throughput::GOLDENS),
        ),
    ]
    .iter()
    .filter_map(|(name, path)| fixture_ref(name, path))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_discrete_gpu_is_read_from_vulkaninfo() {
        let summary = "Devices:\n========\nGPU0:\n\tapiVersion         = 1.4.312\n\tdeviceType         = PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU\n\tdeviceName         = AMD Ryzen (RADV)\n\tdriverName         = radv\n\tdriverInfo         = Mesa 26.2\nGPU1:\n\tdeviceType         = PHYSICAL_DEVICE_TYPE_DISCRETE_GPU\n\tdeviceName         = NVIDIA GeForce RTX 4090\n\tdriverName         = NVIDIA\n\tdriverInfo         = 610.57.04\n";
        let gpu = vulkan_gpu(summary).unwrap();
        assert_eq!(gpu.name, "NVIDIA GeForce RTX 4090");
        assert_eq!(
            (gpu.driver.as_str(), gpu.kind.as_str()),
            ("NVIDIA", "discrete")
        );
        let only = vulkan_gpu(
            "\tdeviceType         = PHYSICAL_DEVICE_TYPE_CPU\n\tdeviceName         = llvmpipe\n",
        )
        .unwrap();
        assert_eq!(
            (only.name.as_str(), only.kind.as_str()),
            ("llvmpipe", "cpu")
        );
        assert!(vulkan_gpu("").is_none());
    }

    #[test]
    fn models_resolve_the_preferred_file_and_name_the_download() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join("crates/dettivo-speech/catalogue")).unwrap();
        std::fs::write(
            root.join(CATALOGUE),
            "[[models]]\nprovider = \"whisper\"\nid = \"tiny.en\"\nfile_name = \"ggml-tiny.en.bin\"\nsha256 = \"abc\"\n[[models]]\nprovider = \"whisper\"\nid = \"large-v3-turbo\"\nfile_name = \"ggml-large-v3-turbo.bin\"\nsha256 = \"def\"\n",
        )
        .unwrap();
        let models = dir.path().join("models");
        std::fs::create_dir_all(models.join("whisper/tiny.en")).unwrap();
        std::fs::write(models.join("whisper/tiny.en/ggml-tiny.en.bin"), b"x").unwrap();
        let resolved = Models::resolve(&root, Some(&models)).unwrap();
        let whisper = resolved.get("whisper").unwrap();
        assert_eq!(
            (whisper.id.as_str(), whisper.sha256.as_str()),
            ("tiny.en", "abc")
        );
        let parakeet = resolved.get("parakeet").unwrap_err();
        assert!(
            parakeet.contains("dettivo speech download --provider parakeet --model parakeet-v3"),
            "{parakeet}"
        );
        assert!(
            resolved
                .get("llm")
                .unwrap_err()
                .contains("dettivo llm download --model qwen3-4b-instruct-2507")
        );
        assert_eq!(resolved.refs().len(), 1);
        let fixture =
            fixture_ref("tiny", &models.join("whisper/tiny.en/ggml-tiny.en.bin")).unwrap();
        assert_eq!(fixture.bytes, 1);
        assert_eq!(fixture.sha256.len(), 64);
    }
}

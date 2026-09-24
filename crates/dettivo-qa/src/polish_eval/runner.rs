//! The runner: a daemon of its own in an isolated profile with `[llm]
//! provider = "local"` and the candidate model, every row through
//! `polish.test` in `enhanced` mode so the prompt profile, the guards,
//! the repair pass and the fallback are the product's, and the engine's
//! backend read back once the first row has loaded it. The candidate is
//! a catalogue id (its files hard-linked from the machine's model
//! directory, never the directory itself), a sideload manifest name (the
//! experiments directory read where it is), a GGUF path (wrapped in a
//! manifest of its own), or a `fixture:<dir>` / `echo` mock for a run
//! without a model.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::dataset::Sample;
use super::scorer::Runtime;
use crate::{
    profile::{ModelSource, Profile},
    replay, scenarios,
};

/// What the run measures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    /// A catalogue language model by id.
    Catalogue(String),
    /// The sideload a manifest under the experiments directory names.
    Sideload(String),
    /// One GGUF file, run as a sideload of its own.
    Path(PathBuf),
    /// `DETTIVO_MOCK_LLM=fixture:<dir>`.
    Fixture(PathBuf),
    /// `DETTIVO_MOCK_LLM=echo`.
    Echo,
}

impl Candidate {
    /// Parses `--model`: `fixture:<dir>`, `echo`, `sideload` (the
    /// `current` manifest), `sideload:<name>`, a path ending in `.gguf`,
    /// or a catalogue id.
    pub fn parse(text: &str) -> Self {
        let t = text.trim();
        if t == "echo" {
            return Self::Echo;
        }
        if let Some(dir) = t.strip_prefix("fixture:") {
            return Self::Fixture(PathBuf::from(dir));
        }
        if t == "sideload" {
            return Self::Sideload("current".into());
        }
        if let Some(name) = t.strip_prefix("sideload:") {
            return Self::Sideload(name.to_string());
        }
        if t.ends_with(".gguf") || t.contains('/') {
            return Self::Path(PathBuf::from(t));
        }
        Self::Catalogue(t.to_string())
    }

    /// The id a report names.
    pub fn id(&self) -> String {
        match self {
            Self::Catalogue(id) => id.clone(),
            Self::Sideload(name) => format!("sideload:{name}"),
            Self::Path(p) => p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "gguf".into()),
            Self::Fixture(_) => "mock-fixture".into(),
            Self::Echo => "mock-echo".into(),
        }
    }

    /// Whether a real engine runs.
    pub fn is_model(&self) -> bool {
        !matches!(self, Self::Fixture(_) | Self::Echo)
    }
}

/// How the daemon is set up.
#[derive(Debug, Clone)]
pub struct Options {
    /// The candidate.
    pub candidate: Candidate,
    /// Pin the engine to the CPU.
    pub cpu: bool,
    /// The experiments directory a sideload reads; the machine's default
    /// when `None`.
    pub experiments_dir: Option<PathBuf>,
    /// The machine's model directory (`$XDG_DATA_HOME/dettivo/models`).
    pub models_dir: Option<PathBuf>,
    /// `[llm] timeout_ms` for the run.
    pub timeout_ms: u64,
    /// The directory holding `dettivo-engine-llm` (`[engines] directory`);
    /// the daemon's own directory when `None`. A Vulkan build of the
    /// engine kept beside a CPU workspace build goes here.
    pub engines_dir: Option<PathBuf>,
}

/// The daemon of a run.
pub struct Runner {
    profile: Profile,
    child: Child,
    socket: PathBuf,
    candidate: Candidate,
    timeout_ms: u64,
    engine_dir: PathBuf,
    /// The model file the engine reports, when a real one ran.
    pub model_file: Option<String>,
}

/// Hard-links (or symlinks, across filesystems) every file of
/// `<models>/llm/<id>/` into the profile's own model tree.
/// The `baseModel` a manifest names, when it is a `gguf-lora`.
fn manifest_base(dir: &Path, name: &str) -> Option<String> {
    let file = if name.ends_with(".json") {
        dir.join(name)
    } else {
        dir.join(format!("{name}.json"))
    };
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
    (v["format"] == "gguf-lora")
        .then(|| v["baseModel"].as_str().map(str::to_string))
        .flatten()
}

impl Runner {
    /// Starts the daemon for `opts`.
    pub fn start(repo: &Path, opts: &Options) -> Result<Self, String> {
        let daemon_bin = scenarios::binary(repo, "dettivod")?;
        let engine_dir = match &opts.engines_dir {
            Some(dir) => dir.clone(),
            None => daemon_bin
                .parent()
                .map(Path::to_path_buf)
                .ok_or("dettivod has no parent directory")?,
        };
        if opts.candidate.is_model() && !engine_dir.join("dettivo-engine-llm").is_file() {
            return Err(format!(
                "dettivo-engine-llm is not built beside {}",
                daemon_bin.display()
            ));
        }
        let source = opts
            .models_dir
            .clone()
            .map(|real| ModelSource::new(real, repo));
        let mut profile =
            Profile::create("polish-eval", source.as_ref()).map_err(|e| e.to_string())?;
        let root = profile.root.clone();
        let mut llm = vec![
            "provider = \"local\"".to_string(),
            "ollama_url = \"http://127.0.0.1:1\"".to_string(),
            format!("timeout_ms = {}", opts.timeout_ms),
        ];
        let mut env = profile.env();
        match &opts.candidate {
            Candidate::Catalogue(id) => {
                profile.link_model(&format!("llm/{id}"))?;
                llm.push(format!("model = {id:?}"));
            }
            Candidate::Sideload(name) => {
                let dir = match &opts.experiments_dir {
                    Some(d) => d.clone(),
                    None => opts
                        .models_dir
                        .as_deref()
                        .ok_or("no model directory")?
                        .join("polish-experiments"),
                };
                if let Some(base) = manifest_base(&dir, name) {
                    profile.link_model(&format!("llm/{base}"))?;
                }
                llm.push(format!("polish_experiment = {name:?}"));
                llm.push(format!("experiments_dir = {:?}", dir.display().to_string()));
            }
            Candidate::Path(gguf) => {
                let gguf =
                    std::fs::canonicalize(gguf).map_err(|e| format!("{}: {e}", gguf.display()))?;
                let exp = root.join("data/dettivo/models/polish-experiments");
                let dir = exp.join("eval-candidate");
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                let name = gguf.file_name().ok_or("the GGUF path has no file name")?;
                std::os::unix::fs::symlink(&gguf, dir.join(name))
                    .map_err(|e| format!("{}: {e}", dir.display()))?;
                let id = opts.candidate.id();
                std::fs::write(
                    exp.join("eval.json"),
                    json!({"id": id, "displayName": id, "relativeModelPath": "eval-candidate"})
                        .to_string(),
                )
                .map_err(|e| format!("{}: {e}", exp.display()))?;
                llm.push("polish_experiment = \"eval\"".into());
            }
            Candidate::Fixture(dir) => {
                let dir =
                    std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                env.insert(
                    "DETTIVO_MOCK_LLM".into(),
                    format!("fixture:{}", dir.display()),
                );
            }
            Candidate::Echo => {
                env.insert("DETTIVO_MOCK_LLM".into(), "echo".into());
            }
        }
        let config = format!(
            "[hotkeys]\nbackend = \"none\"\n[engines]\ndirectory = {:?}\n[engines.llm]\nbackend = {:?}\n[llm]\n{}\n",
            engine_dir.display().to_string(),
            if opts.cpu { "cpu" } else { "auto" },
            llm.join("\n")
        );
        std::fs::write(root.join("cfg/dettivo/config.toml"), config)
            .map_err(|e| format!("config: {e}"))?;
        let child = crate::profile::command(&daemon_bin, &env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("start dettivod: {e}"))?;
        let socket = profile.socket();
        let runner = Self {
            profile,
            child,
            socket,
            candidate: opts.candidate.clone(),
            timeout_ms: opts.timeout_ms,
            engine_dir,
            model_file: None,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !replay::answers_ping(&runner.socket) {
            if Instant::now() > deadline {
                return Err(format!(
                    "dettivod did not answer on {}",
                    runner.socket.display()
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if runner.candidate.is_model() {
            let providers = runner.call("llm.providers.list", json!({}))?;
            let local = &providers["providers"][0];
            if local["available"] != Value::Bool(true) {
                return Err(format!(
                    "the local provider is not available: {}{}",
                    local["detail"].as_str().unwrap_or_default(),
                    local["hint"]
                        .as_str()
                        .map(|h| format!(" ({h})"))
                        .unwrap_or_default()
                ));
            }
        }
        Ok(runner)
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let line = json!({"jsonrpc": "2.0", "id": "eval", "method": method, "params": params});
        // A row waits for the model load on top of its own budget.
        let budget = Duration::from_millis(self.timeout_ms) + Duration::from_secs(180);
        let answer = replay::call_within(&self.socket, &line.to_string(), budget)
            .map_err(|e| e.to_string())?;
        if let Some(err) = answer.get("error") {
            return Err(format!(
                "{method}: {}",
                err["message"].as_str().unwrap_or("error")
            ));
        }
        Ok(answer["result"].clone())
    }

    /// The id the daemon serves the candidate under: the sideload row's
    /// id for a sideload (the manifest's `id`), the candidate's own id
    /// otherwise.
    pub fn served_id(&self) -> String {
        if let Candidate::Sideload(_) | Candidate::Path(_) = &self.candidate {
            if let Ok(status) = self.call("llm.models.status", json!({})) {
                if let Some(row) = status["models"]
                    .as_array()
                    .and_then(|rows| rows.iter().find(|m| m["source"] == "sideload"))
                {
                    if let Some(id) = row["id"].as_str() {
                        return id.to_string();
                    }
                }
            }
        }
        self.candidate.id()
    }

    /// The engine's backend as it reports it (`mock` for a mock run).
    pub fn backend(&mut self) -> String {
        if !self.candidate.is_model() {
            return "mock".into();
        }
        match self.call("llm.engine.status", json!({})) {
            Ok(status) => {
                if let Some(model) = status["model"].as_str() {
                    self.model_file = Some(model.to_string());
                }
                status["backend"].as_str().unwrap_or("unknown").to_string()
            }
            Err(e) => {
                eprintln!("polish-eval: llm.engine.status: {e}");
                "unknown".into()
            }
        }
    }

    /// Runs one row through `polish.test` in `enhanced` mode. An unknown
    /// preset or style fails naming the row.
    pub fn run_row(&self, sample: &Sample) -> Result<Runtime, String> {
        let mut params = json!({
            "input": sample.input,
            "preset": sample.preset,
            "rules": [],
            "mode": "enhanced",
        });
        if let Some(style) = &sample.style {
            params["style"] = json!(style);
        }
        if let Some(app) = &sample.app_bundle_id {
            params["bundle_id"] = json!(app);
        }
        let started = Instant::now();
        let result = self
            .call("polish.test", params)
            .map_err(|e| format!("row {}: {e}", sample.id))?;
        let wall_ms = started.elapsed().as_millis() as u64;
        let notice_kind = result["notice"]["kind"].as_str();
        let notice_reason = result["notice"]["reason"].as_str().unwrap_or_default();
        Ok(Runtime {
            output: result["output"].as_str().unwrap_or_default().to_string(),
            model: result["model"].as_str().unwrap_or_default().to_string(),
            backend: String::new(),
            wall_ms,
            guard_rejection: (notice_kind == Some("guard_rejected"))
                .then(|| notice_reason.to_string()),
            fallback_used: notice_kind.is_some(),
        })
    }

    /// The raw generation path for `--engine-only`: the engine binary the
    /// daemon ran.
    pub fn engine_binary(&self, repo: &Path) -> Result<PathBuf, String> {
        let own = self.engine_dir.join("dettivo-engine-llm");
        if own.is_file() {
            return Ok(own);
        }
        scenarios::binary(repo, "dettivo-engine-llm")
    }

    /// The profile root, for a `--keep-profile` inspection.
    pub fn root(&self) -> &Path {
        &self.profile.root
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .output();
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_argument_names_every_candidate_kind() {
        assert_eq!(
            Candidate::parse("qwen3-4b-instruct-2507"),
            Candidate::Catalogue("qwen3-4b-instruct-2507".into())
        );
        assert_eq!(
            Candidate::parse("sideload"),
            Candidate::Sideload("current".into())
        );
        assert_eq!(
            Candidate::parse("sideload:trial"),
            Candidate::Sideload("trial".into())
        );
        assert_eq!(
            Candidate::parse("/x/tuned.gguf"),
            Candidate::Path(PathBuf::from("/x/tuned.gguf"))
        );
        assert_eq!(Candidate::parse("/x/tuned.gguf").id(), "tuned");
        assert_eq!(
            Candidate::parse("fixture:/f"),
            Candidate::Fixture(PathBuf::from("/f"))
        );
        assert_eq!(Candidate::parse("echo"), Candidate::Echo);
        assert!(!Candidate::Echo.is_model());
        assert!(Candidate::Sideload("current".into()).is_model());
    }
}

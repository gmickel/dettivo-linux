//! QA mode (ADR 0011): the environment variables that make a run
//! deterministic, with the macOS names kept so QA knowledge stays portable
//! across ports. One parser serves the daemon; the Qt hosts mirror the
//! same list in `qt/host/qa_environment.cpp`, and `docs/qa.md` is the
//! table both are checked against.
//!
//! Rules: a hook (`DETTIVO_MOCK_*`, `DETTIVO_E2E_*`, `DETTIVO_FORCE_CPU`,
//! `DETTIVO_MODEL_SERVER`) needs QA mode on; QA mode is refused in a
//! release build unless `DETTIVO_QA_ALLOW_RELEASE=1`; and a variable in
//! the reserved prefixes that is not documented is rejected by name.

use std::ffi::OsString;
use std::path::PathBuf;

/// Whether the binary was built with optimisations (a release build).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildKind {
    /// `cargo build` (debug assertions on).
    Debug,
    /// `cargo build --release`.
    Release,
}

impl BuildKind {
    /// The kind of this binary.
    pub fn current() -> Self {
        if cfg!(debug_assertions) {
            Self::Debug
        } else {
            Self::Release
        }
    }
}

/// How the language model layer is mocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockLlm {
    /// The layer echoes its input.
    Echo,
    /// The layer replays fixture responses from a directory.
    Fixture(PathBuf),
}

/// The GUI route `DETTIVO_E2E_OPEN` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// The home screen.
    Home,
    /// The dictation history.
    History,
    /// One dictation.
    HistoryDetail,
    /// The meetings list.
    Meetings,
    /// The live meeting.
    MeetingLive,
    /// One meeting after the stop.
    MeetingDetail,
    /// Settings, optionally a sub-route (`settings.hotkeys`).
    Settings(Option<String>),
    /// The first-run screens.
    Onboarding,
}

/// The route names `DETTIVO_E2E_OPEN` accepts, as `docs/app.md` lists them.
pub const ROUTES: &[&str] = &[
    "home",
    "history",
    "history.detail",
    "meetings",
    "meetings.live",
    "meetings.detail",
    "settings",
    "settings.<section>",
    "onboarding",
];

/// What `DETTIVO_E2E_DISCLOSURE` seeds into the state file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disclosure {
    /// The meeting disclosure is recorded as acknowledged.
    Acknowledged,
    /// The acknowledgement is cleared, so the first meeting asks.
    Pending,
}

/// A first-run step `DETTIVO_E2E_STEP` starts at (ADR 0009).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Key bindings.
    Keys,
    /// Model download.
    Models,
    /// Try it.
    Try,
}

/// Every QA setting, parsed and validated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QaEnv {
    /// QA mode is on (`DETTIVO_QA_MODE`, `DETTIVO_QA` or `DETTIVO_MOCK_MODE`).
    pub enabled: bool,
    /// `DETTIVO_QA_ALLOW_RELEASE=1`.
    pub allow_release: bool,
    /// `DETTIVO_MOCK_MODE=1`: mock audio, insertion, LLM and downloads together.
    pub mock_mode: bool,
    /// `DETTIVO_MOCK_MIC=<path.wav>`.
    pub mock_mic: Option<PathBuf>,
    /// `DETTIVO_MOCK_SYSTEM_AUDIO=<path.wav>`.
    pub mock_system_audio: Option<PathBuf>,
    /// `DETTIVO_MOCK_INSERT=1`; `DETTIVO_MOCK_MODE=1` implies it unless
    /// `DETTIVO_MOCK_INSERT=0` keeps the real chain.
    pub mock_insert: bool,
    /// `DETTIVO_MOCK_INPUT=1` (accepted for macOS harness compatibility).
    pub mock_input: bool,
    /// `DETTIVO_MOCK_A11Y=1` (accepted for macOS harness compatibility).
    pub mock_a11y: bool,
    /// `DETTIVO_MOCK_LLM=echo|fixture:<dir>`.
    pub mock_llm: Option<MockLlm>,
    /// `DETTIVO_MOCK_ENGINE=<name>=fixture:<dir>[,...]`.
    pub mock_engines: Vec<(String, PathBuf)>,
    /// `DETTIVO_FORCE_CPU=1`.
    pub force_cpu: bool,
    /// `DETTIVO_MODEL_SERVER=<url>`.
    pub model_server: Option<String>,
    /// `DETTIVO_E2E_COMPLETE=1`.
    pub e2e_complete: bool,
    /// `DETTIVO_E2E_STEP=<keys|models|try>`.
    pub e2e_step: Option<Step>,
    /// `DETTIVO_E2E_SEED=1`.
    pub e2e_seed: bool,
    /// `DETTIVO_E2E_OPEN=<route>`.
    pub e2e_open: Option<Route>,
    /// `DETTIVO_E2E_OSD_STATE=<state>`: `dettivo-osd` shows one pill state
    /// without a daemon, for the visual baseline.
    pub e2e_osd_state: Option<String>,
    /// `DETTIVO_E2E_ROUTE=<sub>`.
    pub e2e_route: Option<String>,
    /// `DETTIVO_E2E_EXPORT_DIR=<dir>`: the app writes an export there
    /// instead of asking the portal file chooser (the daemon accepts the
    /// name and ignores it).
    pub e2e_export_dir: Option<PathBuf>,
    /// `DETTIVO_E2E_DISCLOSURE=acknowledged|pending`: the daemon seeds or
    /// clears the meeting disclosure acknowledgement in the state file
    /// at start, so a drive shows the dialog on a fresh profile or skips it.
    pub e2e_disclosure: Option<Disclosure>,
    /// `DETTIVO_QA_PACING=<file>`: a Qt host writes its frame pacing
    /// summary there (the daemon accepts the name and ignores it).
    pub qa_pacing: Option<PathBuf>,
    /// `DETTIVO_QA_PLANT=<basic_control|basic_background|basic_content|default_font>`: a Qt host plants
    /// a wrong-style item so the negative style check is proven.
    pub qa_plant: Option<String>,
    /// `DETTIVO_QA_CANARY=1`: the theme applies a deliberate token
    /// regression so the visual job's canary run fails as expected.
    pub qa_canary: bool,
}

/// A rejected QA environment, always naming the variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QaError {
    /// The variable at fault.
    pub variable: String,
    /// Why.
    pub message: String,
}

impl std::fmt::Display for QaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.variable, self.message)
    }
}

impl std::error::Error for QaError {}

/// The documented variables, exactly as `docs/qa.md` lists them.
pub const KNOWN: &[&str] = &[
    "DETTIVO_QA_MODE",
    "DETTIVO_QA",
    "DETTIVO_QA_ALLOW_RELEASE",
    "DETTIVO_MOCK_MODE",
    "DETTIVO_MOCK_MIC",
    "DETTIVO_MOCK_SYSTEM_AUDIO",
    "DETTIVO_MOCK_INSERT",
    "DETTIVO_MOCK_INPUT",
    "DETTIVO_MOCK_A11Y",
    "DETTIVO_MOCK_LLM",
    "DETTIVO_MOCK_ENGINE",
    "DETTIVO_FORCE_CPU",
    "DETTIVO_MODEL_SERVER",
    "DETTIVO_E2E_COMPLETE",
    "DETTIVO_E2E_STEP",
    "DETTIVO_E2E_SEED",
    "DETTIVO_E2E_OPEN",
    "DETTIVO_E2E_ROUTE",
    "DETTIVO_E2E_EXPORT_DIR",
    "DETTIVO_E2E_DISCLOSURE",
    "DETTIVO_E2E_OSD_STATE",
    "DETTIVO_E2E_BAR_STATE",
    "DETTIVO_E2E_MEETING_STATE",
    "DETTIVO_E2E_STATE",
    "DETTIVO_QA_PACING",
    "DETTIVO_QA_PLANT",
    "DETTIVO_QA_CANARY",
];

/// What `DETTIVO_QA_PLANT` may plant into a rendered surface.
pub const PLANTS: &[&str] = &[
    "basic_control",
    "basic_background",
    "basic_content",
    "default_font",
];

/// The pill states `DETTIVO_E2E_OSD_STATE` accepts.
pub const OSD_STATES: &[&str] = &[
    "listening",
    "transcribing",
    "enhancing",
    "inserted",
    "copied",
    "error",
    "hidden",
];

/// Prefixes reserved for QA; an unknown name under one is an error.
pub const RESERVED_PREFIXES: &[&str] = &["DETTIVO_QA", "DETTIVO_MOCK_", "DETTIVO_E2E_"];

fn err(variable: &str, message: impl Into<String>) -> QaError {
    QaError {
        variable: variable.to_string(),
        message: message.into(),
    }
}

fn flag(value: &OsString) -> bool {
    crate::paths::env_flag(value)
}

impl QaEnv {
    /// Parses the process environment.
    pub fn from_env(build: BuildKind) -> Result<Self, QaError> {
        let vars: Vec<(String, OsString)> = std::env::vars_os()
            .filter_map(|(k, v)| k.into_string().ok().map(|k| (k, v)))
            .collect();
        Self::from_vars(&vars, build)
    }

    /// Parses an explicit variable list, so tests pin every input.
    pub fn from_vars(vars: &[(String, OsString)], build: BuildKind) -> Result<Self, QaError> {
        let mut qa = Self::default();
        let get = |name: &str| -> Option<&OsString> {
            vars.iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v)
                .filter(|v| !v.is_empty())
        };
        for (name, _) in vars {
            if RESERVED_PREFIXES.iter().any(|p| name.starts_with(p))
                && !KNOWN.contains(&name.as_str())
            {
                return Err(err(name, "unknown QA variable (see docs/qa.md)"));
            }
        }
        let truthy = |name: &str| get(name).is_some_and(flag);
        qa.mock_mode = truthy("DETTIVO_MOCK_MODE");
        qa.enabled = truthy("DETTIVO_QA_MODE") || truthy("DETTIVO_QA") || qa.mock_mode;
        qa.allow_release = truthy("DETTIVO_QA_ALLOW_RELEASE");
        if qa.enabled && build == BuildKind::Release && !qa.allow_release {
            return Err(err(
                "DETTIVO_QA_MODE",
                "QA mode is refused in a release build; set DETTIVO_QA_ALLOW_RELEASE=1 to allow it",
            ));
        }
        for name in KNOWN {
            if !matches!(
                *name,
                "DETTIVO_QA_MODE" | "DETTIVO_QA" | "DETTIVO_MOCK_MODE" | "DETTIVO_QA_ALLOW_RELEASE"
            ) && get(name).is_some()
                && !qa.enabled
            {
                return Err(err(name, "needs QA mode (DETTIVO_QA_MODE=1)"));
            }
        }
        let hook = |name: &'static str| -> Option<String> {
            let v = get(name)?;
            Some(v.to_string_lossy().into_owned())
        };
        qa.mock_mic = hook("DETTIVO_MOCK_MIC").map(PathBuf::from);
        qa.mock_system_audio = hook("DETTIVO_MOCK_SYSTEM_AUDIO").map(PathBuf::from);
        qa.mock_insert = match hook("DETTIVO_MOCK_INSERT") {
            Some(v) => flag(&OsString::from(v)),
            None => qa.mock_mode,
        };
        qa.mock_input = hook("DETTIVO_MOCK_INPUT").is_some_and(|v| flag(&OsString::from(v)));
        qa.mock_a11y = hook("DETTIVO_MOCK_A11Y").is_some_and(|v| flag(&OsString::from(v)));
        qa.mock_llm = match hook("DETTIVO_MOCK_LLM") {
            None => None,
            Some(v) if v == "echo" => Some(MockLlm::Echo),
            Some(v) => match v.strip_prefix("fixture:") {
                Some(dir) if !dir.is_empty() => Some(MockLlm::Fixture(PathBuf::from(dir))),
                _ => {
                    return Err(err(
                        "DETTIVO_MOCK_LLM",
                        "expected `echo` or `fixture:<dir>`",
                    ));
                }
            },
        };
        if let Some(v) = hook("DETTIVO_MOCK_ENGINE") {
            for entry in v.split(',').map(str::trim).filter(|e| !e.is_empty()) {
                let Some((name, spec)) = entry.split_once('=') else {
                    return Err(err(
                        "DETTIVO_MOCK_ENGINE",
                        "expected `<name>=fixture:<dir>`",
                    ));
                };
                let Some(dir) = spec.strip_prefix("fixture:").filter(|d| !d.is_empty()) else {
                    return Err(err(
                        "DETTIVO_MOCK_ENGINE",
                        "expected `<name>=fixture:<dir>`",
                    ));
                };
                qa.mock_engines.push((name.to_string(), PathBuf::from(dir)));
            }
        }
        qa.force_cpu = hook("DETTIVO_FORCE_CPU").is_some_and(|v| flag(&OsString::from(v)));
        qa.model_server = hook("DETTIVO_MODEL_SERVER");
        if let Some(url) = &qa.model_server {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err(err("DETTIVO_MODEL_SERVER", "expected an http(s) URL"));
            }
        }
        qa.e2e_complete = hook("DETTIVO_E2E_COMPLETE").is_some_and(|v| flag(&OsString::from(v)));
        qa.e2e_seed = hook("DETTIVO_E2E_SEED").is_some_and(|v| flag(&OsString::from(v)));
        qa.e2e_step = match hook("DETTIVO_E2E_STEP").as_deref() {
            None => None,
            Some("keys") => Some(Step::Keys),
            Some("models") => Some(Step::Models),
            Some("try") => Some(Step::Try),
            Some(_) => return Err(err("DETTIVO_E2E_STEP", "expected keys, models or try")),
        };
        qa.e2e_open = match hook("DETTIVO_E2E_OPEN").as_deref() {
            None => None,
            Some("home") => Some(Route::Home),
            Some("history") => Some(Route::History),
            Some("history.detail") => Some(Route::HistoryDetail),
            Some("meetings") => Some(Route::Meetings),
            Some("meetings.live") => Some(Route::MeetingLive),
            Some("meetings.detail") => Some(Route::MeetingDetail),
            Some("settings") => Some(Route::Settings(None)),
            Some("onboarding") => Some(Route::Onboarding),
            Some(other) => match other.strip_prefix("settings.") {
                Some(sub) if !sub.is_empty() => Some(Route::Settings(Some(sub.to_string()))),
                _ => {
                    return Err(err(
                        "DETTIVO_E2E_OPEN",
                        format!("unknown route; the routes are {}", ROUTES.join(", ")),
                    ));
                }
            },
        };
        qa.e2e_route = hook("DETTIVO_E2E_ROUTE");
        qa.e2e_export_dir = hook("DETTIVO_E2E_EXPORT_DIR").map(PathBuf::from);
        qa.e2e_disclosure = match hook("DETTIVO_E2E_DISCLOSURE").as_deref() {
            None => None,
            Some("acknowledged") => Some(Disclosure::Acknowledged),
            Some("pending") => Some(Disclosure::Pending),
            Some(_) => {
                return Err(err(
                    "DETTIVO_E2E_DISCLOSURE",
                    "expected acknowledged or pending",
                ));
            }
        };
        qa.e2e_osd_state = hook("DETTIVO_E2E_OSD_STATE");
        if let Some(state) = &qa.e2e_osd_state {
            if !OSD_STATES.contains(&state.as_str()) {
                return Err(err(
                    "DETTIVO_E2E_OSD_STATE",
                    format!("expected one of {}", OSD_STATES.join(", ")),
                ));
            }
        }
        qa.qa_pacing = hook("DETTIVO_QA_PACING").map(PathBuf::from);
        qa.qa_plant = hook("DETTIVO_QA_PLANT");
        if let Some(plant) = &qa.qa_plant {
            if !PLANTS.contains(&plant.as_str()) {
                return Err(err(
                    "DETTIVO_QA_PLANT",
                    format!("expected one of {}", PLANTS.join(", ")),
                ));
            }
        }
        qa.qa_canary = hook("DETTIVO_QA_CANARY").is_some_and(|v| flag(&OsString::from(v)));
        for (name, allowed) in [
            (
                "DETTIVO_E2E_STATE",
                &[
                    "history-empty",
                    "meetings-empty",
                    "search-no-results",
                    "engine-loading",
                    "engine-crashed",
                    "download-failed",
                    "microphone-missing",
                    "insertion-fell-back",
                    "meeting-recovered",
                    "hint-sheet",
                ][..],
            ),
            (
                "DETTIVO_E2E_BAR_STATE",
                &[
                    "panel-idle",
                    "panel-recording",
                    "panel-transcribing",
                    "panel-meeting",
                    "panel-hint",
                    "glyph-idle",
                    "glyph-listening",
                    "glyph-transcribing",
                    "glyph-meeting",
                    "glyph-error",
                ][..],
            ),
            (
                "DETTIVO_E2E_MEETING_STATE",
                &[
                    "list",
                    "list-empty",
                    "live",
                    "detail-transcript",
                    "detail-notes",
                    "detail-analysis",
                    "rename",
                    "import",
                    "disclosure",
                ][..],
            ),
        ] {
            if let Some(value) = hook(name)
                && !allowed.contains(&value.as_str())
            {
                return Err(err(name, format!("expected one of {}", allowed.join(", "))));
            }
        }
        for (var, path) in [
            ("DETTIVO_MOCK_MIC", &qa.mock_mic),
            ("DETTIVO_MOCK_SYSTEM_AUDIO", &qa.mock_system_audio),
        ] {
            if let Some(p) = path {
                if !p.is_file() {
                    return Err(err(var, format!("{} is not a file", p.display())));
                }
            }
        }
        Ok(qa)
    }
}

#[cfg(test)]
#[path = "qa_tests.rs"]
mod tests;

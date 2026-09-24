//! The drive pack. A scenario is written against `driver::Driver` only
//! (R2): `dettivo-qa lint-scenarios` refuses a scenario file that names a
//! driver implementation, so every scenario runs on either driver.

pub mod app_daemon;
pub mod app_memory;
mod app_memory_launch;
pub mod app_routes;
pub mod app_support;
pub mod app_theme_live;
pub mod daemon;
pub mod first_insert_timing;
pub mod first_run_fresh;
pub mod first_run_provisioned;
pub mod first_run_resume;
pub mod first_run_steps;
pub mod first_run_support;
pub mod history_roundtrip;
pub mod history_seed;
pub mod history_seeded;
pub mod history_states;
pub mod hotkeys_hyprland;
pub mod hotkeys_hyprland_chords;
pub mod insertion_matrix;
pub mod keyboard_only;
pub mod meeting_live;
pub mod meeting_live_events;
pub mod meeting_recovery;
pub mod meeting_rig;
pub mod meeting_token_coverage;
pub mod meeting_token_support;
pub mod meetings_import_gui;
pub mod meetings_live_gui;
pub mod meetings_seeded;
pub mod never_into_self;
pub mod omarchy_bar;
pub mod omarchy_osd_host;
pub mod omarchy_setup_idempotent;
pub mod osd_dictation;
pub mod placeholder_window;
pub mod settings_checks;
pub mod settings_edit;
pub mod settings_env_override;
pub mod settings_roundtrip;
pub mod settings_support;
pub mod support;
pub mod theme_switch;

use std::path::Path;
use std::time::Duration;

use crate::driver::Driver;
use crate::evidence::Timings;
use crate::profile::Profile;

/// What a scenario needs from the runner.
pub struct Context<'a> {
    /// The isolated profile.
    pub profile: &'a mut Profile,
    /// Where evidence for this scenario goes.
    pub evidence_dir: &'a Path,
    /// The repository root (binaries under `target/`, `build/qt/`).
    pub repo_root: &'a Path,
    /// Step timings.
    pub timings: &'a mut Timings,
    /// Evidence files written, relative to `evidence_dir`.
    pub evidence: &'a mut Vec<String>,
    /// How long to wait for windows and labels.
    pub timeout: Duration,
}

/// A scenario: an id and a body that drives through the interface.
pub trait Scenario {
    /// Stable id, also the evidence directory name.
    fn id(&self) -> &'static str;
    /// One line for the pack listing.
    fn summary(&self) -> &'static str;
    /// What the scenario needs from the desktop beyond the driver; an
    /// `Err` names the missing piece and the runner records a skip.
    fn preflight(&self) -> Result<(), String> {
        Ok(())
    }
    /// False for a scenario that drives through the compositor and the
    /// socket alone, so a missing accessibility bus does not skip it.
    fn needs_driver(&self) -> bool {
        true
    }
    /// The models the scenario needs beyond the test set every profile
    /// carries (`profile::TEST_MODELS`), by name under the models
    /// directory (`diarize/diarization`); the runner links each into the
    /// profile's private tree before the daemon starts and skips the
    /// scenario naming the first one the machine does not have.
    fn models(&self) -> &'static [&'static str] {
        &[]
    }
    /// What the scenario needs before it can mean anything (a model, a
    /// binary); an `Err` skips the scenario with that reason.
    fn preconditions(&self, _ctx: &Context<'_>) -> Result<(), String> {
        Ok(())
    }
    /// Runs the drive; an `Err` is the failure reason.
    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String>;
}

/// Every scenario in the pack, in run order.
pub fn all() -> Vec<Box<dyn Scenario>> {
    vec![
        Box::new(placeholder_window::PlaceholderWindow),
        Box::new(insertion_matrix::InsertionMatrix),
        Box::new(never_into_self::NeverIntoSelf),
        Box::new(hotkeys_hyprland::HotkeysHyprland),
        Box::new(osd_dictation::OsdDictation),
        Box::new(history_roundtrip::HistoryRoundtrip),
        Box::new(meeting_recovery::MeetingRecovery),
        Box::new(meeting_rig::MeetingRig),
        Box::new(meeting_live::MeetingLive),
        Box::new(first_insert_timing::FirstInsertTiming),
        Box::new(theme_switch::ThemeSwitch),
        Box::new(app_routes::AppRoutes),
        Box::new(app_theme_live::AppThemeLive),
        Box::new(app_daemon::AppDaemon),
        Box::new(first_run_fresh::FirstRunFresh),
        Box::new(first_run_provisioned::FirstRunProvisioned),
        Box::new(history_seeded::HistorySeeded),
        Box::new(omarchy_osd_host::OmarchyOsdHost),
        Box::new(omarchy_bar::OmarchyBar),
        Box::new(settings_roundtrip::SettingsRoundtrip::whole()),
        Box::new(first_run_steps::FirstRunSteps),
        Box::new(first_run_resume::FirstRunResume),
        Box::new(settings_env_override::SettingsEnvOverride),
        Box::new(history_states::HistoryStates),
        Box::new(omarchy_setup_idempotent::OmarchySetupIdempotent),
        Box::new(meetings_seeded::MeetingsSeeded),
        Box::new(meetings_live_gui::MeetingsLiveGui),
        Box::new(meetings_import_gui::MeetingsImportGui),
        Box::new(meeting_token_coverage::MeetingTokenCoverage),
        Box::new(keyboard_only::KeyboardOnly),
    ]
}

/// A scenario by id: one of `all()`, or a settings section on its own
/// (`settings_roundtrip.<section>`, the GUI pack's eight steps).
pub fn by_id(id: &str) -> Option<Box<dyn Scenario>> {
    match id {
        "app_memory.wayland" => return Some(Box::new(app_memory::AppMemory("wayland"))),
        "app_memory.xvfb" => return Some(Box::new(app_memory::AppMemory("xvfb"))),
        _ => {}
    }
    all()
        .into_iter()
        .find(|s| s.id() == id)
        .or_else(|| settings_roundtrip::SettingsRoundtrip::sectioned(id).map(|s| Box::new(s) as _))
}

/// The build profile a binary path belongs to: `release`, `debug`, or
/// `qt` for a binary from the CMake tree (built Release).
pub fn binary_profile(path: &Path) -> &'static str {
    let text = path.to_string_lossy();
    if text.contains("/target/release/") {
        "release"
    } else if text.contains("/target/debug/") {
        "debug"
    } else {
        "qt"
    }
}

/// The variable that names the build profile the drives run,
/// `debug` (the default) or `release`; `pack release` pins `release`
/// for its own process and every verb it spawns.
pub const PROFILE_ENV: &str = "DETTIVO_BUILD_PROFILE";

static PROFILE_OVERRIDE: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// The build profile this run drives: the in-process pin, else the
/// environment, else `debug`.
pub fn build_profile() -> String {
    if let Some(p) = PROFILE_OVERRIDE.get() {
        return (*p).to_string();
    }
    match std::env::var(PROFILE_ENV).ok().as_deref() {
        Some("release") => "release".into(),
        _ => "debug".into(),
    }
}

/// Pins this process to the release profile (the release gate).
pub fn pin_release_profile() {
    let _ = PROFILE_OVERRIDE.set("release");
}

/// The path of a built binary the scenarios launch: the Qt tree for the
/// Qt apps, otherwise `target/<profile>/<name>` for the run's build
/// profile, so what a pack drives is chosen once and named, never the
/// file that happens to be newer.
pub fn binary(repo_root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    binary_for(repo_root, name, &build_profile())
}

fn binary_for(repo_root: &Path, name: &str, profile: &str) -> Result<std::path::PathBuf, String> {
    let qt = repo_root.join("build/qt/apps").join(name).join(name);
    if qt.is_file() {
        return Ok(qt);
    }
    let path = repo_root.join("target").join(profile).join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "{name} is not built for the {profile} profile (looked under build/qt/apps and target/{profile}; `just build{}` builds it)",
            if profile == "release" { "-release" } else { "" }
        ))
    }
}

#[cfg(test)]
mod binary_tests {
    use super::*;

    #[test]
    fn the_profile_chooses_the_binary_and_a_newer_file_in_the_other_profile_never_does() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("target/release")).unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/release/dettivod"), b"old release").unwrap();
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(root.join("target/debug/dettivod"), b"new debug").unwrap();
        let release = binary_for(root, "dettivod", "release").unwrap();
        assert!(
            release.ends_with("target/release/dettivod"),
            "{}",
            release.display()
        );
        let debug = binary_for(root, "dettivod", "debug").unwrap();
        assert!(
            debug.ends_with("target/debug/dettivod"),
            "{}",
            debug.display()
        );
        let why = binary_for(root, "dettivo", "release").unwrap_err();
        assert!(
            why.contains("target/release") && why.contains("just build-release"),
            "{why}"
        );
        assert_eq!(binary_profile(&release), "release");
        assert_eq!(binary_profile(&debug), "debug");
    }
}

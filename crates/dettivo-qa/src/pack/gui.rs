//! The GUI packs (fn-35, ADR 0037): one pack per surface, each runnable
//! alone, and `gui`, which runs the four in order into one report. The
//! fallback driver's steps must pass; the cua-driver steps skip with the
//! reason where the tool is absent, so a cua regression never hides a
//! GUI failure. Every scenario step's captured trees are scanned for
//! developer text and walked for unnamed controls after the step
//! (`scan`), and the report carries the findings and the coverage per
//! surface.

use super::{Expect, Pack, Step, StepKind};
use crate::scenarios::settings_roundtrip::SettingsRoundtrip;

/// The four surfaces, in the order `gui` runs them.
pub const SURFACES: &[&str] = &["onboarding", "settings", "history", "omarchy"];

const fn on(
    id: &'static str,
    driver: &'static str,
    expect: Expect,
    surface: &'static str,
    contributes: &'static str,
) -> Step {
    Step {
        id,
        kind: StepKind::Scenario,
        driver: Some(driver),
        expect,
        surface: Some(surface),
        contributes,
    }
}

/// A scenario on the fallback driver (must pass) and on cua-driver
/// (skip allowed where the tool is absent).
fn both(id: &'static str, surface: &'static str, contributes: &'static str) -> [Step; 2] {
    [
        on(id, "atspi", Expect::Pass, surface, contributes),
        on(id, "cua", Expect::SkipAllowed, surface, contributes),
    ]
}

const fn command(id: &'static str, surface: &'static str, contributes: &'static str) -> Step {
    Step {
        id,
        kind: StepKind::Command,
        driver: None,
        expect: Expect::Pass,
        surface: Some(surface),
        contributes,
    }
}

fn onboarding_steps() -> Vec<Step> {
    let mut steps = Vec::new();
    steps.extend(both(
        "first_run_fresh",
        "onboarding",
        "the three screens, the snippet, the download, the take",
    ));
    steps.extend(both(
        "first_run_provisioned",
        "onboarding",
        "a provisioned profile opens Home",
    ));
    steps.extend(both(
        "first_run_steps",
        "onboarding",
        "each step opens by name; three screens, no welcome",
    ));
    steps.extend(both(
        "first_run_resume",
        "onboarding",
        "a flow closed on Models reopens on Models",
    ));
    steps
}

fn settings_steps() -> Vec<Step> {
    let mut steps: Vec<Step> = SettingsRoundtrip::section_ids()
        .into_iter()
        .map(|id| Step {
            id,
            kind: StepKind::Scenario,
            driver: None,
            expect: Expect::Pass,
            surface: Some("settings"),
            contributes: "the section's keys round-trip, the planted comment survives",
        })
        .collect();
    steps.extend(both(
        "settings_env_override",
        "settings",
        "the environment badge and the disabled control",
    ));
    steps.push(command(
        "lint_settings_keys",
        "settings",
        "every schema key on a route or excused",
    ));
    steps
}

fn history_steps() -> Vec<Step> {
    let mut steps = Vec::new();
    steps.extend(both(
        "history_seeded",
        "history",
        "the seed by day, search, detail, export, re-run, delete",
    ));
    steps.extend(both(
        "app_routes",
        "history",
        "every route by name with its title",
    ));
    steps.extend(both(
        "history_states",
        "history",
        "the empty state and the no-hits state",
    ));
    // The keyboard drive (fn-38, ADR 0042): every route by Tab with the
    // focus ring, and the four conventions with the hint sheet.
    steps.extend(both(
        "keyboard_only",
        "history",
        "Tab reaches every control on every route; Super+F, Escape, / and ? work",
    ));
    steps
}

fn omarchy_steps() -> Vec<Step> {
    vec![
        command(
            "omarchy_validate",
            "omarchy",
            "the manifest and omarchy plugin validate",
        ),
        command(
            "omarchy_shim_load",
            "omarchy",
            "the plugin loads under the shell shim and answers events",
        ),
        command(
            "omarchy_version_skew",
            "omarchy",
            "the install and upgrade hints",
        ),
        Step {
            id: "omarchy_bar",
            kind: StepKind::Scenario,
            driver: None,
            expect: Expect::SkipAllowed,
            surface: Some("omarchy"),
            contributes: "the bar widget's pixels against the daemon's state",
        },
        Step {
            id: "omarchy_setup_idempotent",
            kind: StepKind::Scenario,
            driver: None,
            expect: Expect::SkipAllowed,
            surface: Some("omarchy"),
            contributes: "dettivo setup omarchy twice, nothing changes by hash",
        },
    ]
}

/// The steps of one surface.
pub fn surface_steps(surface: &str) -> Vec<Step> {
    match surface {
        "onboarding" => onboarding_steps(),
        "settings" => settings_steps(),
        "history" => history_steps(),
        "omarchy" => omarchy_steps(),
        _ => Vec::new(),
    }
}

/// The five packs: `gui-<surface>` for each surface and `gui` for all.
pub fn packs() -> Vec<Pack> {
    let mut packs = vec![
        Pack {
            name: "gui-onboarding",
            summary: "first run on both drivers: the three screens, the provisioned profile, each step by name, the resume",
            steps: onboarding_steps(),
        },
        Pack {
            name: "gui-settings",
            summary: "settings: the eight section round trips, the environment override, the key-coverage lint",
            steps: settings_steps(),
        },
        Pack {
            name: "gui-history",
            summary: "history: the seed, every route, the empty and no-hits states",
            steps: history_steps(),
        },
        Pack {
            name: "gui-omarchy",
            summary: "the Omarchy plugin: validate, the shim load, the version skew, the bar and the idempotent setup on a desktop",
            steps: omarchy_steps(),
        },
    ];
    let steps = SURFACES.iter().flat_map(|s| surface_steps(s)).collect();
    packs.push(Pack {
        name: "gui",
        summary: "the four GUI surfaces in order: onboarding, settings, history, omarchy; one report with the scans per surface",
        steps,
    });
    packs
}

/// `pack` narrowed to one surface when `surface` is given: an unknown
/// surface, or one the pack does not carry, is refused naming the four.
pub fn narrow(pack: Pack, surface: Option<&str>) -> Result<Pack, String> {
    let Some(surface) = surface else {
        return Ok(pack);
    };
    if !SURFACES.contains(&surface) {
        return Err(format!(
            "unknown surface {surface:?}; the surfaces are: {}",
            SURFACES.join(", ")
        ));
    }
    let steps: Vec<Step> = pack
        .steps
        .iter()
        .filter(|s| s.surface == Some(surface))
        .copied()
        .collect();
    if steps.is_empty() {
        return Err(format!(
            "pack {} has no {surface} steps; the surfaces are: {}",
            pack.name,
            SURFACES.join(", ")
        ));
    }
    Ok(Pack { steps, ..pack })
}

/// The program and arguments a command step runs from the repository
/// root; `None` for an id that is not a command.
pub fn command_for(id: &str) -> Option<(&'static str, Vec<&'static str>)> {
    match id {
        "lint_settings_keys" => Some(("scripts/lint-settings-keys.sh", Vec::new())),
        // The five meeting export formats byte-equal to their goldens
        // (the meetings pack, ADR 0039); a bare tool name runs from PATH.
        "meeting_export_goldens" => Some((
            "cargo",
            vec![
                "test",
                "-q",
                "-p",
                "dettivo-storage",
                "--test",
                "meeting_export",
            ],
        )),
        "omarchy_validate" => Some(("scripts/lint-omarchy-plugin.sh", vec!["build/qt"])),
        "omarchy_shim_load" => Some((
            "scripts/qa/omarchy-plugin-test.sh",
            vec![
                "build/qt",
                "OmarchyPluginEvents::test_1_the_event_stream_drives_glyph_panel_and_pill",
                "OmarchyPluginEvents::test_2_every_action_is_one_command",
            ],
        )),
        "omarchy_version_skew" => Some((
            "scripts/qa/omarchy-plugin-test.sh",
            vec![
                "build/qt",
                "OmarchyPluginHints::test_1_install_hint_without_the_module",
                "OmarchyPluginHints::test_2_upgrade_hint_against_an_old_daemon",
            ],
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::{StepOutcome, StepResult, by_name, driver_for, passed, run_steps};

    #[test]
    fn the_gui_pack_composes_the_four_surfaces_in_order_from_real_steps() {
        let gui = by_name("gui").unwrap();
        assert_eq!(gui.surfaces(), SURFACES);
        let mut expected = Vec::new();
        for surface in SURFACES {
            let own = by_name(&format!("gui-{surface}")).unwrap();
            assert_eq!(own.surfaces(), [*surface]);
            expected.extend(own.steps.iter().map(|s| (s.id, s.driver)));
        }
        let composed: Vec<_> = gui.steps.iter().map(|s| (s.id, s.driver)).collect();
        assert_eq!(composed, expected);
        for step in &gui.steps {
            match step.kind {
                StepKind::Scenario => assert!(
                    crate::scenarios::by_id(step.id).is_some(),
                    "{} is not a scenario",
                    step.id
                ),
                StepKind::Command => {
                    assert!(command_for(step.id).is_some(), "{} has no command", step.id)
                }
                _ => panic!("{} is not a GUI step kind", step.id),
            }
        }
        let names: Vec<&str> = crate::pack::packs().iter().map(|p| p.name).collect();
        for name in [
            "gui-onboarding",
            "gui-settings",
            "gui-history",
            "gui-omarchy",
            "gui",
        ] {
            assert!(names.contains(&name), "{name} missing from {names:?}");
        }
        let sections: Vec<&str> = gui
            .steps
            .iter()
            .filter(|s| s.id.starts_with("settings_roundtrip."))
            .map(|s| s.id)
            .collect();
        assert_eq!(sections.len(), 8);
        assert_eq!(sections[0], "settings_roundtrip.general");
        assert_eq!(driver_for(&gui.steps[0], "cua"), "atspi");
        assert_eq!(driver_for(&gui.steps[1], "atspi"), "cua");
        assert_eq!(driver_for(gui.steps.last().unwrap(), "atspi"), "atspi");
    }

    #[test]
    fn the_surface_filter_narrows_and_names_the_four() {
        let gui = by_name("gui").unwrap();
        let settings = narrow(gui.clone(), Some("settings")).unwrap();
        assert!(settings.steps.iter().all(|s| s.surface == Some("settings")));
        assert_eq!(
            settings.steps.len(),
            by_name("gui-settings").unwrap().steps.len()
        );
        let err = narrow(gui.clone(), Some("garage")).unwrap_err();
        assert!(
            err.contains("garage") && err.contains("onboarding, settings, history, omarchy"),
            "{err}"
        );
        let own = by_name("gui-history").unwrap();
        let err = narrow(own, Some("omarchy")).unwrap_err();
        assert!(err.contains("gui-history has no omarchy steps"), "{err}");
        assert_eq!(
            narrow(gui.clone(), None).unwrap().steps.len(),
            gui.steps.len()
        );
    }

    #[test]
    fn cua_and_desktop_steps_may_skip_while_the_fallback_steps_must_pass() {
        let gui = by_name("gui").unwrap();
        let results = run_steps(&gui, "atspi", false, &mut |step, driver| StepResult {
            id: step.id.into(),
            driver: driver.into(),
            outcome: if driver == "cua"
                || step.id.starts_with("omarchy_bar")
                || step.id.starts_with("omarchy_setup")
            {
                StepOutcome::Skip
            } else {
                StepOutcome::Pass
            },
            duration_ms: 1,
            evidence: None,
            reason: None,
            surface: None,
            scan: None,
        });
        assert!(passed(&gui, &results));
        assert!(results.iter().all(|r| r.surface.is_some()));
        let results = run_steps(&gui, "atspi", true, &mut |step, driver| StepResult {
            id: step.id.into(),
            driver: driver.into(),
            outcome: if step.id == "settings_roundtrip.polish" {
                StepOutcome::Fail
            } else {
                StepOutcome::Pass
            },
            duration_ms: 1,
            evidence: None,
            reason: None,
            surface: None,
            scan: None,
        });
        assert!(!passed(&gui, &results));
        let failed: Vec<&str> = results
            .iter()
            .filter(|r| r.outcome == StepOutcome::Fail)
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(failed, ["settings_roundtrip.polish"]);
    }
}

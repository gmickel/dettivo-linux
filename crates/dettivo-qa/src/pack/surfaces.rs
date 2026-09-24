//! The per-surface block of a GUI pack report (fn-35 R1, R6): for every
//! surface the pack names, its steps with their outcomes, the routes
//! scanned, every developer-text finding with the step and route it was
//! on, and the accessibility coverage over every interactive element the
//! surface showed. The block says which screen leaked what.

use serde::{Deserialize, Serialize};

use super::{Pack, StepOutcome, StepResult};

/// One step under a surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceStep {
    /// The step id.
    pub id: String,
    /// The driver it ran on.
    pub driver: String,
    /// The outcome.
    pub outcome: StepOutcome,
}

/// A developer-text finding, located.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Located {
    /// The step.
    pub step: String,
    /// The driver.
    pub driver: String,
    /// The route.
    pub route: String,
    /// The class.
    pub class: String,
    /// The element's accessible name.
    pub name: String,
}

/// An unnamed interactive element, located.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnnamedControl {
    /// The step.
    pub step: String,
    /// The driver.
    pub driver: String,
    /// The route.
    pub route: String,
    /// The element's role.
    pub role: String,
    /// Its index in the walk.
    pub index: u32,
}

/// The negative text block of a surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NegativeText {
    /// Routes scanned across the surface's steps.
    pub routes: usize,
    /// Every finding.
    pub findings: Vec<Located>,
}

/// One surface's block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceReport {
    /// `onboarding`, `settings`, `history` or `omarchy`.
    pub name: String,
    /// The surface's steps in run order.
    pub steps: Vec<SurfaceStep>,
    /// True when none of the surface's steps failed hard.
    pub passed: bool,
    /// The scan over every route.
    pub negative_text: NegativeText,
    /// `named / interactive` over every route, `1.0` for inspected
    /// screens without a control, `None` when no tree was inspected.
    pub a11y_coverage: Option<f64>,
    /// Interactive elements over every route.
    pub a11y_interactive: usize,
    /// Of those, named.
    pub a11y_named: usize,
    /// Every unnamed interactive element.
    pub a11y_offenders: Vec<UnnamedControl>,
}

/// Builds the block for every surface of `pack`; empty for a pack
/// without surfaces.
pub fn build(pack: &Pack, steps: &[StepResult]) -> Vec<SurfaceReport> {
    pack.surfaces()
        .into_iter()
        .map(|surface| {
            let own: Vec<(&super::Step, &StepResult)> = pack
                .steps
                .iter()
                .zip(steps)
                .filter(|(s, _)| s.surface == Some(surface))
                .collect();
            let mut negative = NegativeText::default();
            let mut offenders = Vec::new();
            let (mut interactive, mut named) = (0, 0);
            for (_, r) in &own {
                let Some(scan) = &r.scan else { continue };
                negative.routes += scan.routes.len();
                for route in &scan.routes {
                    negative
                        .findings
                        .extend(route.negative_text.iter().map(|f| Located {
                            step: r.id.clone(),
                            driver: r.driver.clone(),
                            route: route.route.clone(),
                            class: f.class.clone(),
                            name: f.name.clone(),
                        }));
                    offenders.extend(route.a11y.offenders.iter().map(|o| UnnamedControl {
                        step: r.id.clone(),
                        driver: r.driver.clone(),
                        route: route.route.clone(),
                        role: o.role.clone(),
                        index: o.index,
                    }));
                    interactive += route.a11y.interactive;
                    named += route.a11y.named;
                }
            }
            SurfaceReport {
                name: surface.to_string(),
                passed: own.iter().all(|(s, r)| !r.hard_failure(s.expect)),
                steps: own
                    .iter()
                    .map(|(_, r)| SurfaceStep {
                        id: r.id.clone(),
                        driver: r.driver.clone(),
                        outcome: r.outcome,
                    })
                    .collect(),
                a11y_coverage: if negative.routes == 0 {
                    None
                } else if interactive == 0 {
                    Some(1.0)
                } else {
                    Some(named as f64 / interactive as f64)
                },
                negative_text: negative,
                a11y_interactive: interactive,
                a11y_named: named,
                a11y_offenders: offenders,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y_tree::{Coverage, Offender};
    use crate::pack::scan::{RouteScan, StepScan, TextFinding};
    use crate::pack::{by_name, run_steps};

    #[test]
    fn the_block_locates_every_finding_and_sums_the_coverage_per_surface() {
        let gui = by_name("gui").unwrap();
        let results = run_steps(&gui, "atspi", true, &mut |step, driver| {
            let scan = match (step.id, driver) {
                ("first_run_steps", "atspi") => Some(StepScan {
                    routes: vec![
                        RouteScan {
                            route: "keys".into(),
                            negative_text: vec![TextFinding {
                                class: "internal identifier".into(),
                                name: "parity_gap".into(),
                            }],
                            a11y: Coverage {
                                interactive: 3,
                                named: 2,
                                coverage: 2.0 / 3.0,
                                offenders: vec![Offender {
                                    role: "push button".into(),
                                    id: "/e/7".into(),
                                    index: 7,
                                    bounds: None,
                                }],
                            },
                        },
                        RouteScan {
                            route: "models".into(),
                            negative_text: Vec::new(),
                            a11y: Coverage {
                                interactive: 5,
                                named: 5,
                                coverage: 1.0,
                                offenders: Vec::new(),
                            },
                        },
                    ],
                }),
                ("history_states", "atspi") => Some(StepScan {
                    routes: vec![RouteScan {
                        route: "empty".into(),
                        negative_text: Vec::new(),
                        a11y: Coverage {
                            interactive: 4,
                            named: 4,
                            coverage: 1.0,
                            offenders: Vec::new(),
                        },
                    }],
                }),
                _ => None,
            };
            StepResult {
                id: step.id.into(),
                driver: driver.into(),
                outcome: if step.id == "first_run_steps" && driver == "atspi" {
                    StepOutcome::Fail
                } else {
                    StepOutcome::Pass
                },
                duration_ms: 1,
                evidence: None,
                reason: None,
                surface: None,
                scan,
            }
        });
        let surfaces = build(&gui, &results);
        let names: Vec<&str> = surfaces.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["onboarding", "settings", "history", "omarchy"]);
        let onboarding = &surfaces[0];
        assert!(!onboarding.passed);
        assert_eq!(onboarding.steps.len(), 8);
        assert_eq!(onboarding.negative_text.routes, 2);
        assert_eq!(onboarding.negative_text.findings.len(), 1);
        let f = &onboarding.negative_text.findings[0];
        assert_eq!(
            (f.step.as_str(), f.route.as_str(), f.class.as_str()),
            ("first_run_steps", "keys", "internal identifier")
        );
        assert_eq!((onboarding.a11y_interactive, onboarding.a11y_named), (8, 7));
        assert!((onboarding.a11y_coverage.unwrap() - 0.875).abs() < 1e-9);
        assert_eq!(onboarding.a11y_offenders[0].index, 7);
        let history = &surfaces[2];
        assert!(history.passed);
        assert_eq!(history.a11y_coverage, Some(1.0));
        assert_eq!(history.negative_text.routes, 1);
        let omarchy = &surfaces[3];
        assert_eq!(omarchy.a11y_coverage, None, "no tree was inspected");
        assert_eq!(omarchy.negative_text.routes, 0);
    }
}

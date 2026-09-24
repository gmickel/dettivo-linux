//! How the release gate classifies a blocker: external when its reason
//! names a cause outside this machine and this repository (the CPU-only
//! VM, a tool that is not installed, the live desktop the Xvfb session
//! cannot reach, a GPU counter, a person at the microphone), unexplained
//! otherwise. A failed row is never a blocker; it fails its step.

use super::release::Blocker;

/// Classifies a blocker's reason: `Some(needs)` when the cause is outside
/// this machine and this repository, `None` when it is something to fix.
pub fn external_need(reason: &str) -> Option<String> {
    let r = reason.to_ascii_lowercase();
    let rules: &[(&[&str], &str)] = &[
        (
            &["cpu-only vm", "cpu-only virtual machine"],
            "a CPU-only virtual machine that runs the same command and checks its report in",
        ),
        (
            &["cua-driver", "cua driver", "missing: cua"],
            "cua-driver installed (scripts/qa/install-cua-driver.sh)",
        ),
        (
            &[
                "workflows are disabled",
                "no workflow run",
                "gh is not installed",
                "gh: not found",
            ],
            "the GitHub Actions workflows enabled and a run for this commit",
        ),
        (
            &[
                "hyprctl",
                "hyprland session",
                "hyprland_instance_signature",
                "dettivo_qa_omarchy_live",
                "omarchy is not",
                "no omarchy",
                "grim is not",
                "not a hyprland",
            ],
            "the live Hyprland desktop with the Omarchy shell (the gate runs the drives under Xvfb, off the live screen)",
        ),
        (
            &["/dev/uinput"],
            "a writable /dev/uinput for the virtual keyboard",
        ),
        (
            &["nvidia-smi", "gpu counter", "no gpu work expected"],
            "a GPU process counter on this machine",
        ),
        (
            &["real microphone", "a person"],
            "a person who dictates once through the real microphone and records the receipt",
        ),
    ];
    rules
        .iter()
        .find(|(needles, _)| needles.iter().any(|n| r.contains(n)))
        .map(|(_, needs)| (*needs).to_string())
}

/// A blocker from a reason, classified.
pub fn blocker(step: &str, reason: &str) -> Blocker {
    let attribution_unavailable = step == "pack_meetings/gpu_workload_proof"
        && [
            "workload attribution unavailable: engine pid ",
            "workload attribution unavailable: aggregate gpu_busy_percent ",
        ]
        .iter()
        .any(|prefix| reason.starts_with(prefix));
    let needs = if attribution_unavailable {
        Some("a per-process GPU workload counter that attributes inference execution to the tested engine".to_string())
    } else {
        external_need(reason)
    };
    Blocker {
        step: step.to_string(),
        reason: reason.to_string(),
        external: needs.is_some(),
        needs,
    }
}

#[cfg(test)]
mod tests {
    use super::blocker;
    use crate::pack::StepOutcome;
    use crate::pack::release_steps::{judge_pack, row};
    use serde_json::json;

    #[test]
    fn workload_attribution_skips_are_external_only_for_the_gpu_proof_step() {
        let step = "pack_meetings/gpu_workload_proof";
        for reason in [
            "workload attribution unavailable: engine pid 3112120 has a GPU allocation in 23 of 23 samples; process presence does not prove inference execution",
            "workload attribution unavailable: aggregate gpu_busy_percent peaked at 97% in 23 samples; other processes or devices may supply this activity",
        ] {
            let result = blocker(step, reason);
            assert!(result.external, "{reason}");
            assert!(result.needs.unwrap().contains("workload"));
            assert!(!blocker("pack_meetings/meeting_throughput", reason).external);
        }
        assert!(
            !blocker(
                step,
                "workload attribution unavailable: unknown instrumentation error"
            )
            .external
        );
        assert!(
            !blocker(
                step,
                "no dettivo-engine process was seen under the daemon in 23 samples"
            )
            .external
        );
    }

    #[test]
    fn an_external_gpu_counter_blocker_never_excuses_a_failed_proof_row() {
        let reason = "workload attribution unavailable: engine pid 3112120 has a GPU allocation";
        for outcome in ["skip", "fail"] {
            let report = json!({
                "passed": true,
                "blockers": [{"step": "gpu_workload_proof", "driver": "none", "reason": reason}],
                "steps": [{"id": "gpu_workload_proof", "driver": "none", "outcome": outcome, "reason": reason}],
            });
            let mut result = row("pack_meetings", String::new());
            judge_pack("pack_meetings", &report, &mut result);
            assert!(result.blockers[0].external);
            assert_eq!(
                result.outcome,
                if outcome == "fail" {
                    StepOutcome::Fail
                } else {
                    StepOutcome::Pass
                }
            );
        }
    }
}

//! Native keyboard checks require an unlocked compositor session.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use super::super::Context;

pub(super) fn check(ctx: &mut Context<'_>) -> Result<(), String> {
    let Some(signature) = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE") else {
        return Ok(());
    };
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or("native keyboard session is unreadable: XDG_RUNTIME_DIR is missing")?;
    let socket = runtime.join("hypr").join(signature).join(".socket.sock");
    let query = || -> std::io::Result<String> {
        let mut stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        stream.write_all(b"j/monitors")?;
        let mut answer = String::new();
        stream.take(1024 * 1024).read_to_string(&mut answer)?;
        Ok(answer)
    };
    let answer = query().map_err(|e| format!("native keyboard session is unreadable: {e}"))?;
    let file = "keyboard-desktop.json";
    std::fs::write(ctx.evidence_dir.join(file), &answer).map_err(|e| format!("{file}: {e}"))?;
    ctx.evidence.push(file.into());
    let monitors = serde_json::from_str(&answer)
        .map_err(|e| format!("native keyboard session is unreadable: {e}"))?;
    unlocked(&monitors)
}

fn unlocked(monitors: &serde_json::Value) -> Result<(), String> {
    let monitors = monitors
        .as_array()
        .ok_or("native keyboard session is unreadable: expected monitor array")?;
    let blockers: Vec<_> = monitors
        .iter()
        .filter_map(|m| m["solitaryBlockedBy"].as_array())
        .collect();
    if blockers.iter().any(|b| b.iter().any(|v| v == "LOCK")) {
        return Err(
            "native keyboard session is locked; unlock the desktop before running keyboard_only"
                .into(),
        );
    }
    // Hyprland returns WORKSPACE before checking the session lock. This mirrors
    // Omarchy's omarchy-hyprland-session-locked probe: absence of LOCK alone is insufficient.
    if blockers.iter().any(|b| !b.iter().any(|v| v == "WORKSPACE")) {
        return Ok(());
    }
    Err("native keyboard session is unreadable: no monitor reports its lock blockers".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn locked_and_unreadable_sessions_cannot_prove_keyboard_coverage() {
        for (monitors, expected) in [
            (
                json!([{"solitaryBlockedBy": ["LOCK", "WINDOWED"]}]),
                "locked",
            ),
            (
                json!([{"solitaryBlockedBy": []}, {"solitaryBlockedBy": ["LOCK"]}]),
                "locked",
            ),
            (json!([]), "unreadable"),
            (json!([{}]), "unreadable"),
            (json!([{"solitaryBlockedBy": ["WORKSPACE"]}]), "unreadable"),
        ] {
            let reason = unlocked(&monitors).expect_err("native session must be checked");
            assert!(reason.contains(expected), "{reason}");
        }
        assert_eq!(
            unlocked(&json!([{"solitaryBlockedBy": ["WINDOWED"]}])),
            Ok(())
        );
    }
}

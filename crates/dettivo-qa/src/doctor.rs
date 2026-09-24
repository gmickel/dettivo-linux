//! `doctor`: names the missing piece of the desktop the rig needs (R1's
//! error surface): display, session bus, accessibility bus, the tools.

use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// One check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// What was checked.
    pub name: String,
    /// True when present.
    pub ok: bool,
    /// What was found, or what is missing and how to get it.
    pub detail: String,
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(tool).is_file()))
        .unwrap_or(false)
}

/// Runs every check.
pub fn checks() -> Vec<Check> {
    let mut out = Vec::new();
    let display = std::env::var("DISPLAY").unwrap_or_default();
    out.push(Check {
        name: "x11 display".into(),
        ok: !display.is_empty(),
        detail: if display.is_empty() {
            "DISPLAY is unset; run under XWayland (Hyprland) or scripts/qa/xvfb-session.sh".into()
        } else {
            display
        },
    });
    let bus = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default();
    let runtime_bus = std::env::var("XDG_RUNTIME_DIR")
        .map(|r| Path::new(&r).join("bus").exists())
        .unwrap_or(false);
    out.push(Check {
        name: "session bus".into(),
        ok: !bus.is_empty() || runtime_bus,
        detail: if bus.is_empty() && !runtime_bus {
            "no DBUS_SESSION_BUS_ADDRESS and no $XDG_RUNTIME_DIR/bus; start one with dbus-run-session".into()
        } else if bus.is_empty() {
            "$XDG_RUNTIME_DIR/bus".into()
        } else {
            bus
        },
    });
    let a11y = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.a11y.Bus",
            "/org/a11y/bus",
            "org.a11y.Bus",
            "GetAddress",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    // The launcher can answer with the path of a socket nobody listens
    // on any more (a launcher that died, or one that clobbered the
    // desktop's file); a drive on that bus fails after the preflight, so
    // the check connects rather than trusting the answer.
    let (a11y_ok, a11y_detail) = match a11y {
        None => (
            false,
            "org.a11y.Bus does not answer; install at-spi2-core and start at-spi-bus-launcher"
                .to_string(),
        ),
        Some(answer) => match a11y_socket(&answer) {
            Some(path) if std::os::unix::net::UnixStream::connect(&path).is_err() => (
                false,
                format!(
                    "org.a11y.Bus answers {} but nothing listens there; restart at-spi-bus-launcher or drive under a private session bus",
                    path.display()
                ),
            ),
            _ => (true, answer),
        },
    };
    out.push(Check {
        name: "accessibility bus".into(),
        ok: a11y_ok,
        detail: a11y_detail,
    });
    for (tool, hint) in [
        ("cua-driver", "scripts/qa/install-cua-driver.sh"),
        ("pactl", "pacman -S libpulse (PipeWire's pactl)"),
        ("pw-play", "pacman -S pipewire"),
        ("pw-record", "pacman -S pipewire"),
        ("Xvfb", "pacman -S xorg-server-xvfb (CI only)"),
    ] {
        let present = on_path(tool);
        out.push(Check {
            name: tool.into(),
            ok: present,
            detail: if present {
                "on PATH".into()
            } else {
                format!("not on PATH; {hint}")
            },
        });
    }
    out
}

/// The socket path in a `GetAddress` answer such as
/// `s "unix:path=/run/user/1000/at-spi/bus_0"`; `None` for an abstract
/// or non-unix address, which the check then trusts.
pub fn a11y_socket(answer: &str) -> Option<std::path::PathBuf> {
    let start = answer.find("unix:path=")? + "unix:path=".len();
    let rest = &answer[start..];
    let end = rest.find(['"', ',', ';']).unwrap_or(rest.len());
    Some(std::path::PathBuf::from(&rest[..end]))
}

/// True when the pieces a drive needs (display, session bus, a11y bus) are all present.
pub fn drive_ready(checks: &[Check]) -> Result<(), String> {
    for name in ["x11 display", "session bus", "accessibility bus"] {
        if let Some(c) = checks.iter().find(|c| c.name == name) {
            if !c.ok {
                return Err(format!("{name}: {}", c.detail));
            }
        }
    }
    Ok(())
}

/// Human rendering.
pub fn human(checks: &[Check]) -> String {
    checks
        .iter()
        .map(|c| {
            format!(
                "{} {:<18} {}\n",
                if c.ok { "ok  " } else { "MISS" },
                c.name,
                c.detail
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bus_address_names_its_socket() {
        assert_eq!(
            a11y_socket("s \"unix:path=/run/user/1000/at-spi/bus_0\""),
            Some(std::path::PathBuf::from("/run/user/1000/at-spi/bus_0"))
        );
        assert_eq!(
            a11y_socket("unix:path=/tmp/a,guid=abc"),
            Some(std::path::PathBuf::from("/tmp/a"))
        );
        assert_eq!(a11y_socket("unix:abstract=/tmp/dbus-x"), None);
    }
}

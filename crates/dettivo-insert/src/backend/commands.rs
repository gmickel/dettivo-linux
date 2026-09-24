//! `ydotool` and `xdotool` as optional backends: never installed or
//! configured by Dettivo (FR-P5), run as commands with a timeout, the
//! text handed over on standard input so it never appears in a process
//! list or a log.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{Availability, Backend, Ctx, Failure, Kind, Performed};
use crate::session::Session;
use crate::settings::PasteKeys;

/// How long a helper command may run.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);

/// The inter-key delay xdotool needs when it remaps keycodes for
/// characters outside the layout.
pub const REMAP_DELAY_MS: u64 = 40;

/// Runs `program args` with `stdin` on its standard input and the
/// command timeout. A helper that could not start delivered nothing; one
/// that exited with an error or ran out of time may have typed a prefix,
/// so its failure is `During` and the chain never types the text again.
fn run(program: &str, args: &[String], stdin: Option<&str>) -> Result<(), Failure> {
    run_within(program, args, stdin, COMMAND_TIMEOUT)
}

fn run_within(
    program: &str,
    args: &[String],
    stdin: Option<&str>,
    timeout: Duration,
) -> Result<(), Failure> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failure::Before(format!("{program}: {e}")))?;
    if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let text = text.to_string();
        std::thread::spawn(move || {
            let _ = pipe.write_all(text.as_bytes());
        });
    }
    // Standard error is drained while the command runs, so a chatty helper
    // never blocks on a full pipe; the text is read back after it exits.
    let drain = child.stderr.take().map(|mut stderr| {
        std::thread::spawn(move || {
            let mut err = String::new();
            let _ = std::io::Read::read_to_string(&mut stderr, &mut err);
            err
        })
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                let err = drain.and_then(|h| h.join().ok()).unwrap_or_default();
                return Err(Failure::During(format!(
                    "{program} exited with {status}: {}",
                    err.trim().lines().next().unwrap_or("")
                )));
            }
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Failure::During(format!(
                    "{program} did not finish within {timeout:?}"
                )));
            }
            Err(e) => return Err(Failure::During(format!("{program}: {e}"))),
        }
    }
}

/// `ydotool` through its uinput daemon socket.
pub struct Ydotool;

impl Backend for Ydotool {
    fn name(&self) -> &'static str {
        "ydotool"
    }

    fn kind(&self) -> Kind {
        Kind::Keystroke
    }

    fn availability(&self, session: &Session) -> Availability {
        if !session.has_ydotool {
            return Availability::Unavailable("ydotool is not on PATH".into());
        }
        match &session.ydotool_socket {
            Some(_) => Availability::Available,
            None => Availability::Unavailable(
                "ydotoold is not running (no socket at $YDOTOOL_SOCKET or /run/user/<uid>/.ydotool_socket)".into(),
            ),
        }
    }

    fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure> {
        let args = vec![
            "type".to_string(),
            "--key-delay".to_string(),
            ctx.settings.inter_key_delay_ms.to_string(),
            "--file".to_string(),
            "-".to_string(),
        ];
        ctx.recheck()?;
        run("ydotool", &args, Some(text))?;
        Ok(Performed::typed(text.chars().count()))
    }

    fn paste_keystroke(&self, keys: PasteKeys, _ctx: &Ctx<'_>) -> Result<(), String> {
        // evdev codes: ctrl 29, shift 42, v 47, insert 110.
        let sequence: &[&str] = match keys {
            PasteKeys::CtrlV => &["29:1", "47:1", "47:0", "29:0"],
            PasteKeys::CtrlShiftV => &["29:1", "42:1", "47:1", "47:0", "42:0", "29:0"],
            PasteKeys::ShiftInsert => &["42:1", "110:1", "110:0", "42:0"],
        };
        let mut args = vec!["key".to_string()];
        args.extend(sequence.iter().map(|s| s.to_string()));
        run("ydotool", &args, None).map_err(|f| f.to_string())
    }
}

/// `xdotool` on an X11 display (XTEST).
pub struct Xdotool;

impl Backend for Xdotool {
    fn name(&self) -> &'static str {
        "xdotool"
    }

    fn kind(&self) -> Kind {
        Kind::Keystroke
    }

    fn availability(&self, session: &Session) -> Availability {
        if !session.is_x11() {
            return Availability::Unavailable("no X11 display (DISPLAY is unset)".into());
        }
        if !session.has_xdotool {
            return Availability::Unavailable("xdotool is not on PATH".into());
        }
        Availability::Available
    }

    fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure> {
        // A character outside the X layout makes xdotool remap a spare
        // keycode, and the client picks the new mapping up asynchronously;
        // such a text gets at least the delay that lets it keep up.
        let delay = if text.is_ascii() {
            ctx.settings.inter_key_delay_ms
        } else {
            ctx.settings.inter_key_delay_ms.max(REMAP_DELAY_MS)
        };
        let args = vec![
            "type".to_string(),
            "--delay".to_string(),
            delay.to_string(),
            "--file".to_string(),
            "-".to_string(),
        ];
        ctx.recheck()?;
        run("xdotool", &args, Some(text))?;
        Ok(Performed::typed(text.chars().count()))
    }

    fn paste_keystroke(&self, keys: PasteKeys, _ctx: &Ctx<'_>) -> Result<(), String> {
        let combo = match keys {
            PasteKeys::CtrlV => "ctrl+v",
            PasteKeys::CtrlShiftV => "ctrl+shift+v",
            PasteKeys::ShiftInsert => "shift+Insert",
        };
        run(
            "xdotool",
            &[
                "key".to_string(),
                "--clearmodifiers".to_string(),
                combo.to_string(),
            ],
            None,
        )
        .map_err(|f| f.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_names_the_missing_piece() {
        let none = Session::default();
        assert_eq!(
            Ydotool.availability(&none).reason(),
            Some("ydotool is not on PATH")
        );
        let with_binary = Session {
            has_ydotool: true,
            ..Session::default()
        };
        assert!(
            Ydotool
                .availability(&with_binary)
                .reason()
                .unwrap()
                .contains("ydotoold")
        );
        assert!(
            Xdotool
                .availability(&none)
                .reason()
                .unwrap()
                .contains("DISPLAY")
        );
        let x11 = Session {
            x11_display: Some(":0".into()),
            has_xdotool: true,
            ..Session::default()
        };
        assert!(Xdotool.availability(&x11).is_available());
    }

    /// A helper that could not start delivered nothing (the chain may try
    /// the next backend); one that exited with an error or timed out may
    /// have typed a prefix, so its failure says so and the text is never
    /// typed again.
    #[test]
    fn a_failing_command_says_whether_text_may_have_been_delivered() {
        let err = run("false", &[], None).unwrap_err();
        assert!(
            matches!(&err, Failure::During(e) if e.contains("exited with")),
            "{err:?}"
        );
        let missing = run("nonexistent-dettivo-tool", &[], None).unwrap_err();
        assert!(matches!(missing, Failure::Before(_)), "{missing:?}");
        run("cat", &[], Some("hello")).unwrap();
        let slow = run_within(
            "sleep",
            &["5".to_string()],
            None,
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(
            matches!(&slow, Failure::During(e) if e.contains("did not finish within")),
            "{slow:?}"
        );
    }
}

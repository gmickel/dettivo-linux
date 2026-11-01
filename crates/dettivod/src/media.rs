//! What a dictation does to playing media (`[audio] while_dictating`,
//! ADR 0091): nothing, pause the MPRIS players, duck the default sink to
//! 20 % along a smoothstep ramp, or mute it. It acts only once a take has
//! recorded past [`TAP`], so a tap the hold key discards never touches
//! media, and it runs on a thread of its own, so capture never waits for
//! it. At the end it puts back only what it changed: a volume or mute the
//! user changed during the take stays. A meeting never runs beside a
//! dictation (each refuses to start while the other runs), so nothing is
//! changed under a meeting's recording of the default sink's monitor.

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use dettivo_core::config::schema::WhileDictating;

/// A take must record this long before media is touched: the hold key's
/// minimum hold (ADR 0083), under which a tap is discarded.
pub const TAP: Duration = Duration::from_millis(300);
/// The ducked level of the default sink.
pub const DUCK_LEVEL: f32 = 0.20;
/// How long the duck ramp takes, and its steps.
const RAMP: Duration = Duration::from_millis(200);
const STEPS: u32 = 10;
/// Two volumes this close are the same (wpctl reports two decimals).
const SAME: f32 = 0.015;

/// The default sink's volume.
pub trait Volume: Send {
    /// The volume (1.0 is 100 %) and whether it is muted.
    fn get(&self) -> Result<(f32, bool), String>;
    /// Sets the volume.
    fn set(&self, volume: f32) -> Result<(), String>;
    /// Mutes or unmutes.
    fn mute(&self, muted: bool) -> Result<(), String>;
}

/// The playing media players.
pub trait Players: Send {
    /// Pauses the playing players; their names.
    fn pause(&self) -> Result<Vec<String>, String>;
    /// Resumes the named players.
    fn resume(&self, names: &[String]) -> Result<(), String>;
}

/// The default sink through `wpctl`.
pub struct Wpctl;

fn wpctl(args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("wpctl")
        .args(args)
        .output()
        .map_err(|e| format!("wpctl: {e}"))?;
    if !out.status.success() {
        return Err(format!("wpctl {}: {}", args[0], out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `Volume: 0.40` or `Volume: 0.40 [MUTED]`.
fn parse_volume(text: &str) -> Option<(f32, bool)> {
    let mut words = text.split_whitespace();
    (words.next()? == "Volume:").then_some(())?;
    let volume = words.next()?.parse().ok()?;
    Some((volume, text.contains("[MUTED]")))
}

impl Volume for Wpctl {
    fn get(&self) -> Result<(f32, bool), String> {
        let text = wpctl(&["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
        parse_volume(&text).ok_or_else(|| format!("wpctl: unexpected answer {text:?}"))
    }
    fn set(&self, volume: f32) -> Result<(), String> {
        wpctl(&[
            "set-volume",
            "@DEFAULT_AUDIO_SINK@",
            &format!("{volume:.3}"),
        ])
        .map(drop)
    }
    fn mute(&self, muted: bool) -> Result<(), String> {
        wpctl(&[
            "set-mute",
            "@DEFAULT_AUDIO_SINK@",
            if muted { "1" } else { "0" },
        ])
        .map(drop)
    }
}

/// The MPRIS players on the session bus.
pub struct Mpris;

impl Players for Mpris {
    fn pause(&self) -> Result<Vec<String>, String> {
        dettivo_hotkeys::mpris::pause_playing_blocking(None)
    }
    fn resume(&self, names: &[String]) -> Result<(), String> {
        dettivo_hotkeys::mpris::resume_blocking(None, names).map(drop)
    }
}

/// The action in force: `[hotkeys] pause_media` reads as `pause` when
/// `while_dictating` is left at `nothing`.
pub fn effective(while_dictating: WhileDictating, pause_media: bool) -> WhileDictating {
    match while_dictating {
        WhileDictating::Nothing if pause_media => WhileDictating::Pause,
        other => other,
    }
}

enum Msg {
    Started(WhileDictating),
    Ended,
}

/// What was changed, so the end puts back only that.
#[derive(Debug, PartialEq)]
enum Changed {
    Nothing,
    Paused(Vec<String>),
    Ducked { from: f32, to: f32 },
    Muted,
}

/// The media service: a thread that acts on take starts and ends.
pub struct Media {
    tx: Mutex<Sender<Msg>>,
}

impl Media {
    /// Starts the thread over `volume` and `players`.
    pub fn new(volume: Box<dyn Volume>, players: Box<dyn Players>) -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("media".into())
            .spawn(move || run(&rx, volume.as_ref(), players.as_ref()));
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "media thread not started: media is left alone");
        }
        Self { tx: Mutex::new(tx) }
    }

    /// A take started; `action` is the effective `while_dictating`.
    pub fn started(&self, action: WhileDictating) {
        let _ = self.send(Msg::Started(action));
    }

    /// The take ended (completed, cancelled or failed).
    pub fn ended(&self) {
        let _ = self.send(Msg::Ended);
    }

    fn send(&self, msg: Msg) -> Result<(), mpsc::SendError<Msg>> {
        self.tx.lock().unwrap_or_else(|p| p.into_inner()).send(msg)
    }
}

fn run(rx: &Receiver<Msg>, volume: &dyn Volume, players: &dyn Players) {
    while let Ok(msg) = rx.recv() {
        let Msg::Started(action) = msg else { continue };
        match rx.recv_timeout(TAP) {
            Ok(Msg::Ended) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
            Ok(Msg::Started(_)) | Err(RecvTimeoutError::Timeout) => {}
        }
        let changed = apply(action, volume, players);
        while !matches!(rx.recv(), Ok(Msg::Ended) | Err(_)) {}
        restore(changed, volume, players);
    }
}

fn apply(action: WhileDictating, volume: &dyn Volume, players: &dyn Players) -> Changed {
    let done = match action {
        WhileDictating::Nothing => Ok(Changed::Nothing),
        WhileDictating::Pause => players.pause().map(Changed::Paused),
        WhileDictating::Mute => volume.get().and_then(|(_, muted)| {
            if muted {
                Ok(Changed::Nothing)
            } else {
                volume.mute(true).map(|()| Changed::Muted)
            }
        }),
        WhileDictating::Duck => volume.get().map(|(from, _)| {
            let to = from.min(DUCK_LEVEL);
            if from - to < SAME {
                return Changed::Nothing;
            }
            match ramp(volume, from, to) {
                Ok(()) => Changed::Ducked { from, to },
                Err(why) => {
                    tracing::info!(reason = %why, "duck stopped: the volume is left as it is");
                    Changed::Nothing
                }
            }
        }),
    };
    done.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "media left alone while dictating");
        Changed::Nothing
    })
}

fn restore(changed: Changed, volume: &dyn Volume, players: &dyn Players) {
    let done = match changed {
        Changed::Nothing => Ok(()),
        Changed::Paused(names) if names.is_empty() => Ok(()),
        Changed::Paused(names) => players.resume(&names),
        Changed::Muted => volume
            .get()
            .and_then(|(_, muted)| if muted { volume.mute(false) } else { Ok(()) }),
        Changed::Ducked { from, to } => volume.get().and_then(|(now, _)| {
            if (now - to).abs() > SAME {
                return Ok(());
            }
            ramp(volume, to, from)
        }),
    };
    if let Err(e) = done {
        tracing::warn!(error = %e, "media not put back after dictating");
    }
}

/// Moves the volume from `from` to `to` along a smoothstep over [`RAMP`];
/// gives up when the volume is not the one the last step set, because
/// the user changed it.
fn ramp(volume: &dyn Volume, from: f32, to: f32) -> Result<(), String> {
    let mut last = from;
    for step in 1..=STEPS {
        let (now, _) = volume.get()?;
        if (now - last).abs() > SAME {
            return Err("the volume was changed during the ramp".into());
        }
        let t = step as f32 / STEPS as f32;
        last = from + (to - from) * t * t * (3.0 - 2.0 * t);
        volume.set(last)?;
        std::thread::sleep(RAMP / STEPS);
    }
    Ok(())
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;

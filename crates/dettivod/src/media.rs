//! What a dictation does to playing media (`[audio] while_dictating`,
//! ADR 0091): nothing, pause the MPRIS players, duck the default sink to
//! 20 % along a smoothstep ramp whose steps move at most 0.04, or mute it. It acts only once a take has
//! recorded past [`TAP`], so a tap the hold key discards never touches
//! media, and it runs on a thread of its own, so capture never waits for
//! it. At the end it puts back only what it changed, on the sink it
//! changed: a volume or mute the user changed during the take stays. The
//! end waits for that restore before the take's slot frees, and a meeting
//! starts only on a free slot, so nothing changes under a meeting's
//! recording of the default sink's monitor.

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use dettivo_core::config::schema::WhileDictating;

/// A take must record this long before media is touched: the hold key's
/// minimum hold (ADR 0083), under which a tap is discarded.
pub const TAP: Duration = dettivo_hotkeys::hold::MIN_HOLD;
/// The ducked level of the default sink.
pub const DUCK_LEVEL: f32 = 0.20;
/// The most one ramp step moves the volume, in hundredths: a user change
/// Dettivo detects mid-ramp is shifted by at most this much (ADR 0091).
const MAX_STEP: u32 = 4;
/// The pause after each ramp step.
const STEP: Duration = Duration::from_millis(20);
/// Volumes are set in hundredths, the precision wpctl reports, so two
/// volumes closer than half of one are the same and a change of one
/// hundredth is the user's.
const SAME: f32 = 0.005;

/// A sink's volume, addressed by the id [`Volume::default_sink`] gave.
pub trait Volume: Send {
    /// The default sink's id now.
    fn default_sink(&self) -> Result<String, String>;
    /// The volume (1.0 is 100 %) and whether it is muted.
    fn get(&self, sink: &str) -> Result<(f32, bool), String>;
    /// Moves the volume by `by` from the one the sink has when the move
    /// lands, so a volume the user set since the last read is moved, not
    /// replaced.
    fn change(&self, sink: &str, by: f32) -> Result<(), String>;
    /// Mutes or unmutes.
    fn mute(&self, sink: &str, muted: bool) -> Result<(), String>;
}

/// The playing media players.
pub trait Players: Send {
    /// Pauses the playing players; their names.
    fn pause(&self) -> Result<Vec<String>, String>;
    /// Resumes the named players.
    fn resume(&self, names: &[String]) -> Result<(), String>;
}

/// The sinks through `wpctl`.
pub struct Wpctl;

/// How long one `wpctl` run may take before it is killed.
const WPCTL_TIMEOUT: Duration = Duration::from_secs(2);

fn wpctl(args: &[&str]) -> Result<String, String> {
    use std::io::Read;
    let mut child = std::process::Command::new("wpctl")
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("wpctl: {e}"))?;
    let deadline = std::time::Instant::now() + WPCTL_TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|e| format!("wpctl: {e}"))? {
            Some(status) => break status,
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("wpctl {}: no answer within 2 s", args[0]));
            }
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    };
    if !status.success() {
        return Err(format!("wpctl {}: {status}", args[0]));
    }
    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout
            .read_to_string(&mut out)
            .map_err(|e| format!("wpctl: {e}"))?;
    }
    Ok(out)
}

/// `Volume: 0.40` or `Volume: 0.40 [MUTED]`.
fn parse_volume(text: &str) -> Option<(f32, bool)> {
    let mut words = text.split_whitespace();
    (words.next()? == "Volume:").then_some(())?;
    let volume = words.next()?.parse().ok()?;
    Some((volume, text.contains("[MUTED]")))
}

/// `id 52, type PipeWire:Interface:Node` from `wpctl inspect`.
fn parse_id(text: &str) -> Option<String> {
    let id = text.trim_start().strip_prefix("id ")?.split(',').next()?;
    id.parse::<u32>().ok().map(|n| n.to_string())
}

impl Volume for Wpctl {
    fn default_sink(&self) -> Result<String, String> {
        let text = wpctl(&["inspect", "@DEFAULT_AUDIO_SINK@"])?;
        parse_id(&text).ok_or_else(|| "wpctl: no default sink id".to_string())
    }
    fn get(&self, sink: &str) -> Result<(f32, bool), String> {
        let text = wpctl(&["get-volume", sink])?;
        parse_volume(&text).ok_or_else(|| format!("wpctl: unexpected answer {text:?}"))
    }
    fn change(&self, sink: &str, by: f32) -> Result<(), String> {
        let sign = if by < 0.0 { '-' } else { '+' };
        wpctl(&["set-volume", sink, &format!("{:.2}{sign}", by.abs())]).map(drop)
    }
    fn mute(&self, sink: &str, muted: bool) -> Result<(), String> {
        wpctl(&["set-mute", sink, if muted { "1" } else { "0" }]).map(drop)
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
    /// The take ended; the sender hears once media is back.
    Ended(Sender<()>),
}

/// What was changed, and where, so the end puts back only that.
#[derive(Debug, PartialEq)]
enum Changed {
    Nothing,
    Paused(Vec<String>),
    Ducked { sink: String, from: f32, to: f32 },
    Muted { sink: String },
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

    /// The take ended (completed, cancelled or failed): returns once the
    /// media work of the take is done and media is back, so the session
    /// holds its slot, and a meeting stays out, until then. Every MPRIS
    /// call and `wpctl` run is bounded by its own timeout; a media thread
    /// that died ends the wait.
    pub fn ended(&self) {
        let (done, back) = mpsc::channel();
        if self.send(Msg::Ended(done)).is_ok() {
            let _ = back.recv();
        }
    }

    fn send(&self, msg: Msg) -> Result<(), mpsc::SendError<Msg>> {
        self.tx.lock().unwrap_or_else(|p| p.into_inner()).send(msg)
    }
}

fn run(rx: &Receiver<Msg>, volume: &dyn Volume, players: &dyn Players) {
    while let Ok(msg) = rx.recv() {
        let action = match msg {
            Msg::Started(action) => action,
            Msg::Ended(done) => {
                let _ = done.send(());
                continue;
            }
        };
        match rx.recv_timeout(TAP) {
            Ok(Msg::Ended(done)) => {
                let _ = done.send(());
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
            Ok(Msg::Started(_)) | Err(RecvTimeoutError::Timeout) => {}
        }
        let changed = apply(action, volume, players);
        let done = loop {
            match rx.recv() {
                Ok(Msg::Ended(done)) => break Some(done),
                Ok(Msg::Started(_)) => {}
                Err(_) => break None,
            }
        };
        restore(changed, volume, players);
        if let Some(done) = done {
            let _ = done.send(());
        }
    }
}

fn apply(action: WhileDictating, volume: &dyn Volume, players: &dyn Players) -> Changed {
    let done = match action {
        WhileDictating::Nothing => Ok(Changed::Nothing),
        WhileDictating::Pause => players.pause().map(Changed::Paused),
        WhileDictating::Mute => volume.default_sink().and_then(|sink| {
            let (_, muted) = volume.get(&sink)?;
            if muted {
                Ok(Changed::Nothing)
            } else {
                volume.mute(&sink, true).map(|()| Changed::Muted { sink })
            }
        }),
        WhileDictating::Duck => volume.default_sink().and_then(|sink| {
            let (from, _) = volume.get(&sink)?;
            let to = from.min(DUCK_LEVEL);
            if from - to < SAME {
                return Ok(Changed::Nothing);
            }
            Ok(match ramp(volume, &sink, from, to) {
                Ok(()) => Changed::Ducked { sink, from, to },
                Err(why) => {
                    tracing::info!(reason = %why, "duck stopped: the volume is left as it is");
                    Changed::Nothing
                }
            })
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
        Changed::Muted { sink } => volume.get(&sink).and_then(|(_, muted)| {
            if muted {
                volume.mute(&sink, false)
            } else {
                Ok(())
            }
        }),
        Changed::Ducked { sink, from, to } => volume.get(&sink).and_then(|(now, _)| {
            if (now - to).abs() > SAME {
                return Ok(());
            }
            ramp(volume, &sink, to, from)
        }),
    };
    if let Err(e) = done {
        tracing::warn!(error = %e, "media not put back after dictating");
    }
}

/// The volume in hundredths, the precision `wpctl` reports.
fn cents(volume: f32) -> i32 {
    (volume * 100.0).round() as i32
}

/// The steps of a ramp over `distance` hundredths: the smoothstep's
/// steepest step is 1.5 times the average one, so this many keep every
/// step, rounded to hundredths, within [`MAX_STEP`].
fn steps(distance: i32) -> u32 {
    (distance.unsigned_abs() * 3).div_ceil(2 * MAX_STEP).max(1)
}

/// Moves the sink's volume from `from` to `to` along a smoothstep, in
/// hundredths, no step more than [`MAX_STEP`], [`STEP`] apart; gives up
/// when the volume is not the one the last step left, because the user
/// changed it. Each step is a relative move: `wpctl` has no
/// compare-and-set, and a volume the user sets between a step's read and
/// its move is then moved by that one step instead of replaced, so the
/// next read sees it.
fn ramp(volume: &dyn Volume, sink: &str, from: f32, to: f32) -> Result<(), String> {
    let (start, distance) = (cents(from), cents(to) - cents(from));
    let n = steps(distance);
    let mut last = start;
    for step in 1..=n {
        let (now, _) = volume.get(sink)?;
        if cents(now) != last {
            return Err("the volume was changed during the ramp".into());
        }
        let t = step as f32 / n as f32;
        let next = start + (distance as f32 * t * t * (3.0 - 2.0 * t)).round() as i32;
        if next != last {
            volume.change(sink, (next - last) as f32 / 100.0)?;
        }
        last = next;
        std::thread::sleep(STEP);
    }
    Ok(())
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;

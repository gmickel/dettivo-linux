//! fn-85 R5: ducking ramps the default sink down and back smoothly, on
//! the sink it lowered, never overrides a volume the user set, never acts
//! on a tap, the end returns only once media is back, and `pause_media`
//! reads as `pause`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::*;

/// A sink: its volume, its mute and the volume after every change;
/// `user_at` makes the user set a volume just after the n-th change,
/// `user_before` just before it, after the ramp's read.
#[derive(Debug, Default, Clone)]
struct Sink {
    volume: f32,
    muted: bool,
    sets: Vec<f32>,
    user_at: Option<(usize, f32)>,
    user_before: Option<(usize, f32)>,
}

#[derive(Default)]
struct Sinks {
    default: String,
    by_id: BTreeMap<String, Sink>,
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Sinks>>);

impl Fake {
    fn sink(&self, id: &str) -> Sink {
        self.0.lock().unwrap().by_id[id].clone()
    }
    fn with(&self, id: &str, f: impl FnOnce(&mut Sink)) {
        f(self.0.lock().unwrap().by_id.get_mut(id).unwrap());
    }
}

fn hundredths(volume: f32) -> f32 {
    (volume * 100.0).round() / 100.0
}

/// How long the default duck, 0.80 to 0.20, takes, with slack.
fn duck_time() -> Duration {
    STEP * steps(60) + Duration::from_millis(200)
}

impl Volume for Fake {
    fn default_sink(&self) -> Result<String, String> {
        Ok(self.0.lock().unwrap().default.clone())
    }
    fn get(&self, sink: &str) -> Result<(f32, bool), String> {
        let s = self.sink(sink);
        Ok((s.volume, s.muted))
    }
    fn change(&self, sink: &str, by: f32) -> Result<(), String> {
        self.with(sink, |s| {
            if let Some((n, user)) = s.user_before
                && s.sets.len() + 1 == n
            {
                s.volume = user;
            }
            s.volume = hundredths(s.volume + by);
            s.sets.push(s.volume);
            if let Some((n, user)) = s.user_at
                && s.sets.len() == n
            {
                s.volume = user;
            }
        });
        Ok(())
    }
    fn mute(&self, sink: &str, muted: bool) -> Result<(), String> {
        self.with(sink, |s| s.muted = muted);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakePlayers(Arc<Mutex<Vec<String>>>);

impl Players for FakePlayers {
    fn pause(&self) -> Result<Vec<String>, String> {
        self.0.lock().unwrap().push("pause".into());
        Ok(vec!["org.mpris.MediaPlayer2.mpv".into()])
    }
    fn resume(&self, names: &[String]) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .push(format!("resume {}", names.join(",")));
        Ok(())
    }
}

/// Sink `a` at `volume` as the default, sink `b` at 0.2.
fn sinks(volume: f32) -> Fake {
    let fake = Fake::default();
    let mut s = fake.0.lock().unwrap();
    s.default = "a".into();
    for (id, v) in [("a", volume), ("b", 0.2)] {
        s.by_id.insert(
            id.into(),
            Sink {
                volume: v,
                ..Sink::default()
            },
        );
    }
    drop(s);
    fake
}

/// The media thread over the fakes, and its sender.
fn spawn(volume: &Fake, players: &FakePlayers) -> (Sender<Msg>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let (v, p) = (volume.clone(), players.clone());
    (tx, std::thread::spawn(move || run(&rx, &v, &p)))
}

/// Ends the take and waits for the media thread's acknowledgement, as
/// the session's end does.
fn end(tx: &Sender<Msg>) {
    let (done, back) = mpsc::channel();
    tx.send(Msg::Ended(done)).unwrap();
    back.recv_timeout(Duration::from_secs(5))
        .expect("the end is acknowledged");
}

/// A take held past the tap and the ramp, then ended; the state the
/// moment the end is acknowledged is what a meeting starting next sees.
fn take(volume: &Fake, players: &FakePlayers, action: WhileDictating, mid: impl FnOnce()) {
    let (tx, thread) = spawn(volume, players);
    tx.send(Msg::Started(action)).unwrap();
    std::thread::sleep(TAP + duck_time());
    mid();
    end(&tx);
    let after = volume.0.lock().unwrap().by_id.clone();
    drop(tx);
    thread.join().unwrap();
    assert_eq!(
        format!("{:?}", volume.0.lock().unwrap().by_id),
        format!("{after:?}"),
        "media was back when the end returned"
    );
}

/// The default duck, 0.80 to 0.20, and its restore: eased, and no step
/// moves the volume more than 0.04.
#[test]
fn duck_ramps_down_to_the_duck_level_and_back_smoothly() {
    let volume = sinks(0.8);
    let mut ducked = 0.0;
    take(
        &volume,
        &FakePlayers::default(),
        WhileDictating::Duck,
        || {
            ducked = volume.sink("a").volume;
        },
    );
    assert!((ducked - DUCK_LEVEL).abs() < 1e-6, "{ducked}");
    let a = volume.sink("a");
    assert!((a.volume - 0.8).abs() < 1e-6, "restored to {}", a.volume);
    let bottom = a.sets.iter().position(|&v| v == DUCK_LEVEL).unwrap();
    let (down, up) = a.sets.split_at(bottom + 1);
    assert!(down.windows(2).all(|w| w[1] < w[0]), "{down:?}");
    assert!(up.windows(2).all(|w| w[1] > w[0]), "{up:?}");
    let moves: Vec<f32> = std::iter::once(0.8)
        .chain(a.sets.iter().copied())
        .collect::<Vec<_>>()
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .collect();
    assert!(moves.iter().all(|&m| m <= 0.04 + 1e-4), "{moves:?}");
    // A smoothstep: the first and last steps of the duck are its smallest.
    let largest = moves[..down.len()].iter().copied().fold(0.0, f32::max);
    assert!(
        moves[0] < largest && moves[down.len() - 1] < largest,
        "{moves:?}"
    );
}

/// A volume the user sets during the ramp or during the take stays, down
/// to a change of one hundredth.
#[test]
fn a_volume_the_user_set_is_never_overridden() {
    let volume = sinks(0.8);
    volume.with("a", |s| s.user_at = Some((3, 0.5)));
    take(
        &volume,
        &FakePlayers::default(),
        WhileDictating::Duck,
        || {},
    );
    let a = volume.sink("a");
    assert_eq!(a.volume, 0.5);
    assert_eq!(a.sets.len(), 3, "the ramp gave up and nothing was put back");

    for user in [0.6, 0.21] {
        let volume = sinks(0.8);
        take(
            &volume,
            &FakePlayers::default(),
            WhileDictating::Duck,
            || {
                volume.with("a", |s| s.volume = user);
            },
        );
        let a = volume.sink("a");
        assert_eq!(a.volume, user);
        assert!(a.sets.windows(2).all(|w| w[1] < w[0]), "no ramp back up");
    }
}

/// A volume the user sets between a step's read and its move is moved by
/// that one step, at most 0.04, never replaced: the ramp stops there and
/// nothing is put back. A set that lands inside one `wpctl` call can still
/// be lost; PipeWire has no compare-and-set (ADR 0091).
#[test]
fn a_volume_set_between_a_steps_read_and_its_set_stays_within_a_step() {
    // An early, small step and one from the steep middle of the curve.
    for n in [2, 10] {
        let volume = sinks(0.8);
        volume.with("a", |s| s.user_before = Some((n, 0.66)));
        take(
            &volume,
            &FakePlayers::default(),
            WhileDictating::Duck,
            || {},
        );
        let a = volume.sink("a");
        assert!((a.volume - 0.66).abs() <= 0.04 + 1e-4, "{n}: {a:?}");
        assert_eq!(a.sets.len(), n, "the ramp gave up and nothing was put back");
    }
}

/// The default sink changing during the take leaves the new one alone:
/// the restore goes to the sink Dettivo lowered or muted.
#[test]
fn the_restore_goes_to_the_sink_that_was_changed() {
    for action in [WhileDictating::Duck, WhileDictating::Mute] {
        let volume = sinks(0.8);
        volume.with("b", |s| s.muted = true);
        take(&volume, &FakePlayers::default(), action, || {
            volume.0.lock().unwrap().default = "b".into();
        });
        let (a, b) = (volume.sink("a"), volume.sink("b"));
        assert!(
            (a.volume - 0.8).abs() < 1e-6 && !a.muted,
            "{action:?}: {a:?}"
        );
        assert!(b.sets.is_empty() && b.muted, "{action:?}: {b:?}");
    }
}

/// A take that ends within the tap window touches nothing, for every
/// action.
#[test]
fn a_tap_never_touches_media() {
    for action in [
        WhileDictating::Duck,
        WhileDictating::Mute,
        WhileDictating::Pause,
    ] {
        let (volume, players) = (sinks(0.8), FakePlayers::default());
        let (tx, thread) = spawn(&volume, &players);
        tx.send(Msg::Started(action)).unwrap();
        std::thread::sleep(TAP / 3);
        end(&tx);
        drop(tx);
        thread.join().unwrap();
        let a = volume.sink("a");
        assert!(a.sets.is_empty() && !a.muted, "{action:?}");
        assert!(players.0.lock().unwrap().is_empty(), "{action:?}");
    }
}

#[test]
fn pause_resumes_what_it_paused_and_mute_unmutes_only_its_own_mute() {
    let players = FakePlayers::default();
    take(&sinks(0.8), &players, WhileDictating::Pause, || {});
    assert_eq!(
        *players.0.lock().unwrap(),
        ["pause", "resume org.mpris.MediaPlayer2.mpv"]
    );

    let volume = sinks(0.8);
    let mut muted_mid = false;
    take(&volume, &players, WhileDictating::Mute, || {
        muted_mid = volume.sink("a").muted;
    });
    assert!(muted_mid && !volume.sink("a").muted);

    let volume = sinks(0.8);
    volume.with("a", |s| s.muted = true);
    take(&volume, &players, WhileDictating::Mute, || {});
    assert!(volume.sink("a").muted, "the user's own mute stays");
}

#[test]
fn pause_media_reads_as_pause_and_wpctl_answers_parse() {
    assert_eq!(
        effective(WhileDictating::Nothing, true),
        WhileDictating::Pause
    );
    assert_eq!(effective(WhileDictating::Duck, true), WhileDictating::Duck);
    assert_eq!(
        effective(WhileDictating::Nothing, false),
        WhileDictating::Nothing
    );
    assert_eq!(parse_volume("Volume: 0.40\n"), Some((0.40, false)));
    assert_eq!(parse_volume("Volume: 1.00 [MUTED]\n"), Some((1.0, true)));
    assert_eq!(parse_volume("nonsense"), None);
    assert_eq!(
        parse_id("id 52, type PipeWire:Interface:Node\n").as_deref(),
        Some("52")
    );
    assert_eq!(parse_id("nonsense"), None);
}

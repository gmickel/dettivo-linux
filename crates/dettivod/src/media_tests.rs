//! fn-85 R5: ducking ramps the default sink down and back smoothly,
//! never overrides a volume the user set, never acts on a tap, and
//! `pause_media` reads as `pause`.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::*;

/// A sink: its volume, its mute and every volume set on it; `user_at`
/// makes the user set `user_volume` on the n-th set.
#[derive(Default)]
struct Sink {
    volume: f32,
    muted: bool,
    sets: Vec<f32>,
    user_at: Option<(usize, f32)>,
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Sink>>);

impl Volume for Fake {
    fn get(&self) -> Result<(f32, bool), String> {
        let s = self.0.lock().unwrap();
        Ok((s.volume, s.muted))
    }
    fn set(&self, volume: f32) -> Result<(), String> {
        let mut s = self.0.lock().unwrap();
        s.volume = volume;
        s.sets.push(volume);
        if let Some((n, user)) = s.user_at
            && s.sets.len() == n
        {
            s.volume = user;
        }
        Ok(())
    }
    fn mute(&self, muted: bool) -> Result<(), String> {
        self.0.lock().unwrap().muted = muted;
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

fn sink(volume: f32) -> Fake {
    let fake = Fake::default();
    fake.0.lock().unwrap().volume = volume;
    fake
}

/// The media thread over the fakes, and its sender.
fn spawn(volume: &Fake, players: &FakePlayers) -> (Sender<Msg>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let (v, p) = (volume.clone(), players.clone());
    (tx, std::thread::spawn(move || run(&rx, &v, &p)))
}

/// A take held past the tap and the ramp, then ended.
fn take(volume: &Fake, players: &FakePlayers, action: WhileDictating, mid: impl FnOnce()) {
    let (tx, thread) = spawn(volume, players);
    tx.send(Msg::Started(action)).unwrap();
    std::thread::sleep(TAP + RAMP + Duration::from_millis(200));
    mid();
    tx.send(Msg::Ended).unwrap();
    drop(tx);
    thread.join().unwrap();
}

#[test]
fn duck_ramps_down_to_the_duck_level_and_back_smoothly() {
    let volume = sink(0.8);
    let mut ducked = 0.0;
    take(
        &volume,
        &FakePlayers::default(),
        WhileDictating::Duck,
        || {
            ducked = volume.0.lock().unwrap().volume;
        },
    );
    assert!((ducked - DUCK_LEVEL).abs() < 1e-6, "{ducked}");
    let s = volume.0.lock().unwrap();
    assert!((s.volume - 0.8).abs() < 1e-6, "restored to {}", s.volume);
    let (down, up) = s.sets.split_at(STEPS as usize);
    assert!(down.windows(2).all(|w| w[1] < w[0]), "{down:?}");
    assert!(up.windows(2).all(|w| w[1] > w[0]), "{up:?}");
    // A smoothstep: small first and last steps, the largest in the middle.
    let steps: Vec<f32> = std::iter::once(0.8)
        .chain(down.iter().copied())
        .collect::<Vec<_>>()
        .windows(2)
        .map(|w| w[0] - w[1])
        .collect();
    assert!(steps[0] < steps[4] && steps[9] < steps[5], "{steps:?}");
}

/// A volume the user sets during the ramp or during the take stays.
#[test]
fn a_volume_the_user_set_is_never_overridden() {
    let volume = sink(0.8);
    volume.0.lock().unwrap().user_at = Some((3, 0.5));
    take(
        &volume,
        &FakePlayers::default(),
        WhileDictating::Duck,
        || {},
    );
    let s = volume.0.lock().unwrap();
    assert_eq!(s.volume, 0.5);
    assert_eq!(s.sets.len(), 3, "the ramp gave up and nothing was put back");
    drop(s);

    let volume = sink(0.8);
    take(
        &volume,
        &FakePlayers::default(),
        WhileDictating::Duck,
        || {
            volume.0.lock().unwrap().volume = 0.6;
        },
    );
    let s = volume.0.lock().unwrap();
    assert_eq!(s.volume, 0.6);
    assert_eq!(s.sets.len(), STEPS as usize, "no ramp back up");
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
        let (volume, players) = (sink(0.8), FakePlayers::default());
        let (tx, thread) = spawn(&volume, &players);
        tx.send(Msg::Started(action)).unwrap();
        std::thread::sleep(TAP / 3);
        tx.send(Msg::Ended).unwrap();
        drop(tx);
        thread.join().unwrap();
        let s = volume.0.lock().unwrap();
        assert!(s.sets.is_empty() && !s.muted, "{action:?}");
        assert!(players.0.lock().unwrap().is_empty(), "{action:?}");
    }
}

#[test]
fn pause_resumes_what_it_paused_and_mute_unmutes_only_its_own_mute() {
    let players = FakePlayers::default();
    take(&sink(0.8), &players, WhileDictating::Pause, || {});
    assert_eq!(
        *players.0.lock().unwrap(),
        ["pause", "resume org.mpris.MediaPlayer2.mpv"]
    );

    let volume = sink(0.8);
    let mut muted_mid = false;
    take(&volume, &players, WhileDictating::Mute, || {
        muted_mid = volume.0.lock().unwrap().muted;
    });
    assert!(muted_mid && !volume.0.lock().unwrap().muted);

    let volume = sink(0.8);
    volume.0.lock().unwrap().muted = true;
    take(&volume, &players, WhileDictating::Mute, || {});
    assert!(volume.0.lock().unwrap().muted, "the user's own mute stays");
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
}

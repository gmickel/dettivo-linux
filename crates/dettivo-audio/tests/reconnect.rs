//! A PipeWire restart while the daemon runs: a private PipeWire with one
//! virtual microphone is stopped under a running capture and started
//! again. The capture ends with an error naming PipeWire, the service says
//! it is disconnected and refuses to open, and once PipeWire is back it
//! reconnects on its own and captures again.
//!
//! The test never reaches the user's PipeWire: the process points
//! `PIPEWIRE_RUNTIME_DIR` at a private directory before the service
//! connects, and it opens the default source only once the graph names
//! the private microphone as the default, which no other graph does.
//! Skipped (and said so) where `pipewire` or `wireplumber` is not
//! installed, as in the CI container: without a session manager nothing
//! links a capture.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use dettivo_audio::service::{AudioService, CaptureHandle};
use dettivo_audio::{CaptureError, EndReason, Event, Target};

const MIC: &str = "dettivo-reconnect-test-mic";

fn installed(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// A private PipeWire (and WirePlumber when installed) under `root`.
struct Private {
    root: PathBuf,
    children: Vec<Child>,
}

impl Private {
    fn new(root: &Path) -> Self {
        let pw = root.join("config/pipewire/pipewire.conf.d");
        let wp = root.join("config/wireplumber/wireplumber.conf.d");
        std::fs::create_dir_all(&pw).unwrap();
        std::fs::create_dir_all(&wp).unwrap();
        std::fs::create_dir_all(root.join("run")).unwrap();
        std::fs::create_dir_all(root.join("state")).unwrap();
        std::fs::write(
            pw.join("test.conf"),
            format!(
                "context.objects = [\n  {{ factory = adapter\n    args = {{ factory.name = support.null-audio-sink node.name = \"{MIC}\"\n             media.class = \"Audio/Source/Virtual\" audio.position = [ MONO ]\n             object.linger = true priority.session = 2000 }} }}\n  {{ factory = adapter\n    args = {{ factory.name = support.null-audio-sink node.name = \"dettivo-reconnect-test-out\"\n             media.class = \"Audio/Sink\" audio.position = [ FL FR ] object.linger = true }} }}\n]\n"
            ),
        )
        .unwrap();
        std::fs::write(
            wp.join("test.conf"),
            "wireplumber.profiles = { main = { monitor.alsa = disabled monitor.alsa-midi = disabled monitor.bluez = disabled monitor.bluez.midi = disabled monitor.libcamera = disabled monitor.v4l2 = disabled } }\n",
        )
        .unwrap();
        Self {
            root: root.to_path_buf(),
            children: Vec::new(),
        }
    }

    fn spawn(&self, bin: &str) -> Child {
        Command::new(bin)
            .env("PIPEWIRE_RUNTIME_DIR", self.root.join("run"))
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env_remove("PIPEWIRE_REMOTE")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("{bin}: {e}"))
    }

    fn up(&mut self) {
        let socket = self.root.join("run/pipewire-0");
        let _ = std::fs::remove_file(&socket);
        self.children.push(self.spawn("pipewire"));
        wait_for(
            "the private PipeWire socket",
            Duration::from_secs(5),
            || socket.exists(),
        );
        self.children.push(self.spawn("wireplumber"));
    }

    fn down(&mut self) {
        for mut child in self.children.drain(..).rev() {
            let _ = Command::new("kill").arg(child.id().to_string()).status();
            let _ = child.wait();
        }
    }
}

impl Drop for Private {
    fn drop(&mut self) {
        self.down();
    }
}

fn wait_for(what: &str, within: Duration, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + within;
    while !done() {
        assert!(Instant::now() < until, "{what} within {within:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The next `Ended` or the first PCM, whichever `want` asks for.
fn next_matching(
    capture: &CaptureHandle,
    within: Duration,
    want: impl Fn(&Event) -> bool,
) -> Option<Event> {
    let until = Instant::now() + within;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match capture.events().recv_timeout(left) {
            Ok(event) if want(&event) => return Some(event),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

/// True once the graph the service sees names the private microphone as
/// the default source: only then may the default be opened.
fn private_default(service: &AudioService) -> bool {
    service.snapshot().default_source.as_deref() == Some(MIC)
}

fn open_mic(service: &AudioService) -> Result<CaptureHandle, CaptureError> {
    assert!(
        private_default(service),
        "only the private microphone is opened"
    );
    service.open(Target::Default, 50)
}

fn hears_audio(capture: &CaptureHandle) -> bool {
    next_matching(capture, Duration::from_secs(5), |e| {
        matches!(e, Event::Pcm { .. })
    })
    .is_some()
}

#[test]
fn capture_recovers_after_pipewire_restarts() {
    if !installed("pipewire") || !installed("wireplumber") {
        eprintln!("skipped: pipewire or wireplumber is not installed");
        return;
    }
    // A short directory: a PipeWire socket path must fit in 108 bytes.
    let dir = tempfile::Builder::new()
        .prefix("dq")
        .tempdir_in("/tmp")
        .unwrap();
    let mut private = Private::new(dir.path());
    // SAFETY: this binary holds this one test and sets the variables
    // before the service or any other thread of ours exists, so nothing
    // reads the environment concurrently.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("PIPEWIRE_RUNTIME_DIR", dir.path().join("run"));
        std::env::remove_var("PIPEWIRE_REMOTE");
    }
    private.up();

    let service = AudioService::start().expect("the private PipeWire is reachable");
    wait_for("the private microphone", Duration::from_secs(5), || {
        private_default(&service)
    });
    let capture = open_mic(&service).expect("the private microphone opens");
    assert!(hears_audio(&capture), "audio before the restart");

    private.down();
    let ended = next_matching(&capture, Duration::from_secs(3), |e| {
        matches!(e, Event::Ended { .. })
    })
    .expect("the capture ends when PipeWire goes away");
    match ended {
        Event::Ended {
            reason: EndReason::Error(why),
        } => assert!(why.contains("PipeWire"), "the end names PipeWire: {why}"),
        other => panic!("ended with an error, got {other:?}"),
    }
    wait_for("the service to notice", Duration::from_secs(2), || {
        !service.connected()
    });
    assert!(
        matches!(
            service.open(Target::Node(MIC.into()), 50),
            Err(CaptureError::NoPipeWire(_))
        ),
        "an open while PipeWire is gone names PipeWire"
    );
    assert!(service.snapshot().nodes.is_empty(), "no stale nodes");

    private.up();
    wait_for("the service to reconnect", Duration::from_secs(10), || {
        service.connected() && private_default(&service)
    });
    let again = open_mic(&service).expect("the microphone opens after the restart");
    assert!(hears_audio(&again), "audio after the restart");
    drop(again);
    service.shutdown();
}

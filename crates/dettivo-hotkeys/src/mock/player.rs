//! The mock MPRIS player: `org.mpris.MediaPlayer2.<suffix>` with a
//! `PlaybackStatus` the test reads back and `Pause`, `Play`, `PlayPause`
//! and `Stop` that change it.

use std::sync::{Arc, Mutex};

use zbus::interface;

use super::Served;

struct PlayerIface {
    status: Arc<Mutex<String>>,
}

impl PlayerIface {
    fn set(&self, status: &str) {
        *self.status.lock().unwrap_or_else(|p| p.into_inner()) = status.to_string();
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerIface {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn pause(&self) {
        self.set("Paused");
    }

    fn play(&self) {
        self.set("Playing");
    }

    fn play_pause(&self) {
        let now = self.playback_status();
        self.set(if now == "Playing" {
            "Paused"
        } else {
            "Playing"
        });
    }

    fn stop(&self) {
        self.set("Stopped");
    }
}

/// An MPRIS player on the bus; its name vanishes when it is dropped.
pub struct MockPlayer {
    _served: Served,
    status: Arc<Mutex<String>>,
}

impl MockPlayer {
    /// Serves `org.mpris.MediaPlayer2.<suffix>` with an initial status.
    pub fn serve(address: &str, suffix: &str, status: &str) -> Result<Self, String> {
        let shared = Arc::new(Mutex::new(status.to_string()));
        let name = format!("{}{suffix}", crate::mpris::PREFIX);
        let iface_status = shared.clone();
        let served = Served::start(
            move |b| {
                b.name(name)?.serve_at(
                    "/org/mpris/MediaPlayer2",
                    PlayerIface {
                        status: iface_status,
                    },
                )
            },
            address.to_string(),
        )?;
        Ok(Self {
            _served: served,
            status: shared,
        })
    }

    /// The status as the player reports it now.
    pub fn status(&self) -> String {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

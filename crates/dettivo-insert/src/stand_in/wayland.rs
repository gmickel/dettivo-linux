//! A stand-in Wayland compositor on a socket of its own: the registry
//! with the globals a test asks for, `wl_display.sync`, binds, the
//! virtual keyboard (its key presses are counted) and the input-idle
//! notifier. Requests it does not know are read and ignored.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{Server, bind};

/// A global the compositor advertises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Global {
    /// `wl_seat` version 7.
    Seat,
    /// `zwp_virtual_keyboard_manager_v1`.
    VirtualKeyboard,
    /// `ext_idle_notifier_v1` version 2.
    IdleNotifier,
}

impl Global {
    fn interface(self) -> (&'static str, u32) {
        match self {
            Self::Seat => ("wl_seat", 7),
            Self::VirtualKeyboard => ("zwp_virtual_keyboard_manager_v1", 1),
            Self::IdleNotifier => ("ext_idle_notifier_v1", 2),
        }
    }
}

/// The compositor.
pub struct WaylandStandIn {
    socket: PathBuf,
    presses: Arc<AtomicUsize>,
    clients: Arc<AtomicUsize>,
    server: Server,
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_ne_bytes(bytes[at..at + 4].try_into().unwrap_or_default())
}

fn message(object: u32, opcode: u16, args: &[u8]) -> Vec<u8> {
    let size = 8 + args.len() as u32;
    let mut m = object.to_ne_bytes().to_vec();
    m.extend((size << 16 | u32::from(opcode)).to_ne_bytes());
    m.extend(args);
    m
}

fn string(text: &str) -> Vec<u8> {
    let mut s = ((text.len() + 1) as u32).to_ne_bytes().to_vec();
    s.extend(text.as_bytes());
    s.push(0);
    while s.len() % 4 != 0 {
        s.push(0);
    }
    s
}

/// One client: objects by id with their interface.
fn serve(mut stream: UnixStream, globals: &[Global], presses: &AtomicUsize) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut objects: HashMap<u32, &'static str> = HashMap::from([(1, "wl_display")]);
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    while let Ok(n @ 1..) = stream.read(&mut chunk) {
        buf.extend_from_slice(&chunk[..n]);
        while buf.len() >= 8 && buf.len() >= (word(&buf, 4) >> 16) as usize {
            let size = ((word(&buf, 4) >> 16) as usize).max(8);
            let msg: Vec<u8> = buf.drain(..size).collect();
            let (object, opcode) = (word(&msg, 0), word(&msg, 4) & 0xffff);
            let mut out = Vec::new();
            match (objects.get(&object).copied(), opcode) {
                (Some("wl_display"), 0) => {
                    let callback = word(&msg, 8);
                    out.extend(message(callback, 0, &0u32.to_ne_bytes()));
                    out.extend(message(1, 1, &callback.to_ne_bytes()));
                }
                (Some("wl_display"), 1) => {
                    let registry = word(&msg, 8);
                    objects.insert(registry, "wl_registry");
                    for (i, global) in globals.iter().enumerate() {
                        let (interface, version) = global.interface();
                        let mut args = (i as u32 + 1).to_ne_bytes().to_vec();
                        args.extend(string(interface));
                        args.extend(version.to_ne_bytes());
                        out.extend(message(registry, 0, &args));
                    }
                }
                (Some("wl_registry"), 0) => {
                    let name = word(&msg, 8) as usize;
                    let id = word(&msg, msg.len() - 4);
                    if let Some(global) = globals.get(name.wrapping_sub(1)) {
                        objects.insert(id, global.interface().0);
                    }
                }
                (Some("zwp_virtual_keyboard_manager_v1"), 0) => {
                    objects.insert(word(&msg, 12), "zwp_virtual_keyboard_v1");
                }
                (Some("zwp_virtual_keyboard_v1"), 1) if word(&msg, 16) == 1 => {
                    presses.fetch_add(1, Ordering::Relaxed);
                }
                (Some("ext_idle_notifier_v1"), 1 | 2) => {
                    objects.insert(word(&msg, 8), "ext_idle_notification_v1");
                }
                _ => {}
            }
            if !out.is_empty() && writer.write_all(&out).is_err() {
                return;
            }
        }
    }
}

impl WaylandStandIn {
    /// Starts on `socket` (a stale file there is replaced) advertising
    /// `globals`.
    pub fn start(socket: &Path, globals: &[Global]) -> std::io::Result<Self> {
        let presses = Arc::new(AtomicUsize::new(0));
        let clients = Arc::new(AtomicUsize::new(0));
        let globals = globals.to_vec();
        let (p, c) = (presses.clone(), clients.clone());
        let server = Server::start(bind(socket)?, move |stream| {
            c.fetch_add(1, Ordering::Relaxed);
            serve(stream, &globals, &p);
        })?;
        Ok(Self {
            socket: socket.to_path_buf(),
            presses,
            clients,
            server,
        })
    }

    /// The display socket.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Key presses its virtual keyboards received.
    pub fn presses(&self) -> usize {
        self.presses.load(Ordering::Relaxed)
    }

    /// Connections it accepted.
    pub fn clients(&self) -> usize {
        self.clients.load(Ordering::Relaxed)
    }

    /// Dies like a crashed compositor: every client loses its connection,
    /// and the socket file stays and refuses new ones.
    pub fn crash(&mut self) {
        self.server.crash();
    }
}

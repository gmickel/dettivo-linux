//! The audio service: one thread owns the PipeWire graph and every
//! capture (the PipeWire proxies are single-threaded), and the rest of the
//! daemon talks to it through `Send` handles over channels. When PipeWire
//! goes away the thread ends every capture with an error naming it and
//! connects again once PipeWire is back, so a PipeWire restart never
//! needs a daemon restart.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use crate::capture::Capture;
use crate::graph::{Graph, LOST, Node, State};
use crate::{CaptureError, EndReason, Event, Target};

/// How often the audio thread tries PipeWire again while it is gone.
const RECONNECT: Duration = Duration::from_secs(1);

#[derive(Debug)]
enum Cmd {
    Snapshot(Sender<State>),
    Open {
        target: Target,
        level_interval_ms: u64,
        reply: Sender<Result<(u64, Receiver<Event>), CaptureError>>,
    },
    Stop(u64),
    Shutdown,
}

/// A `Send` handle to the audio thread.
#[derive(Clone)]
pub struct AudioService {
    cmds: Sender<Cmd>,
    /// True while the audio thread's graph is connected to PipeWire.
    connected: Arc<AtomicBool>,
}

/// A running capture, usable from any thread.
#[derive(Debug)]
pub struct CaptureHandle {
    id: u64,
    events: Receiver<Event>,
    cmds: Sender<Cmd>,
}

/// What the audio thread owns.
struct Audio {
    graph: Option<Graph>,
    /// The graph's `alive` flag, shared with [`AudioService::connected`].
    connected: Arc<AtomicBool>,
    captures: HashMap<u64, Capture>,
    next_id: u64,
    last_try: Instant,
}

impl Audio {
    /// Drops a graph whose PipeWire went away, ending its captures with
    /// the error, and connects a new one, trying at most every
    /// [`RECONNECT`].
    fn reconnect(&mut self) {
        if self.connected.load(Ordering::SeqCst) {
            return;
        }
        if let Some(dead) = self.graph.take() {
            for (_, mut c) in self.captures.drain() {
                c.end(EndReason::Error(LOST.into()));
            }
            drop(dead);
        } else if self.last_try.elapsed() < RECONNECT {
            return;
        }
        self.last_try = Instant::now();
        self.graph = Graph::connect(self.connected.clone()).ok();
    }

    fn open(
        &mut self,
        target: Target,
        level_interval_ms: u64,
    ) -> Result<(u64, Receiver<Event>), CaptureError> {
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| CaptureError::NoPipeWire(LOST.into()))?;
        let mut c = Capture::open(graph, target, level_interval_ms)?;
        let id = self.next_id;
        self.next_id += 1;
        let events = c.take_events();
        self.captures.insert(id, c);
        Ok((id, events))
    }

    fn run(&mut self, rx: &Receiver<Cmd>) {
        loop {
            let cmd = rx.recv_timeout(RECONNECT);
            self.reconnect();
            match cmd {
                Ok(Cmd::Snapshot(reply)) => {
                    let _ =
                        reply.send(self.graph.as_ref().map(Graph::snapshot).unwrap_or_default());
                }
                Ok(Cmd::Open {
                    target,
                    level_interval_ms,
                    reply,
                }) => {
                    let _ = reply.send(self.open(target, level_interval_ms));
                }
                Ok(Cmd::Stop(id)) => {
                    if let Some(mut c) = self.captures.remove(&id) {
                        c.stop();
                    }
                }
                Ok(Cmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        for (_, mut c) in self.captures.drain() {
            c.stop();
        }
    }
}

impl AudioService {
    /// Connects to PipeWire on a new thread; `NoPipeWire` when it is absent.
    pub fn start() -> Result<Self, CaptureError> {
        let (cmds, rx) = mpsc::channel::<Cmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), CaptureError>>();
        let connected = Arc::new(AtomicBool::new(false));
        let alive = connected.clone();
        std::thread::Builder::new()
            .name("dettivo-audio".into())
            .spawn(move || {
                let graph = match Graph::connect(alive.clone()) {
                    Ok(g) => {
                        let _ = ready_tx.send(Ok(()));
                        g
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                Audio {
                    graph: Some(graph),
                    connected: alive,
                    captures: HashMap::new(),
                    next_id: 1,
                    last_try: Instant::now(),
                }
                .run(&rx);
            })
            .map_err(|e| CaptureError::NoPipeWire(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self { cmds, connected }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CaptureError::NoPipeWire(
                "the audio thread did not answer".into(),
            )),
        }
    }

    /// True while the service is connected to PipeWire: false from the
    /// moment PipeWire went away until the service connected again.
    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// The nodes and the default source right now; empty while PipeWire
    /// is gone.
    pub fn snapshot(&self) -> State {
        let (tx, rx) = mpsc::channel();
        if self.cmds.send(Cmd::Snapshot(tx)).is_err() {
            return State::default();
        }
        rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default()
    }

    /// The node the default source resolves to.
    pub fn default_node(&self) -> Option<Node> {
        let s = self.snapshot();
        let name = s.default_source?;
        s.nodes.into_values().find(|n| n.name == name)
    }

    /// The node with `name`.
    pub fn node_named(&self, name: &str) -> Option<Node> {
        self.snapshot().nodes.into_values().find(|n| n.name == name)
    }

    /// The node the default sink resolves to, whose monitor is a
    /// meeting's system track.
    pub fn default_sink_node(&self) -> Option<Node> {
        let s = self.snapshot();
        match s.default_sink {
            Some(name) => s.nodes.into_values().find(|n| n.name == name),
            None => s.nodes.into_values().find(|n| n.is_sink),
        }
    }

    /// Opens a capture on the audio thread; `NoPipeWire` while PipeWire
    /// is gone.
    pub fn open(
        &self,
        target: Target,
        level_interval_ms: u64,
    ) -> Result<CaptureHandle, CaptureError> {
        let (tx, rx) = mpsc::channel();
        self.cmds
            .send(Cmd::Open {
                target,
                level_interval_ms,
                reply: tx,
            })
            .map_err(|_| CaptureError::NoPipeWire("the audio thread is gone".into()))?;
        let (id, events) = rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| CaptureError::Stream("the audio thread did not answer".into()))??;
        Ok(CaptureHandle {
            id,
            events,
            cmds: self.cmds.clone(),
        })
    }

    /// Stops the thread; captures end.
    pub fn shutdown(&self) {
        let _ = self.cmds.send(Cmd::Shutdown);
    }
}

impl CaptureHandle {
    /// The event stream.
    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    /// Requests stop; `Ended` acknowledges the final accepted PCM in `events`.
    pub fn stop(&self) {
        let _ = self.cmds.send(Cmd::Stop(self.id));
    }

    /// `stop` for another thread, which never holds the event stream.
    pub fn stopper(&self) -> impl Fn() + Send + Sync + 'static {
        let (cmds, id) = (self.cmds.clone(), self.id);
        move || {
            let _ = cmds.send(Cmd::Stop(id));
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

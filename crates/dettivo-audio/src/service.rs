//! The audio service: one thread owns the PipeWire graph and every
//! capture (the PipeWire proxies are single-threaded), and the rest of the
//! daemon talks to it through `Send` handles over channels.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::capture::Capture;
use crate::graph::{Graph, Node, State};
use crate::{CaptureError, Event, Target};

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
}

/// A running capture, usable from any thread.
#[derive(Debug)]
pub struct CaptureHandle {
    id: u64,
    events: Receiver<Event>,
    cmds: Sender<Cmd>,
}

impl AudioService {
    /// Connects to PipeWire on a new thread; `NoPipeWire` when it is absent.
    pub fn start() -> Result<Self, CaptureError> {
        let (cmds, rx) = mpsc::channel::<Cmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), CaptureError>>();
        std::thread::Builder::new()
            .name("dettivo-audio".into())
            .spawn(move || {
                let graph = match Graph::connect() {
                    Ok(g) => {
                        let _ = ready_tx.send(Ok(()));
                        g
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let mut captures: HashMap<u64, Capture> = HashMap::new();
                let mut next_id = 1u64;
                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        Cmd::Snapshot(reply) => {
                            let _ = reply.send(graph.snapshot());
                        }
                        Cmd::Open {
                            target,
                            level_interval_ms,
                            reply,
                        } => {
                            let result =
                                Capture::open(&graph, target, level_interval_ms).map(|mut c| {
                                    let id = next_id;
                                    next_id += 1;
                                    let events = c.take_events();
                                    captures.insert(id, c);
                                    (id, events)
                                });
                            let _ = reply.send(result);
                        }
                        Cmd::Stop(id) => {
                            if let Some(mut c) = captures.remove(&id) {
                                c.stop();
                            }
                        }
                        Cmd::Shutdown => break,
                    }
                }
                for (_, mut c) in captures.drain() {
                    c.stop();
                }
                drop(graph);
            })
            .map_err(|e| CaptureError::NoPipeWire(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self { cmds }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CaptureError::NoPipeWire(
                "the audio thread did not answer".into(),
            )),
        }
    }

    /// The nodes and the default source right now.
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

    /// Opens a capture on the audio thread.
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
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

//! The PipeWire graph as the daemon sees it: a thread loop with a registry
//! listener collecting the audio nodes, and a metadata listener following
//! `default.audio.source` and `default.audio.sink`. One `Graph` lives for
//! the daemon's lifetime; captures are created on its loop, and a capture
//! can watch the defaults and the removal of the node it is pinned to.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use pipewire as pw;
use pw::context::ContextRc;
use pw::core::CoreRc;
use pw::metadata::{Metadata, MetadataListener};
use pw::registry::{Listener, RegistryRc};
use pw::thread_loop::ThreadLoopRc;
use pw::types::ObjectType;

use crate::CaptureError;

/// One audio node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// PipeWire object id.
    pub id: u32,
    /// `node.name`.
    pub name: String,
    /// `node.description`, or the name when absent.
    pub description: String,
    /// `Audio/Source` (a microphone-like input).
    pub is_source: bool,
    /// `Audio/Sink` (its monitor can be captured).
    pub is_sink: bool,
}

/// Shared state the loop callbacks write and the daemon reads.
#[derive(Debug, Default)]
pub struct State {
    /// Nodes by object id.
    pub nodes: BTreeMap<u32, Node>,
    /// The node name `default.audio.source` names, when any.
    pub default_source: Option<String>,
    /// The node name `default.audio.sink` names, when any.
    pub default_sink: Option<String>,
}

/// Who wants to hear about default-source changes.
type DefaultWatcher = Sender<Option<String>>;

/// Who wants to hear about default-source, default-sink and node removals.
#[derive(Default)]
struct Watchers {
    source: Vec<DefaultWatcher>,
    sink: Vec<DefaultWatcher>,
    removed: Vec<Sender<String>>,
}

/// The live graph.
pub struct Graph {
    pub(crate) loop_: ThreadLoopRc,
    pub(crate) core: CoreRc,
    _context: ContextRc,
    _registry: RegistryRc,
    _listener: Listener,
    _metadata: Rc<RefCell<Option<(Metadata, MetadataListener)>>>,
    state: Arc<Mutex<State>>,
    watchers: Arc<Mutex<Watchers>>,
}

impl Graph {
    /// Connects to PipeWire and starts following the graph. Returns
    /// `NoPipeWire` when the daemon socket is absent.
    pub fn connect() -> Result<Self, CaptureError> {
        pw::init();
        // SAFETY: the thread loop is the only PipeWire loop this process
        // runs; every PipeWire object below is created under its lock, and
        // `Drop for Graph` stops the loop first so no callback runs while
        // the objects held in the fields are dropped afterwards, which is
        // the ownership rule `ThreadLoopRc::new` asks the caller to keep.
        #[allow(unsafe_code)]
        let loop_ = unsafe { ThreadLoopRc::new(Some("dettivo-audio"), None) }
            .map_err(|e| CaptureError::NoPipeWire(e.to_string()))?;
        let guard = loop_.lock();
        let context =
            ContextRc::new(&loop_, None).map_err(|e| CaptureError::NoPipeWire(e.to_string()))?;
        let core = context
            .connect_rc(None)
            .map_err(|e| CaptureError::NoPipeWire(e.to_string()))?;
        let registry = core
            .get_registry_rc()
            .map_err(|e| CaptureError::NoPipeWire(e.to_string()))?;
        let state = Arc::new(Mutex::new(State::default()));
        let watchers: Arc<Mutex<Watchers>> = Arc::new(Mutex::new(Watchers::default()));
        let metadata: Rc<RefCell<Option<(Metadata, MetadataListener)>>> =
            Rc::new(RefCell::new(None));

        let state_add = state.clone();
        let state_remove = state.clone();
        let watchers_remove = watchers.clone();
        let registry_for_bind = registry.clone();
        let metadata_slot = metadata.clone();
        let state_meta = state.clone();
        let watchers_meta = watchers.clone();
        let listener = registry
            .add_listener_local()
            .global(move |global| {
                let Some(props) = global.props.as_ref() else {
                    return;
                };
                match global.type_ {
                    ObjectType::Node => {
                        let class = props.get("media.class").unwrap_or("");
                        let is_source = class == "Audio/Source";
                        let is_sink = class == "Audio/Sink";
                        if !(is_source || is_sink) {
                            return;
                        }
                        let name = props.get("node.name").unwrap_or("").to_string();
                        let description = props
                            .get("node.description")
                            .or_else(|| props.get("node.nick"))
                            .unwrap_or(&name)
                            .to_string();
                        state_add
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .nodes
                            .insert(
                                global.id,
                                Node {
                                    id: global.id,
                                    name,
                                    description,
                                    is_source,
                                    is_sink,
                                },
                            );
                    }
                    ObjectType::Metadata => {
                        if props.get("metadata.name") != Some("default") {
                            return;
                        }
                        let Ok(proxy) = registry_for_bind.bind::<Metadata, _>(global) else {
                            return;
                        };
                        let state_prop = state_meta.clone();
                        let watchers_prop = watchers_meta.clone();
                        let listener = proxy
                            .add_listener_local()
                            .property(move |_subject, key, _type, value| {
                                let is_source = key == Some("default.audio.source");
                                if !is_source && key != Some("default.audio.sink") {
                                    return 0;
                                }
                                let name = value.and_then(|v| {
                                    serde_json::from_str::<serde_json::Value>(v)
                                        .ok()
                                        .and_then(|j| j["name"].as_str().map(str::to_string))
                                });
                                let changed = {
                                    let mut s =
                                        state_prop.lock().unwrap_or_else(|p| p.into_inner());
                                    let slot = if is_source {
                                        &mut s.default_source
                                    } else {
                                        &mut s.default_sink
                                    };
                                    let changed = *slot != name;
                                    *slot = name.clone();
                                    changed
                                };
                                if changed {
                                    let mut ws =
                                        watchers_prop.lock().unwrap_or_else(|p| p.into_inner());
                                    let list = if is_source {
                                        &mut ws.source
                                    } else {
                                        &mut ws.sink
                                    };
                                    list.retain(|w| w.send(name.clone()).is_ok());
                                }
                                0
                            })
                            .register();
                        *metadata_slot.borrow_mut() = Some((proxy, listener));
                    }
                    _ => {}
                }
            })
            .global_remove(move |id| {
                let removed = state_remove
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .nodes
                    .remove(&id);
                if let Some(node) = removed {
                    let mut ws = watchers_remove.lock().unwrap_or_else(|p| p.into_inner());
                    ws.removed.retain(|w| w.send(node.name.clone()).is_ok());
                }
            })
            .register();
        drop(guard);
        loop_.start();
        // Let the initial registry enumeration land before answering.
        std::thread::sleep(std::time::Duration::from_millis(150));
        Ok(Self {
            loop_,
            core,
            _context: context,
            _registry: registry,
            _listener: listener,
            _metadata: metadata,
            state,
            watchers,
        })
    }

    /// A snapshot of the nodes and the default source.
    pub fn snapshot(&self) -> State {
        let s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        State {
            nodes: s.nodes.clone(),
            default_source: s.default_source.clone(),
            default_sink: s.default_sink.clone(),
        }
    }

    /// The node the default sink resolves to (its monitor is the system
    /// track), or any sink when the metadata names none.
    pub fn default_sink_node(&self) -> Option<Node> {
        let s = self.snapshot();
        match &s.default_sink {
            Some(name) => s.nodes.values().find(|n| &n.name == name).cloned(),
            None => s.nodes.values().find(|n| n.is_sink).cloned(),
        }
    }

    /// The node the default source resolves to.
    pub fn default_node(&self) -> Option<Node> {
        let s = self.snapshot();
        let name = s.default_source?;
        s.nodes.values().find(|n| n.name == name).cloned()
    }

    /// The node with `name`.
    pub fn node_named(&self, name: &str) -> Option<Node> {
        self.snapshot()
            .nodes
            .values()
            .find(|n| n.name == name)
            .cloned()
    }

    /// A channel that receives every change of the default source.
    pub fn watch_default(&self) -> Receiver<Option<String>> {
        let (tx, rx) = mpsc::channel();
        self.watchers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .source
            .push(tx);
        rx
    }

    /// A channel that receives every change of the default sink.
    pub fn watch_default_sink(&self) -> Receiver<Option<String>> {
        let (tx, rx) = mpsc::channel();
        self.watchers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sink
            .push(tx);
        rx
    }

    /// A channel that receives the name of every audio node that leaves
    /// the graph (a device unplugged, a null sink unloaded).
    pub fn watch_removed(&self) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();
        self.watchers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .removed
            .push(tx);
        rx
    }
}

impl Drop for Graph {
    fn drop(&mut self) {
        self.loop_.stop();
    }
}

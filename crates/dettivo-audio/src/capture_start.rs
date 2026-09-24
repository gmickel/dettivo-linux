//! Capture startup ordering, independent of the PipeWire stream implementation.

use std::sync::mpsc::Receiver;

use crate::graph::{Node, State};
use crate::{CaptureError, Target};

pub(super) enum Watch {
    Default(Receiver<Option<String>>),
    Pinned {
        name: String,
        removed: Receiver<String>,
    },
}

pub(super) struct Started<T> {
    pub node: Option<Node>,
    pub watch: Watch,
    pub stream: T,
}

pub(super) fn start<T>(
    target: &Target,
    snapshot: impl FnOnce() -> State,
    subscribe: impl FnOnce() -> Watch,
    connect: impl FnOnce(Option<&Node>) -> Result<T, CaptureError>,
) -> Result<Started<T>, CaptureError> {
    let state = snapshot();
    let named = |name: &str| state.nodes.values().find(|n| n.name == name).cloned();
    let node = match target {
        Target::Default => state.default_source.as_deref().and_then(named),
        Target::Node(name) => {
            Some(named(name).ok_or_else(|| CaptureError::UnknownDevice(name.clone()))?)
        }
        Target::SystemMonitor => Some(
            match state.default_sink.as_deref() {
                Some(name) => named(name),
                None => state.nodes.values().find(|n| n.is_sink).cloned(),
            }
            .ok_or_else(|| CaptureError::UnknownDevice("the default sink's monitor".into()))?,
        ),
    };
    if matches!(target, Target::Default) && node.is_none() && state.nodes.is_empty() {
        return Err(CaptureError::NoPipeWire(
            "no audio nodes in the graph".into(),
        ));
    }
    let watch = subscribe();
    let stream = connect(node.as_ref())?;
    Ok(Started {
        node,
        watch,
        stream,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::mpsc;

    #[test]
    fn changes_during_connection_are_observed_for_defaults_and_pinned_nodes() {
        for target in [
            Target::Default,
            Target::SystemMonitor,
            Target::Node("first".into()),
        ] {
            let subscribed = Cell::new(false);
            let (default_tx, default_rx) = mpsc::channel();
            let (removed_tx, removed_rx) = mpsc::channel();
            let started = start(
                &target,
                || State {
                    nodes: [(
                        1,
                        Node {
                            id: 1,
                            name: "first".into(),
                            description: "first".into(),
                            is_source: true,
                            is_sink: true,
                        },
                    )]
                    .into(),
                    default_source: Some("first".into()),
                    default_sink: Some("first".into()),
                },
                || {
                    subscribed.set(true);
                    match &target {
                        Target::Node(name) => Watch::Pinned {
                            name: name.clone(),
                            removed: removed_rx,
                        },
                        _ => Watch::Default(default_rx),
                    }
                },
                |node| {
                    assert_eq!(node.unwrap().name, "first");
                    if subscribed.get() {
                        match target {
                            Target::Node(_) => removed_tx.send("first".into()).unwrap(),
                            _ => default_tx.send(Some("second".into())).unwrap(),
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
            match started.watch {
                Watch::Default(rx) => assert_eq!(
                    rx.try_recv().expect("default changed during connect"),
                    Some("second".into())
                ),
                Watch::Pinned { removed, .. } => assert_eq!(
                    removed.try_recv().expect("node removed during connect"),
                    "first"
                ),
            }
        }
    }
}

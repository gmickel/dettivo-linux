//! `audio.devices`: the capturable PipeWire nodes, the default source and
//! the pinned device (ADR 0006).

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::audio::{Device, DeviceKind, DevicesResult};
use serde_json::Value;

use crate::daemon::Daemon;

/// `audio.devices`.
pub fn devices(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let pinned = daemon.config().config.audio.input_device.clone();
    let result = match daemon.audio() {
        None => DevicesResult {
            pipewire: false,
            default_source: None,
            default_sink: None,
            pinned,
            devices: Vec::new(),
        },
        Some(service) => {
            let snapshot = service.snapshot();
            let mut devices: Vec<Device> = snapshot
                .nodes
                .values()
                .map(|n| Device {
                    name: n.name.clone(),
                    description: n.description.clone(),
                    kind: if n.is_sink {
                        DeviceKind::SinkMonitor
                    } else {
                        DeviceKind::Source
                    },
                    is_default: snapshot.default_source.as_deref() == Some(n.name.as_str()),
                })
                .collect();
            devices.sort_by(|a, b| a.name.cmp(&b.name));
            DevicesResult {
                pipewire: true,
                default_source: snapshot.default_source,
                default_sink: snapshot.default_sink,
                pinned,
                devices,
            }
        }
    };
    serde_json::to_value(result).map_err(|e| {
        JsonRpcError::new(
            AppCode::InternalError,
            format!("cannot encode result: {e}"),
            ErrorDetails::empty(),
        )
    })
}

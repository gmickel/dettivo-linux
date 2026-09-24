//! `audio.*`: the Linux addition that reports the capture side of the
//! machine so the GUI, the CLI's doctor and the bar show the same source
//! of truth for devices (ADR 0006).

use serde::{Deserialize, Serialize};

/// One PipeWire audio node the daemon can capture from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    /// The PipeWire node name (`node.name`), the value `[audio] input_device` takes.
    pub name: String,
    /// The human description (`node.description`).
    pub description: String,
    /// `source` for a microphone-like node, `sink_monitor` for a sink whose monitor can be captured.
    pub kind: DeviceKind,
    /// True for the node the default source resolves to.
    pub is_default: bool,
}

/// What a device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// An input node (`Audio/Source`).
    Source,
    /// An output node whose monitor is captured (`Audio/Sink`).
    SinkMonitor,
}

/// `audio.devices` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevicesResult {
    /// True when PipeWire answered.
    pub pipewire: bool,
    /// The node name the default source resolves to, when any.
    pub default_source: Option<String>,
    /// The node name the default sink resolves to, when any; its monitor
    /// is a meeting's system track. Always present (null without a sink)
    /// so the contract replay sees one key set on every machine.
    #[serde(default)]
    pub default_sink: Option<String>,
    /// The pinned device from `[audio] input_device`, empty when following the default.
    pub pinned: String,
    /// Every capturable node.
    pub devices: Vec<Device>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A runner without a sink still answers the contract's key set: the
    /// replay compares shapes, so `default_sink` is null, never absent.
    #[test]
    fn default_sink_is_null_not_absent_without_a_sink() {
        let value = serde_json::to_value(DevicesResult {
            pipewire: true,
            default_source: None,
            default_sink: None,
            pinned: String::new(),
            devices: Vec::new(),
        })
        .unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "default_sink",
                "default_source",
                "devices",
                "pinned",
                "pipewire"
            ]
        );
        assert!(value["default_sink"].is_null());
    }
}

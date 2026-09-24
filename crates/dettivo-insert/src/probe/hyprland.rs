//! The Hyprland probe: `j/activewindow` over the instance's IPC socket,
//! which answers with the focused window's class, pid and title for
//! Wayland and XWayland windows alike.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use super::{FocusProbe, RawWindow};

/// The probe.
pub struct HyprlandProbe {
    /// `$XDG_RUNTIME_DIR/hypr/<signature>/.socket.sock`.
    pub socket: PathBuf,
}

impl HyprlandProbe {
    fn query(&self, command: &str) -> Result<String, String> {
        let mut stream = UnixStream::connect(&self.socket)
            .map_err(|e| format!("hyprland socket {}: {e}", self.socket.display()))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(command.as_bytes())
            .map_err(|e| format!("hyprland write: {e}"))?;
        let mut out = String::new();
        stream
            .read_to_string(&mut out)
            .map_err(|e| format!("hyprland read: {e}"))?;
        Ok(out)
    }
}

/// Parses the `activewindow` JSON: an empty object means nothing focused.
pub fn parse_active_window(json: &str) -> Result<Option<RawWindow>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json.trim()).map_err(|e| format!("hyprland activewindow: {e}"))?;
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    if object.is_empty() {
        return Ok(None);
    }
    let text = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let class = text("class");
    let app_id = if class.is_empty() {
        text("initialClass")
    } else {
        class
    };
    let pid = object
        .get("pid")
        .and_then(serde_json::Value::as_i64)
        .filter(|p| *p > 0)
        .map(|p| p as u32);
    Ok(Some(RawWindow {
        app_id,
        window: object
            .get("address")
            .and_then(serde_json::Value::as_str)
            .filter(|a| !a.is_empty())
            .map(str::to_string),
        pid,
        title: text("title"),
        xwayland: object
            .get("xwayland")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    }))
}

impl FocusProbe for HyprlandProbe {
    fn name(&self) -> &'static str {
        "hyprland"
    }

    fn focused(&self) -> Result<Option<RawWindow>, String> {
        parse_active_window(&self.query("j/activewindow")?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_window_json_yields_class_pid_and_title() {
        let raw = parse_active_window(
            r#"{"address":"0x1","class":"foot","initialClass":"foot","title":"~","pid":4242,"xwayland":false}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(raw.app_id, "foot");
        assert_eq!(raw.window.as_deref(), Some("0x1"));
        assert_eq!(raw.pid, Some(4242));
        assert_eq!(raw.title, "~");
        assert!(!raw.xwayland);
        assert_eq!(parse_active_window("{}").unwrap(), None);
        assert!(parse_active_window("nope").is_err());
        let x = parse_active_window(
            r#"{"class":"","initialClass":"dettivo-insert-target","pid":-1,"xwayland":true}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(x.app_id, "dettivo-insert-target");
        assert_eq!(x.pid, None);
        assert_eq!(x.window, None);
        assert!(x.xwayland);
    }
}

//! The cua-driver implementation: a private `cua-driver serve` daemon per
//! runner process and `cua-driver call <tool> <json>` for every action.
//! cua-driver 0.20 lists and drives X11 windows (XWayland or Xvfb) and
//! walks their AT-SPI trees; the Qt hosts run under `xcb` in QA so it can
//! see them (ADR 0011).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::cua_answer::{
    base64_decode, find_image, key_press, parse_tree_markdown, refusal, supplement_missing_facts,
};
use super::{App, Driver, DriverError, Element, Launch};

/// The version this repository is pinned to (`scripts/qa/install-cua-driver.sh`).
pub const PINNED_VERSION: &str = "0.20.0";

/// How long one `cua-driver call` may take; a screenshot or a tree walk
/// of a busy window is seconds, never minutes.
const CALL_BUDGET: Duration = Duration::from_secs(60);

/// The cua-driver backed driver.
pub struct CuaDriver {
    socket: PathBuf,
    profile: crate::profile::Profile,
    serve: Option<Child>,
    launched: Vec<Child>,
    geometry: super::atspi::AtspiDriver,
}

impl Default for CuaDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl CuaDriver {
    /// A driver with a private daemon socket inside a fresh temporary
    /// directory (short, so the path fits a Unix socket) that lives as
    /// long as the driver.
    pub fn new() -> Self {
        let profile = crate::profile::Profile::create("cua-driver", None)
            .expect("a private profile for cua-driver");
        Self {
            socket: profile.root.join("run/cua.sock"),
            profile,
            serve: None,
            launched: Vec::new(),
            geometry: super::atspi::AtspiDriver::new(),
        }
    }

    fn ensure_serve(&mut self) -> Result<(), DriverError> {
        if self.serve.is_some() {
            return Ok(());
        }
        let child = crate::profile::command("cua-driver", &self.profile.env())
            .args(["serve", "--no-overlay", "--socket"])
            .arg(&self.socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| DriverError::Missing(format!("cua-driver: {e}")))?;
        self.serve = Some(child);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.socket.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        if !self.socket.exists() {
            return Err(DriverError::Failed(
                "cua-driver serve did not create its socket".into(),
            ));
        }
        Ok(())
    }

    /// `cua-driver call <tool> <params>` against the private daemon,
    /// answered within `CALL_BUDGET` or killed.
    pub fn call(&mut self, tool: &str, mut params: Value) -> Result<Value, DriverError> {
        // The private Xvfb session explicitly permits focusing its own
        // window. A normal desktop keeps CUA's background default.
        if matches!(
            tool,
            "click" | "type_text" | "press_key" | "hotkey" | "scroll"
        ) && std::env::var("CUA_DRIVER_DELIVERY_MODE").as_deref() == Ok("foreground")
        {
            params["delivery_mode"] = json!("foreground");
        }
        self.ensure_serve()?;
        let mut command = crate::profile::command("cua-driver", &self.profile.env());
        command
            .args(["call", tool])
            .arg(params.to_string())
            .arg("--socket")
            .arg(&self.socket);
        let output = super::bounded::output_within(&mut command, CALL_BUDGET).map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut {
                DriverError::Failed(format!("{tool}: cua-driver {e}"))
            } else {
                DriverError::Missing(format!("cua-driver: {e}"))
            }
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() {
            return Err(DriverError::Failed(format!(
                "{tool}: {}{}",
                stdout.trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let answer: Value = serde_json::from_str(stdout.trim())
            .map_err(|e| DriverError::Failed(format!("{tool}: unreadable answer: {e}")))?;
        // `cua-driver call` exits 0 on a refused or failed tool call and
        // says so in the answer; a refusal that went unread would pass as
        // an action that happened.
        if let Some(why) = refusal(&answer) {
            return Err(DriverError::Failed(format!("{tool}: {why}")));
        }
        Ok(answer)
    }

    fn window_of(&mut self, pid: u32, timeout: Duration) -> Result<Option<u64>, DriverError> {
        let deadline = Instant::now() + timeout;
        loop {
            let windows = self.call("list_windows", json!({}))?;
            if let Some(w) = windows["windows"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|w| w["pid"].as_u64() == Some(u64::from(pid)))
            {
                return Ok(w["window_id"].as_u64());
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Driver for CuaDriver {
    fn name(&self) -> &'static str {
        "cua"
    }

    fn preflight(&mut self) -> Result<(), DriverError> {
        let version = crate::profile::command("cua-driver", &self.profile.env())
            .arg("--version")
            .output()
            .map_err(|e| DriverError::Missing(format!("cua-driver not on PATH: {e}")))?;
        let text = String::from_utf8_lossy(&version.stdout);
        if !text.contains(PINNED_VERSION) {
            eprintln!(
                "cua: warning: installed {} but the repository pins {PINNED_VERSION}",
                text.trim()
            );
        }
        let perms = self.call("check_permissions", json!({}))?;
        for (key, what) in [
            ("atspi", "the accessibility bus"),
            ("x11", "an X11 display"),
        ] {
            if perms[key] != Value::Bool(true) {
                return Err(DriverError::Missing(format!(
                    "{what} (cua-driver check_permissions.{key})"
                )));
            }
        }
        Ok(())
    }

    fn launch(&mut self, launch: &Launch, timeout: Duration) -> Result<App, DriverError> {
        let mut cmd = crate::profile::command(&launch.program, &launch.env);
        cmd.args(&launch.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = cmd.spawn().map_err(|e| {
            DriverError::Failed(format!("launch {}: {e}", launch.program.display()))
        })?;
        let pid = child.id();
        self.launched.push(child);
        let window = self.window_of(pid, timeout)?;
        if window.is_none() {
            return Err(DriverError::NotFound(format!(
                "an X11 window for pid {pid} within {timeout:?} (is the app running under QT_QPA_PLATFORM=xcb?)"
            )));
        }
        Ok(App { pid, window })
    }

    fn snapshot(&mut self, app: &App) -> Result<Vec<Element>, DriverError> {
        let mut params = json!({"pid": app.pid, "include_screenshot": false});
        if let Some(w) = app.window {
            params["window_id"] = json!(w);
        }
        let state = self.call("get_window_state", params)?;
        let elements = state["elements"]
            .as_array()
            .or_else(|| state["structuredContent"]["elements"].as_array())
            .cloned()
            .unwrap_or_default();
        // The token names the element within this snapshot; it is what a
        // click hands back (cua-driver 0.17 and later refuse a bare index).
        let mut out: Vec<Element> = elements
            .iter()
            .map(|e| Element {
                index: e["element_index"].as_u64().unwrap_or(0) as u32,
                id: e["element_token"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("cua:{}", e["element_index"].as_u64().unwrap_or(0))),
                role: e["role"].as_str().unwrap_or("").to_string(),
                name: e["label"].as_str().unwrap_or("").to_string(),
                value: e["value"].as_str().map(str::to_string),
                bounds: e["frame"].as_object().map(|f| {
                    (
                        f["x"].as_i64().unwrap_or(0) as i32,
                        f["y"].as_i64().unwrap_or(0) as i32,
                        f["w"].as_i64().unwrap_or(0) as i32,
                        f["h"].as_i64().unwrap_or(0) as i32,
                    )
                }),
                focusable: e["focusable"].as_bool().unwrap_or(true),
                focused: e["focused"].as_bool().unwrap_or(false),
                enabled: e["enabled"].as_bool().unwrap_or(true),
                parent: None,
                native_id: None,
            })
            .collect();
        // cua-driver indexes only actionable elements; static labels live in
        // the markdown rendering of the same tree, so read those too.
        if let Some(markdown) = state["tree_markdown"].as_str() {
            for element in parse_tree_markdown(markdown) {
                if !out.iter().any(|e| e.index == element.index) {
                    out.push(element);
                }
            }
        }
        if let Ok(native) = self.geometry.snapshot(app) {
            supplement_missing_facts(&mut out, &native);
        }
        Ok(out)
    }

    fn click(&mut self, app: &App, element: &Element) -> Result<(), DriverError> {
        // A token is good for one snapshot and every `get_window_state`
        // (a screenshot, a label wait) supersedes it, so the click takes
        // a fresh snapshot and finds the same element in it: the same
        // role and name, at the same index when that still holds.
        let fresh = self.snapshot(app)?;
        let same = |e: &Element| e.role == element.role && e.name == element.name;
        let target = fresh
            .iter()
            .find(|e| e.index == element.index && same(e))
            .or_else(|| fresh.iter().find(|e| same(e)))
            .ok_or_else(|| {
                DriverError::NotFound(format!(
                    "{} {:?} in the tree at click time",
                    element.role, element.name
                ))
            })?;
        if !target.id.starts_with("cua:") && !target.id.starts_with("label:") {
            self.call("click", json!({"pid": app.pid, "element_token": target.id}))?;
            return Ok(());
        }
        // A `label:` or `cua:` id came from the markdown rendering, which
        // lists what cua-driver does not index as actionable.
        Err(DriverError::Failed(format!(
            "{} {:?} is not an actionable element for cua-driver",
            target.role, target.name
        )))
    }

    fn type_text(&mut self, app: &App, text: &str) -> Result<(), DriverError> {
        let mut params = json!({"pid": app.pid, "text": text});
        if let Some(window) = app.window {
            params["window_id"] = json!(window);
        }
        self.call("type_text", params)?;
        Ok(())
    }

    fn press_key(&mut self, app: &App, key: &str) -> Result<(), DriverError> {
        self.call("press_key", key_press(app.pid, key))?;
        Ok(())
    }

    fn read_value(&mut self, app: &App, element: &Element) -> Result<String, DriverError> {
        let fresh = self.snapshot(app)?;
        let same = |e: &Element| e.role == element.role && e.name == element.name;
        let current = fresh
            .iter()
            .find(|e| e.index == element.index && same(e))
            .or_else(|| fresh.iter().find(|e| same(e)))
            .ok_or_else(|| DriverError::NotFound(format!("element {}", element.index)))?;
        if let Some(value) = &current.value {
            return Ok(value.clone());
        }
        // cua-driver 0.20 reads a field's text into its name only when the
        // field has no name of its own, so a named field (`Try it field`)
        // shows its name and hides its text. The in-repo AT-SPI reader
        // answers with the Text interface for the same element.
        let mut atspi = super::atspi::AtspiDriver::new();
        if let Ok(tree) = atspi.snapshot(app) {
            if let Some(twin) = tree.into_iter().find(|e| same(e)) {
                if let Some(value) = twin.value {
                    return Ok(value);
                }
            }
        }
        Ok(current.name.clone())
    }

    fn screenshot(&mut self, app: &App, path: &Path) -> Result<(), DriverError> {
        let mut params = json!({"pid": app.pid, "include_screenshot": true});
        if let Some(w) = app.window {
            params["window_id"] = json!(w);
        }
        let state = self.call("get_window_state", params)?;
        let image = find_image(&state)
            .ok_or_else(|| DriverError::Failed("get_window_state returned no screenshot".into()))?;
        let bytes = base64_decode(image)
            .ok_or_else(|| DriverError::Failed("screenshot is not base64".into()))?;
        std::fs::write(path, bytes)
            .map_err(|e| DriverError::Failed(format!("write {}: {e}", path.display())))
    }

    fn close(&mut self, app: &App) -> Result<(), DriverError> {
        let _ = Command::new("kill")
            .args(["-TERM", &app.pid.to_string()])
            .output();
        Ok(())
    }
}

impl Drop for CuaDriver {
    fn drop(&mut self) {
        for child in &mut self.launched {
            // A term first, so a Qt app unlinks its sockets; the kill is
            // for what ignores it.
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .output();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while std::time::Instant::now() < deadline && matches!(child.try_wait(), Ok(None)) {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(mut serve) = self.serve.take() {
            let _ = crate::profile::command("cua-driver", &self.profile.env())
                .args(["stop", "--socket"])
                .arg(&self.socket)
                .output();
            let _ = serve.kill();
            let _ = serve.wait();
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}

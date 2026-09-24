//! The in-repo fallback driver (ADR 0011): elements through the
//! accessibility bus with the `atspi` crate, input through the X11 XTEST
//! extension and screenshots through `GetImage`, both with `x11rb`. It
//! depends on nothing outside the repository's build, so a cua-driver
//! regression never blocks a drive.

mod input;
mod tree;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
use atspi::proxy::component::ComponentProxy;
use atspi::proxy::text::TextProxy;
use atspi::proxy::value::ValueProxy;
use atspi::{AccessibilityConnection, CoordType, Interface, State};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt as _, ImageFormat};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use super::{App, Driver, DriverError, Element, Launch, write_png};

/// The AT-SPI plus XTEST driver.
pub struct AtspiDriver {
    runtime: tokio::runtime::Runtime,
    bus: Option<AccessibilityConnection>,
    x11: Option<(RustConnection, usize)>,
    launched: Vec<Child>,
}

impl Default for AtspiDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AtspiDriver {
    /// A driver that connects lazily on first use.
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        Self {
            runtime,
            bus: None,
            x11: None,
            launched: Vec::new(),
        }
    }

    fn bus(&mut self) -> Result<&AccessibilityConnection, DriverError> {
        if self.bus.is_none() {
            let conn = self
                .runtime
                .block_on(AccessibilityConnection::new())
                .map_err(|e| {
                    DriverError::Missing(format!("the accessibility bus (org.a11y.Bus): {e}"))
                })?;
            self.bus = Some(conn);
        }
        Ok(self.bus.as_ref().unwrap())
    }

    pub(super) fn x11(&mut self) -> Result<&(RustConnection, usize), DriverError> {
        if self.x11.is_none() {
            let pair = RustConnection::connect(None)
                .map_err(|e| DriverError::Missing(format!("an X11 display (DISPLAY): {e}")))?;
            self.x11 = Some(pair);
        }
        Ok(self.x11.as_ref().unwrap())
    }

    /// Walks the tree of every application whose pid matches, flattened.
    fn walk(&mut self, pid: u32) -> Result<Vec<Element>, DriverError> {
        let bus = self.bus()?;
        let zbus = bus.connection().clone();
        self.runtime.block_on(async move {
            let root = AccessibleProxy::builder(&zbus)
                .destination("org.a11y.atspi.Registry")
                .map_err(|e| DriverError::Failed(e.to_string()))?
                .path("/org/a11y/atspi/accessible/root")
                .map_err(|e| DriverError::Failed(e.to_string()))?
                .build()
                .await
                .map_err(|e| DriverError::Failed(format!("registry root: {e}")))?;
            let apps = root
                .get_children()
                .await
                .map_err(|e| DriverError::Failed(format!("registry children: {e}")))?;
            let mut out = Vec::new();
            let mut index = 0u32;
            for app_ref in apps {
                let app = match app_ref.as_accessible_proxy(&zbus).await {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                let app_pid = match app.get_application().await {
                    Ok(r) => match r.as_accessible_proxy(&zbus).await {
                        Ok(a) => process_id(&zbus, &a).await,
                        Err(_) => None,
                    },
                    Err(_) => None,
                }
                .or(process_id(&zbus, &app).await);
                if app_pid != Some(pid) {
                    continue;
                }
                let mut stack: Vec<(AccessibleProxy<'_>, u32, Option<u32>)> = vec![(app, 0, None)];
                while let Some((node, depth, parent)) = stack.pop() {
                    if depth > 64 || index > 5000 {
                        return Err(DriverError::Failed(format!(
                            "accessibility tree truncated at {index} elements, depth {depth}"
                        )));
                    }
                    let name = node.name().await.unwrap_or_default();
                    let role = node.get_role_name().await.unwrap_or_default();
                    let interfaces = node.get_interfaces().await.unwrap_or_default();
                    let states = node.get_state().await.unwrap_or_default();
                    let value = read_value_of(&zbus, &node, &interfaces).await;
                    // Extents the component does not answer stay `None`:
                    // the walk records unknown geometry rather than a hidden
                    // control, and the accessibility check counts it.
                    let bounds = if interfaces.contains(Interface::Component) {
                        match ComponentProxy::builder(&zbus)
                            .destination(node.inner().destination().clone())
                            .and_then(|b| b.path(node.inner().path().clone()))
                        {
                            Ok(builder) => match builder.build().await {
                                Ok(component) => {
                                    component.get_extents(CoordType::Screen).await.ok()
                                }
                                Err(_) => None,
                            },
                            Err(_) => None,
                        }
                    } else {
                        None
                    };
                    out.push(Element {
                        index,
                        id: node.inner().path().to_string(),
                        role,
                        name,
                        value,
                        bounds,
                        focusable: states.contains(State::Focusable),
                        focused: states.contains(State::Focused),
                        enabled: states.contains(State::Enabled),
                        parent,
                        native_id: None,
                    });
                    let own = index;
                    index += 1;
                    let children = node.get_children().await.map_err(|e| {
                        DriverError::Failed(format!("children of {}: {e}", node.inner().path()))
                    })?;
                    for child in children.into_iter().rev() {
                        let c = child.into_accessible_proxy(&zbus).await.map_err(|e| {
                            DriverError::Failed(format!("child of {}: {e}", node.inner().path()))
                        })?;
                        stack.push((c, depth + 1, Some(own)));
                    }
                }
            }
            Ok(out)
        })
    }

    fn fake_key(&mut self, keycode: u8, press: bool) -> Result<(), DriverError> {
        let (conn, screen) = self.x11()?;
        let root = conn.setup().roots[*screen].root;
        conn.xtest_fake_input(
            if press {
                xproto::KEY_PRESS_EVENT
            } else {
                xproto::KEY_RELEASE_EVENT
            },
            keycode,
            x11rb::CURRENT_TIME,
            root,
            0,
            0,
            0,
        )
        .map_err(|e| DriverError::Failed(format!("xtest: {e}")))?;
        conn.flush()
            .map_err(|e| DriverError::Failed(format!("xtest flush: {e}")))?;
        Ok(())
    }
}

/// The process behind an accessible: AT-SPI does not carry it, so the bus
/// is asked for the credentials of the connection that owns the node.
async fn process_id(zbus: &zbus::Connection, node: &AccessibleProxy<'_>) -> Option<u32> {
    let name = node.inner().destination().to_string();
    let proxy = zbus::fdo::DBusProxy::new(zbus).await.ok()?;
    let unique: zbus::names::BusName = name.as_str().try_into().ok()?;
    proxy.get_connection_unix_process_id(unique).await.ok()
}

async fn read_value_of(
    zbus: &zbus::Connection,
    node: &AccessibleProxy<'_>,
    interfaces: &atspi::InterfaceSet,
) -> Option<String> {
    let dest = node.inner().destination().clone();
    let path = node.inner().path().clone();
    if interfaces.contains(Interface::Text) {
        let text = TextProxy::builder(zbus)
            .destination(dest.clone())
            .ok()?
            .path(path.clone())
            .ok()?
            .build()
            .await
            .ok()?;
        // The whole text in one call: Qt's bridge answers GetText but not
        // the CharacterCount property, so the count is never read first.
        return text.get_text(0, -1).await.ok();
    }
    if interfaces.contains(Interface::Value) {
        let value = ValueProxy::builder(zbus)
            .destination(dest)
            .ok()?
            .path(path)
            .ok()?
            .build()
            .await
            .ok()?;
        return value.current_value().await.ok().map(|v| v.to_string());
    }
    None
}

impl Driver for AtspiDriver {
    fn name(&self) -> &'static str {
        "atspi"
    }

    fn preflight(&mut self) -> Result<(), DriverError> {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none()
            && !Path::new(&format!(
                "{}/bus",
                std::env::var("XDG_RUNTIME_DIR").unwrap_or_default()
            ))
            .exists()
        {
            return Err(DriverError::Missing(
                "a session bus (DBUS_SESSION_BUS_ADDRESS or $XDG_RUNTIME_DIR/bus)".into(),
            ));
        }
        self.x11()?;
        self.bus()?;
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
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(window) = self.window_for_pid(pid)? {
                return Ok(App {
                    pid,
                    window: Some(window),
                });
            }
            if Instant::now() >= deadline {
                return Err(DriverError::NotFound(format!(
                    "an X11 window for pid {pid} within {timeout:?} (is the app running under QT_QPA_PLATFORM=xcb?)"
                )));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn snapshot(&mut self, app: &App) -> Result<Vec<Element>, DriverError> {
        tree::snapshot(|| self.walk(app.pid))
    }

    fn click(&mut self, app: &App, element: &Element) -> Result<(), DriverError> {
        let (x, y, w, h) = element.bounds.ok_or_else(|| {
            DriverError::Failed(format!("element {} has no bounds", element.index))
        })?;
        let focus = self.focus_target(app, element)?;
        let (conn, screen) = self.x11()?;
        let root = conn.setup().roots[*screen].root;
        let cx = i16::try_from(x + w / 2).unwrap_or(0);
        let cy = i16::try_from(y + h / 2).unwrap_or(0);
        conn.xtest_fake_input(
            xproto::MOTION_NOTIFY_EVENT,
            0,
            x11rb::CURRENT_TIME,
            root,
            cx,
            cy,
            0,
        )
        .map_err(|e| DriverError::Failed(format!("xtest: {e}")))?;
        conn.xtest_fake_input(
            xproto::BUTTON_PRESS_EVENT,
            1,
            x11rb::CURRENT_TIME,
            root,
            0,
            0,
            0,
        )
        .map_err(|e| DriverError::Failed(format!("xtest: {e}")))?;
        conn.xtest_fake_input(
            xproto::BUTTON_RELEASE_EVENT,
            1,
            x11rb::CURRENT_TIME,
            root,
            0,
            0,
            0,
        )
        .map_err(|e| DriverError::Failed(format!("xtest: {e}")))?;
        conn.flush()
            .map_err(|e| DriverError::Failed(format!("xtest flush: {e}")))?;
        if let Some(destination) = focus {
            self.await_focus(app, element, &destination)?;
        }
        Ok(())
    }

    fn type_text(&mut self, _app: &App, text: &str) -> Result<(), DriverError> {
        let map = self.keymap()?;
        for ch in text.chars() {
            let Some((keycode, shift)) = map.lookup(ch) else {
                return Err(DriverError::Failed(format!("no keycode for {ch:?}")));
            };
            if shift {
                self.fake_key(map.shift, true)?;
            }
            self.fake_key(keycode, true)?;
            self.fake_key(keycode, false)?;
            if shift {
                self.fake_key(map.shift, false)?;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    fn press_key(&mut self, _app: &App, key: &str) -> Result<(), DriverError> {
        let map = self.keymap()?;
        let (modifiers, name) = key.rsplit_once('+').unwrap_or(("", key));
        let mut held = Vec::new();
        for modifier in modifiers.split('+').filter(|m| !m.is_empty()) {
            let sym = match modifier {
                "Shift" => 0xFFE1,
                "Ctrl" | "Control" => 0xFFE3,
                "Alt" => 0xFFE9,
                "Super" | "Meta" => 0xFFEB,
                other => {
                    return Err(DriverError::Failed(format!("unknown modifier {other:?}")));
                }
            };
            let (code, _) = map.lookup_sym(sym).ok_or_else(|| {
                DriverError::Failed(format!("no keycode for the {modifier} modifier"))
            })?;
            held.push(code);
        }
        let sym = match name {
            "Tab" => 0xFF09,
            "Escape" => 0xFF1B,
            "Return" | "Enter" => 0xFF0D,
            "Space" => 0x20,
            single if single.chars().count() == 1 => {
                // A letter names the key, not its shifted glyph: `Super+F`
                // is the f key under Super.
                let ch = single.chars().next().unwrap_or(' ');
                u32::from(ch.to_ascii_lowercase())
            }
            other => return Err(DriverError::Failed(format!("unknown key {other:?}"))),
        };
        let (code, shifted) = map
            .lookup_sym(sym)
            .ok_or_else(|| DriverError::Failed(format!("no keycode for {name:?}")))?;
        if shifted && !held.contains(&map.shift) {
            held.push(map.shift);
        }
        for &m in &held {
            self.fake_key(m, true)?;
        }
        self.fake_key(code, true)?;
        self.fake_key(code, false)?;
        for &m in held.iter().rev() {
            self.fake_key(m, false)?;
        }
        std::thread::sleep(Duration::from_millis(60));
        Ok(())
    }

    /// Reads text directly from the owned application object. Other
    /// values retain snapshot lookup and recreated role/name fallback.
    fn read_value(&mut self, app: &App, element: &Element) -> Result<String, DriverError> {
        if element.role == "text" {
            return self.read_field(app, element);
        }
        let fresh = self.snapshot(app)?;
        fresh
            .iter()
            .find(|e| e.id == element.id)
            .or_else(|| {
                fresh
                    .iter()
                    .find(|e| e.role == element.role && e.name == element.name)
            })
            .map(|e| e.value.clone().unwrap_or_else(|| e.name.clone()))
            .ok_or_else(|| {
                DriverError::NotFound(format!("element {} ({})", element.index, element.id))
            })
    }

    fn screenshot(&mut self, app: &App, path: &Path) -> Result<(), DriverError> {
        let window = app
            .window
            .ok_or_else(|| DriverError::Failed("app has no window".into()))?;
        let window =
            u32::try_from(window).map_err(|_| DriverError::Failed("bad window id".into()))?;
        let (x, y, w, h) = self.visible_rect(window)?;
        let (conn, _) = self.x11()?;
        let image = conn
            .get_image(ImageFormat::Z_PIXMAP, window, x, y, w, h, !0)
            .map_err(|e| DriverError::Failed(e.to_string()))?
            .reply()
            .map_err(|e| DriverError::Failed(format!("get_image: {e}")))?;
        let (w, h) = (u32::from(w), u32::from(h));
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        if image.depth >= 24 {
            for px in image.data.chunks(4) {
                if px.len() == 4 {
                    rgb.extend_from_slice(&[px[2], px[1], px[0]]);
                }
            }
        } else {
            return Err(DriverError::Failed(format!(
                "unsupported depth {}",
                image.depth
            )));
        }
        write_png(path, w, h, &rgb)
            .map_err(|e| DriverError::Failed(format!("write {}: {e}", path.display())))
    }

    fn close(&mut self, app: &App) -> Result<(), DriverError> {
        let _ = Command::new("kill")
            .args(["-TERM", &app.pid.to_string()])
            .output();
        Ok(())
    }
}

impl Drop for AtspiDriver {
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
    }
}

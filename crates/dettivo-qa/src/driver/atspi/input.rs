//! Targeted field observations keep input independent of list delegate churn.
use atspi::{State, StateSet};

use super::*;

const FOCUS_TIMEOUT: Duration = Duration::from_secs(2);

fn failed(error: impl std::fmt::Display) -> DriverError {
    DriverError::Failed(error.to_string())
}

async fn target<'a>(
    bus: &'a zbus::Connection,
    pid: u32,
    path: &str,
) -> Result<AccessibleProxy<'a>, DriverError> {
    let root = AccessibleProxy::builder(bus)
        .destination("org.a11y.atspi.Registry")
        .map_err(failed)?
        .path("/org/a11y/atspi/accessible/root")
        .map_err(failed)?
        .build()
        .await
        .map_err(failed)?;
    for app_ref in root.get_children().await.map_err(failed)? {
        let Ok(app) = app_ref.as_accessible_proxy(bus).await else {
            continue;
        };
        if process_id(bus, &app).await == Some(pid) {
            return AccessibleProxy::builder(bus)
                .destination(app.inner().destination().to_owned())
                .map_err(failed)?
                .path(path.to_string())
                .map_err(failed)?
                .build()
                .await
                .map_err(failed);
        }
    }
    Err(DriverError::NotFound(format!(
        "accessible application for pid {pid}"
    )))
}

fn editable_field(state: StateSet) -> bool {
    [State::Editable, State::Enabled, State::Focusable]
        .into_iter()
        .all(|flag| state.contains(flag))
}

fn active_window(conn: &RustConnection, root: u32) -> Result<Option<u64>, DriverError> {
    let atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .map_err(failed)?
        .reply()
        .map_err(failed)?
        .atom;
    let reply = conn
        .get_property(false, root, atom, xproto::AtomEnum::WINDOW, 0, 1)
        .map_err(failed)?
        .reply()
        .map_err(failed)?;
    Ok(reply.value32().and_then(|mut v| v.next()).map(u64::from))
}

impl AtspiDriver {
    pub(super) fn focus_target(
        &mut self,
        app: &App,
        element: &Element,
    ) -> Result<Option<String>, DriverError> {
        if element.role != "text" {
            return Ok(None);
        }
        let bus = self.bus()?.connection().clone();
        self.runtime.block_on(async {
            tokio::time::timeout(FOCUS_TIMEOUT, async {
                let node = target(&bus, app.pid, &element.id).await?;
                let state = node.get_state().await.map_err(failed)?;
                Ok(editable_field(state).then(|| node.inner().destination().to_string()))
            })
            .await
            .map_err(|_| failed("text field state timed out"))?
        })
    }

    pub(super) fn await_focus(
        &mut self,
        app: &App,
        element: &Element,
        destination: &str,
    ) -> Result<(), DriverError> {
        let expected = app
            .window
            .ok_or_else(|| failed("app has no window for field focus"))?;
        let bus = self.bus()?.connection().clone();
        self.x11()?;
        let (conn, screen) = self.x11.as_ref().unwrap();
        let root = conn.setup().roots[*screen].root;
        self.runtime.block_on(async {
            tokio::time::timeout(FOCUS_TIMEOUT, async {
                let node = AccessibleProxy::builder(&bus)
                    .destination(destination).map_err(failed)?
                    .path(element.id.as_str()).map_err(failed)?
                    .build().await.map_err(failed)?;
                loop {
                    let state = node.get_state().await.map_err(failed)?;
                    if editable_field(state) && state.contains(State::Focused)
                        && active_window(conn, root)? == Some(expected) {
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            }).await.map_err(|_| failed(format!(
                "field {} focus in owned window {expected} was not acknowledged within {FOCUS_TIMEOUT:?}",
                element.id
            )))?
        })
    }

    pub(super) fn read_field(
        &mut self,
        app: &App,
        element: &Element,
    ) -> Result<String, DriverError> {
        let bus = self.bus()?.connection().clone();
        self.runtime.block_on(async {
            tokio::time::timeout(FOCUS_TIMEOUT, async {
                let node = target(&bus, app.pid, &element.id).await?;
                let interfaces = node.get_interfaces().await.map_err(failed)?;
                read_value_of(&bus, &node, &interfaces)
                    .await
                    .ok_or_else(|| {
                        failed(format!("field {} exposes no readable value", element.id))
                    })
            })
            .await
            .map_err(|_| failed("text field value timed out"))?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_readonly_and_nonfocusable_controls_do_not_wait_for_input_focus() {
        let flags = [State::Editable, State::Enabled, State::Focusable];
        assert!(editable_field(flags.into_iter().collect()));
        for omitted in flags {
            assert!(!editable_field(
                flags.into_iter().filter(|s| *s != omitted).collect()
            ));
        }
    }
}

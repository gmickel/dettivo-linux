//! Text insertion into the focused application (ADR 0007): the backend
//! chain (in-process Wayland virtual keyboard, libei through the
//! RemoteDesktop portal, `ydotool`, `xdotool`, clipboard plus paste
//! keystroke, clipboard only), the focus probes that verify the target
//! before anything is typed, the guards, and `undo`.

pub mod backend;
pub mod chain;
mod delivery;
pub mod guards;
pub mod keymap;
pub mod probe;
pub mod service;
pub mod session;
pub mod settings;

pub use service::{Inserter, Request, Service};
pub use session::{Mock, Session};
pub use settings::Settings;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[dettivo_proto::CRATE_NAME];

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-insert");
    }

    #[test]
    fn upstream_edges_resolve() {
        assert_eq!(super::UPSTREAM, &["dettivo-proto"]);
    }
}

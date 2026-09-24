//! Domain core of the daemon: paths, configuration and runtime state.
//!
//! Everything here is plain synchronous Rust with no I/O framework, so the
//! daemon, its tests and any future in-process tool share one
//! implementation of "where do files live", "what does the file say" and
//! "write it back without losing a comment" (ADR 0009).

pub mod atomic;
pub mod config;
pub mod paths;
pub mod qa;
pub mod state;
pub mod token;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[dettivo_proto::CRATE_NAME];

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-core");
    }

    #[test]
    fn upstream_edges_resolve() {
        assert_eq!(super::UPSTREAM, &["dettivo-proto"]);
    }
}

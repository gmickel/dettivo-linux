//! The text layers of a dictation (FR-M1 to FR-M8, ADR 0023): `raw` (the
//! replacement table, spoken punctuation and whitespace discipline), the
//! deterministic Polish pass in [`polish`] (transforms, presets, styles and
//! post-processors ported from macOS with their golden cases), the frozen
//! effective [`policy`] with its hash, the Enhanced rewrite in [`enhanced`]
//! (prompt profiles, output guards, thinking-block stripping, retries and
//! the fallback to Polish), the [`provider`] layer behind it (`ollama`,
//! `openai_compatible`, the `local` engine, the QA mock and the trust gate
//! for remote endpoints) and the meeting [`analysis`] that runs the
//! finalised transcript through the same providers (ADR 0036).

pub mod analysis;
pub mod enhanced;
pub mod pipeline;
pub mod policy;
pub mod polish;
pub mod provider;
pub mod raw;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[dettivo_proto::CRATE_NAME, dettivo_core::CRATE_NAME];

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-language");
    }

    #[test]
    fn upstream_edges_resolve() {
        assert_eq!(super::UPSTREAM, &["dettivo-proto", "dettivo-core"]);
    }
}

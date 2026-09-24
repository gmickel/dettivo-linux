//! The QA rig (ADR 0011): drivers, isolated profiles, evidence, the
//! contract replay, the virtual audio check, the visual regression job
//! (ADR 0021), the beauty pass over its renders (ADR 0042), the scenario pack, named packs that compose scenarios into
//! one report (ADR 0017), the engine pipelines (the Parakeet alignment
//! spike, ADR 0018; the import merge, ADR 0022; diarization, ADR 0035),
//! the polish evaluation harness (ADR 0032), the NFR targets (`nfr`),
//! the benchmark suite that measures them (`bench`, ADR 0029) and the
//! evidence map over every spec's requirements (`evidence_map`). The
//! `dettivo-qa` binary is a thin command line over these modules.

pub mod a11y_tree;
pub mod alignment;
pub mod audio;
pub mod beauty;
pub mod bench;
mod child;
pub mod contract;
pub mod doctor;
pub mod driver;
pub mod evidence;
pub mod evidence_map;
pub mod keys;
pub mod lint;
pub mod negative_text;
pub mod nfr;
pub mod pacing;
pub mod pack;
pub mod pipeline;
pub mod pipeline_diarize;
pub mod pipeline_import;
pub mod polish_eval;
pub mod profile;
pub mod profile_models;
pub mod replay;
mod replay_diagnostics;
pub mod replay_flags;
pub mod replay_shape;
pub mod runner;
pub mod scenarios;
pub mod socket;
pub mod stateful;
pub mod stats;
pub mod visual;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

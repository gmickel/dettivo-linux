//! Per-namespace method params/result types (`docs/api/dettivo-ipc-v1.md`
//! section 8). Reserved namespaces and methods live in
//! `crate::reserved`.

pub mod audio;
pub mod automation;
pub mod config;
pub mod dictation;
pub mod hotkeys;
pub mod insert;
pub mod llm;
pub mod meetings;
pub mod meetings_notes;
pub mod meetings_segments;
pub mod polish;
pub mod speakers;
pub mod speech;
pub mod system;
pub mod transcripts;
pub mod transcripts_facts;
pub mod transfer;

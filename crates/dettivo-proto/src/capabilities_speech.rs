//! The speech and meeting blocks of `system.capabilities`: the adopted
//! Windows provider-selection methods with the meeting capability per
//! provider (ADR 0018), and the Linux-added `meetings.*` methods with the
//! checkpoint schema. `capabilities` re-exports them, so
//! `capabilities::SpeechCaps` stays the path.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Linux addition: names the Windows-adopted speech provider/selection
/// methods this daemon implements, registered as a delta in
/// `docs/api/linux-deltas.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechCaps {
    /// Method names this daemon implements under `speech.*`.
    pub methods: Vec<String>,
    /// Linux addition: per provider, whether its timestamps qualify it for
    /// meetings (the alignment spike's outcome, ADR 0018); keyed by
    /// provider id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, SpeechProviderCaps>,
}

/// What one speech provider may be used for beyond dictation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechProviderCaps {
    /// The provider's timestamps are precise enough for the meeting merger.
    pub meeting_capable: bool,
}

impl SpeechCaps {
    /// The three Windows-adopted method names Linux implements
    /// (`speech.providers.list`, `speech.selection.get`,
    /// `speech.selection.set`).
    pub fn adopted_windows_methods() -> Self {
        Self {
            methods: vec![
                "speech.providers.list".to_string(),
                "speech.selection.get".to_string(),
                "speech.selection.set".to_string(),
            ],
            providers: BTreeMap::new(),
        }
    }

    /// The same methods plus the meeting capability per provider.
    pub fn with_providers(mut self, providers: &[(&str, bool)]) -> Self {
        self.providers = providers
            .iter()
            .map(|(id, meeting_capable)| {
                (
                    (*id).to_string(),
                    SpeechProviderCaps {
                        meeting_capable: *meeting_capable,
                    },
                )
            })
            .collect();
        self
    }
}

/// Linux addition: the meeting methods beyond the contract's and the
/// checkpoint schema the daemon writes, registered in
/// `docs/api/linux-deltas.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingCaps {
    /// The Linux-added `meetings.*` method names.
    pub methods: Vec<String>,
    /// The `live-checkpoint.json` schema version the daemon writes.
    pub checkpoint_schema: u32,
}

impl MeetingCaps {
    /// What a Linux daemon declares.
    pub fn linux() -> Self {
        Self {
            methods: [
                "meetings.recover",
                "meetings.discard",
                "meetings.disclosure.get",
                "meetings.disclosure.acknowledge",
                "meetings.diarize",
                "meetings.speakers.list",
                "meetings.speakers.rename",
                "meetings.speakers.suggest",
                "meetings.notes.get",
                "meetings.notes.set",
                "meetings.analyze",
                "meetings.analysis.get",
                "meetings.rename",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            checkpoint_schema: 1,
        }
    }
}

//! Retry an imported meeting from its retained mono WAV, keeping its row identity.
use super::*;

impl Jobs {
    pub(crate) fn retry_meeting(
        self: &Arc<Self>,
        daemon: &Daemon,
        mut row: MeetingRow,
        audio: PathBuf,
        engine: Arc<dyn SttEngine>,
    ) -> Result<(), JsonRpcError> {
        let loaded = daemon.config();
        let mut item = DictationItem::new(SourceKind::AudioImport);
        item.id = row.id.clone();
        item.status = ItemStatus::Transcribing;
        item.title = row.title.clone();
        item.duration_ms = row.duration_ms;
        item.language = row.language.clone();
        item.stt_provider = row.stt_provider.clone();
        item.stt_model = row.stt_model.clone();
        let n = self.rerun_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let job_id = format!("job_meeting_retry_{n}");
        let previous = row.clone();
        row.status = dettivo_storage::meetings::MeetingStatus::Transcribing;
        row.is_partial = false;
        row.error_code = None;
        row.error_message = None;
        daemon
            .history()
            .with_store(|s| s.update_meeting(&row))
            .map_err(store_error)?;
        daemon.history().mark_busy(&row.id);
        let d = &loaded.config.dictation;
        let work = Work {
            job_id,
            item,
            audio,
            discard_audio: false,
            decode: None,
            engine,
            language: if row.language.is_empty() {
                "auto".into()
            } else {
                row.language.clone()
            },
            prompt: (!d.vocabulary.is_empty()).then(|| d.vocabulary.join(", ")),
            replacements: d
                .replacements
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            spoken_punctuation: d.spoken_punctuation,
            protect_tokens: d.protect_tokens,
            settings: settings_of(&loaded),
            cancel: Arc::new(AtomicBool::new(false)),
            meeting: Some(row),
            meeting_retention: Some((
                loaded.config.meetings.keep_audio,
                dettivo_storage::retention::ArtifactPolicy::parse(
                    &loaded.config.meetings.artifacts,
                )
                .unwrap_or(dettivo_storage::retention::ArtifactPolicy::Keep),
            )),
            archive: Some(daemon.meeting_archive().clone()),
        };
        if let Err(e) = self.spawn(daemon, work) {
            daemon.history().clear_busy(&previous.id);
            daemon
                .history()
                .with_store(|s| s.update_meeting(&previous))
                .map_err(store_error)?;
            return Err(e);
        }
        Ok(())
    }
}

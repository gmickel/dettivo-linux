//! `speech.vocabulary.suggest` (ADR 0104): one term spoken by both Piper
//! voices, alone and in a sentence, transcribed by the selected speech
//! engine twice, padded and guarded as a dictation take is: with the
//! vocabulary a take carries (the term in it) and with none. In a clip the
//! vocabulary did not fix, what the engine heard with none is proposed as
//! a spelling of the term; nothing is written.

use std::time::Duration;

use dettivo_core::config::vocabulary_schema::{self, VocabularyEntry, term_key};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::speech_vocabulary::{Hearing, SuggestParams, SuggestResult};
use dettivo_session::bias;
use dettivo_speech::suggest::{self, Piper};
use dettivo_speech::{RecognizeRequest, prompt_terms, vocabulary_prompt};
use serde_json::Value;

use super::{json, params};
use crate::daemon::Daemon;

/// How long one clip may take to transcribe, a cold model load included.
const RECOGNIZE_TIMEOUT: Duration = Duration::from_secs(120);

/// `speech.vocabulary.suggest`.
pub fn suggest(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SuggestParams = params(params_value)?;
    // The term as given, so it matches the vocabulary's entry exactly as
    // the file spells it.
    let term = p.term;
    if suggest::words(&term).is_empty() {
        return Err(error(
            AppCode::InvalidParams,
            "the term has no words to say",
        ));
    }
    let loaded = daemon.config();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let piper = Piper::find(
        &loaded.models_dir(&daemon.paths),
        std::env::var_os("PATH").as_deref(),
        home.as_deref(),
    )
    .map_err(|why| error(AppCode::NotFound, &why))?;
    let engines = daemon.engines();
    engines.ensure_selected_verified()?;
    let engine = engines.engine();
    let provider = engine.provider().to_string();
    let dictation = &loaded.config.dictation;
    // The vocabulary a take would carry, with the term in it.
    let key = term_key(&term);
    let mut entries = dictation.vocabulary.clone();
    if !entries.iter().any(|e| term_key(e.term()) == key) {
        entries.push(VocabularyEntry::Word(term.clone()));
    }
    let listed = entries
        .iter()
        .find(|e| term_key(e.term()) == key)
        .map(|e| e.aliases().to_vec())
        .unwrap_or_default();
    let terms = vocabulary_schema::terms(&entries);
    let echo: Vec<String> = if engine.capabilities().supports_custom_vocabulary {
        prompt_terms(&terms).to_vec()
    } else {
        Vec::new()
    };
    let primed = if provider == "parakeet" {
        bias::vocabulary_bias(&entries, dictation.vocabulary_boost)
    } else {
        bias::checked_terms(&echo, &entries)
    };
    let hear = |pcm: &[i16], on: bool| -> Result<String, JsonRpcError> {
        let request = |pcm: Vec<i16>| RecognizeRequest {
            pcm,
            language: dictation.language.clone(),
            prompt: if on { vocabulary_prompt(&terms) } else { None },
            timestamps: true,
            vocabulary: if on { primed.clone() } else { None },
        };
        let recognize = |pcm: Vec<i16>| {
            engine
                .recognize(request(pcm), RECOGNIZE_TIMEOUT)
                .map_err(|e| error(AppCode::InternalError, &format!("{provider}: {e}")))
        };
        let padded =
            dettivo_transcribe::pad_for_engine(pcm.to_vec(), provider == "parakeet", 16_000);
        let was_padded = padded.len() != pcm.len();
        let mut result = recognize(padded)?;
        // As in a take: a padded clip that comes back empty runs once more as spoken.
        if was_padded && result.text.trim().is_empty() {
            result = recognize(pcm.to_vec())?;
        }
        let echo: &[String] = if on { &echo } else { &[] };
        Ok(dettivo_transcribe::guards::clean(&result, echo)
            .trim()
            .to_string())
    };
    let mut hearings = Vec::new();
    for (voice, model) in &piper.voices {
        for (index, said) in suggest::sayings(&term).into_iter().enumerate() {
            let pcm = piper
                .speak(model, &said)
                .map_err(|why| error(AppCode::InternalError, &why))?;
            let heard = hear(&pcm, true)?;
            let unprimed = hear(&pcm, false)?;
            let (right, spelling) = suggest::hear(&term, &heard, &unprimed, index == 1);
            hearings.push(Hearing {
                voice: voice.clone(),
                said,
                heard,
                unprimed,
                right,
                spelling,
            });
        }
    }
    let heard: Vec<_> = hearings
        .iter()
        .map(|h| (h.right, h.spelling.clone()))
        .collect();
    let (recognised, sounds_like) = suggest::propose(&heard, &listed);
    json(SuggestResult {
        term,
        provider,
        model: loaded.config.speech.model.clone(),
        recognised,
        sounds_like,
        hearings,
    })
}

fn error(code: AppCode, message: &str) -> JsonRpcError {
    JsonRpcError::new(code, message.to_string(), ErrorDetails::empty())
}

//! `dettivo-qa bench windowed` (ADR 0102): whether decoding a dictation in
//! rounds while the key is held costs accuracy. The FLEURS utterances
//! `scripts/qa/fetch-fleurs.sh` prepared are joined in manifest order into
//! takes of 30, 60 and 120 s per language. Each take is decoded whole, as
//! dictation does without rounds, and in rounds through
//! `dettivo_transcribe::rounds` with the windows dictation runs: the rounds
//! a take of that length would run while held, then the release's tail,
//! stitched. The take's reference is its utterances' references joined.
//! Every take's word errors land in `windowed-takes.jsonl` as they are
//! measured; the summary gives each model and language the corpus rates and
//! a paired bootstrap interval of their difference over takes.

use std::io::Write;
use std::path::{Path, PathBuf};

use dettivo_transcribe::Word;
use dettivo_transcribe::rounds::{self, Release, Rounds, WINDOWS, Windows};
use serde::Serialize;
use serde_json::{Value, json};

use super::Options;
use super::engine_client::{EngineClient, read_pcm};
use crate::pack::wer::{wer, words};

/// The take lengths, in seconds.
pub const TAKES_S: [u64; 3] = [30, 60, 120];
/// The models measured: provider, id and file.
pub const MODELS: [(&str, &str, &str); 3] = [
    ("parakeet", "parakeet-v3", "tdt-0.6b-v3-q8_0.gguf"),
    ("parakeet", "parakeet-ultra", "parakeet-ultra.q8_0.gguf"),
    ("whisper", "large-v3-turbo", "ggml-large-v3-turbo.bin"),
];
/// The release windows scored: dictation's, and the 8 s of the published scheme.
pub const TAILS_MS: [u64; 2] = [WINDOWS.tail_ms, 8_000];
/// One release window's text: the window, the text, how the release
/// decoded, the audio it decoded (ms) and the words committed.
type Windowed = (u64, String, &'static str, u64, usize);
/// Bootstrap resamples.
pub const RESAMPLES: usize = 10_000;

/// One take's word errors, decoded whole and in rounds.
#[derive(Debug, Clone, Serialize)]
pub struct TakeRow {
    /// The model id.
    pub model: String,
    /// The language code.
    pub language: String,
    /// The take length asked for, in seconds.
    pub take_s: u64,
    /// The release window, in ms.
    pub tail_ms: u64,
    /// The take's place among the takes of that length.
    pub index: usize,
    /// The take's length.
    pub audio_ms: u64,
    /// Reference words.
    pub reference_words: usize,
    /// Word errors of the whole decode.
    pub whole_errors: usize,
    /// Word errors of the decode in rounds.
    pub windowed_errors: usize,
    /// `tail` or `whole` (no usable round).
    pub release: &'static str,
    /// Audio the release decoded, in ms.
    pub release_ms: u64,
    /// Words the rounds committed.
    pub committed: usize,
}

/// One model and language, over every take.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// The model id.
    pub model: String,
    /// The language code.
    pub language: String,
    /// The release window, in ms.
    pub tail_ms: u64,
    /// The take length, or `None` over every length.
    pub take_s: Option<u64>,
    /// Takes scored.
    pub takes: usize,
    /// Reference words.
    pub reference_words: usize,
    /// Corpus word error rate of the whole decodes.
    pub whole_wer: f64,
    /// Corpus word error rate of the decodes in rounds.
    pub windowed_wer: f64,
    /// The second minus the first.
    pub difference: f64,
    /// The 95% paired bootstrap interval of the difference.
    pub ci95_low: f64,
    /// Its upper bound, which R2 holds at +0.004.
    pub ci95_high: f64,
}

/// Utterances joined into takes of at least `take_s`: their samples and
/// their joined reference.
fn takes(items: &[(Vec<i16>, String)], take_s: u64) -> Vec<(Vec<i16>, String)> {
    let mut out = Vec::new();
    let (mut pcm, mut text) = (Vec::new(), Vec::new());
    for (clip, reference) in items {
        pcm.extend_from_slice(clip);
        text.push(reference.as_str());
        if pcm.len() as u64 >= take_s * 16_000 {
            out.push((std::mem::take(&mut pcm), text.join(" ")));
            text.clear();
        }
    }
    out
}

fn ms(samples: usize) -> u64 {
    samples as u64 / 16
}

fn span(pcm: &[i16], from_ms: u64, until_ms: u64) -> &[i16] {
    let a = (from_ms as usize * 16).min(pcm.len());
    let b = (until_ms as usize * 16).min(pcm.len());
    &pcm[a..b]
}

/// One decode with word timestamps, padded as dictation pads a take.
fn decode(
    engine: &mut EngineClient,
    pcm: &[i16],
    language: &str,
    parakeet: bool,
) -> Result<(String, Vec<Word>), String> {
    let padded = dettivo_transcribe::pad_for_engine(pcm.to_vec(), parakeet, 16_000);
    let params = json!({ "language": language, "prompt": null, "timestamps": true });
    let mut result = engine.call("recognize", params.clone(), Some(&padded))?;
    // A padded clip that comes back empty runs once more as captured, as
    // dictation does (ADR 0081).
    if padded.len() != pcm.len()
        && result["text"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        result = engine.call("recognize", params, Some(pcm))?;
    }
    let words: Vec<Word> = result["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["words"].as_array().cloned().unwrap_or_default())
        .filter_map(|w: Value| serde_json::from_value(w).ok())
        .collect();
    Ok((
        result["text"].as_str().unwrap_or_default().to_string(),
        words,
    ))
}

/// The take decoded in rounds: the rounds a held take of its length runs
/// at the cadence, then the release. Rounds before the first that commits
/// change nothing but the last round, so only the committing rounds and
/// the last one run.
fn windowed(
    engine: &mut EngineClient,
    w: &Windows,
    pcm: &[i16],
    language: &str,
    parakeet: bool,
) -> Result<Vec<Windowed>, String> {
    let end = ms(pcm.len());
    let mut r = Rounds::default();
    let last = (end - 1) / w.cadence_ms * w.cadence_ms;
    let mut at = w.cadence_ms;
    while at <= last {
        if at >= w.commit_after_ms || at == last {
            let (from, until) = r.window(w, at);
            let (_, words) = decode(engine, span(pcm, from, until), language, parakeet)?;
            r.absorb(w, until, rounds::shifted(words, from));
        }
        at += w.cadence_ms;
    }
    // The rounds are the same for every release window.
    let mut out = Vec::new();
    for tail_ms in TAILS_MS {
        let w = Windows { tail_ms, ..*w };
        out.push(match r.release(&w, end, None) {
            Release::Tail { from_ms, cut_ms } => {
                let (_, words) = decode(engine, span(pcm, from_ms, end), language, parakeet)?;
                let all = r.stitch(cut_ms, rounds::shifted(words, from_ms));
                (
                    tail_ms,
                    rounds::text(&all),
                    "tail",
                    end - from_ms,
                    r.committed(),
                )
            }
            Release::Reuse(all) => (tail_ms, rounds::text(&all), "reuse", 0, r.committed()),
            Release::Whole => {
                let (text, _) = decode(engine, pcm, language, parakeet)?;
                (tail_ms, text, "whole", end, 0)
            }
        });
    }
    Ok(out)
}

fn errors(reference: &[String], text: &str) -> usize {
    (wer(reference, &words(text)) * reference.len() as f64).round() as usize
}

/// A 95% paired bootstrap interval of the windowed rate minus the whole
/// rate, resampling takes.
pub fn bootstrap(rows: &[&TakeRow], resamples: usize) -> (f64, f64) {
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut diffs: Vec<f64> = (0..resamples)
        .map(|_| {
            let (mut refs, mut d) = (0i64, 0i64);
            for _ in 0..rows.len() {
                let row = rows[(next() % rows.len() as u64) as usize];
                refs += row.reference_words as i64;
                d += row.windowed_errors as i64 - row.whole_errors as i64;
            }
            d as f64 / refs.max(1) as f64
        })
        .collect();
    diffs.sort_by(f64::total_cmp);
    let at = |q: f64| diffs[((resamples as f64 * q) as usize).min(resamples - 1)];
    (at(0.025), at(0.975))
}

fn summary(rows: &[&TakeRow], take_s: Option<u64>) -> Summary {
    let refs: usize = rows.iter().map(|r| r.reference_words).sum();
    let whole: usize = rows.iter().map(|r| r.whole_errors).sum();
    let win: usize = rows.iter().map(|r| r.windowed_errors).sum();
    let (lo, hi) = bootstrap(rows, RESAMPLES);
    let rate = |e: usize| e as f64 / refs.max(1) as f64;
    Summary {
        model: rows[0].model.clone(),
        language: rows[0].language.clone(),
        tail_ms: rows[0].tail_ms,
        take_s,
        takes: rows.len(),
        reference_words: refs,
        whole_wer: rate(whole),
        windowed_wer: rate(win),
        difference: rate(win) - rate(whole),
        ci95_low: lo,
        ci95_high: hi,
    }
}

fn manifest(dir: &Path) -> Result<Vec<(Vec<i16>, String)>, String> {
    let path = dir.join("manifest.tsv");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(wav, reference)| Ok((read_pcm(&dir.join(wav))?, reference.to_string())))
        .collect()
}

fn run(opts: &Options) -> Result<Vec<Summary>, String> {
    let languages = opts.languages_dir.as_deref().ok_or("needs --languages")?;
    let models = opts.models_dir.as_deref().ok_or("no model directory")?;
    let engines = opts.engines_dir.as_deref().ok_or("needs --engines")?;
    let out_dir = opts.out.clone().unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_dir.join("windowed-takes.jsonl"))
        .map_err(|e| e.to_string())?;
    let mut summaries = Vec::new();
    for (provider, id, file) in MODELS {
        if opts.step.as_deref().is_some_and(|s| s != id) {
            continue;
        }
        let model = models.join(provider).join(id).join(file);
        if !model.is_file() {
            eprintln!("windowed: {id} not on disk");
            continue;
        }
        let binary = engines.join(format!("dettivo-engine-{provider}"));
        let mut engine = EngineClient::spawn(&binary, opts.cpu, &out_dir.join("engine.log"))?;
        let (loaded, _) = engine.load(&model, None)?;
        engine.warm_up()?;
        let parakeet = provider == "parakeet";
        for language in ["en", "de"] {
            if !languages.join(language).join("manifest.tsv").is_file() {
                continue;
            }
            let mut items = manifest(&languages.join(language))?;
            // `--quick` scores the first 100 utterances of each language.
            if opts.quick {
                items.truncate(100);
            }
            let mut rows = Vec::new();
            for take_s in TAKES_S {
                for (index, (pcm, reference)) in takes(&items, take_s).into_iter().enumerate() {
                    let reference = words(&reference);
                    let (whole, _) = decode(&mut engine, &pcm, language, parakeet)?;
                    let whole_errors = errors(&reference, &whole);
                    for (tail_ms, text, release, release_ms, committed) in
                        windowed(&mut engine, &WINDOWS, &pcm, language, parakeet)?
                    {
                        let row = TakeRow {
                            model: id.into(),
                            language: language.into(),
                            take_s,
                            tail_ms,
                            index,
                            audio_ms: ms(pcm.len()),
                            reference_words: reference.len(),
                            whole_errors,
                            windowed_errors: errors(&reference, &text),
                            release,
                            release_ms,
                            committed,
                        };
                        let mut line = serde_json::to_value(&row).unwrap_or(Value::Null);
                        line["backend"] = loaded["backend"].clone();
                        let _ = writeln!(log, "{line}");
                        rows.push(row);
                    }
                }
            }
            for tail_ms in TAILS_MS {
                let of_tail: Vec<&TakeRow> = rows.iter().filter(|r| r.tail_ms == tail_ms).collect();
                for take_s in TAKES_S {
                    let of: Vec<&TakeRow> = of_tail
                        .iter()
                        .copied()
                        .filter(|r| r.take_s == take_s)
                        .collect();
                    if !of.is_empty() {
                        summaries.push(summary(&of, Some(take_s)));
                    }
                }
                if !of_tail.is_empty() {
                    let s = summary(&of_tail, None);
                    eprintln!(
                        "windowed: {}",
                        serde_json::to_string(&s).unwrap_or_default()
                    );
                    summaries.push(s);
                }
            }
        }
    }
    let path = out_dir.join("windowed-summary.json");
    let text = serde_json::to_string_pretty(&summaries).unwrap_or_default();
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(summaries)
}

/// The suite's entry point; prints the summary as JSON.
pub fn command(opts: &Options, _json: bool) -> u8 {
    match run(opts) {
        Ok(summaries) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&summaries).unwrap_or_default()
            );
            0
        }
        Err(e) => {
            eprintln!("bench windowed: {e}");
            2
        }
    }
}

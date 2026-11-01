//! `language_wer` (ADR 0085): every speech model of the catalogue that is
//! on disk transcribes the FLEURS utterances `scripts/qa/fetch-fleurs.sh`
//! prepared, once per language it takes, over the engine protocol after
//! the warm-up. The row per model and language carries the corpus word
//! error rate and the realtime factor; `rank` orders the models per
//! language, treating rates within `TIE_POINTS` as a tie the faster model
//! wins (equally fast models fall back to the rate). The ranks and rates
//! are what the catalogue's `language_wer` tables carry.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::engine_client::{EngineClient, ms, read_pcm};
use super::host::CATALOGUE;
use super::{Bench, Status, Step};
use crate::pack::wer::{wer, words};

/// Rates closer than this many points are a tie.
pub const TIE_POINTS: f64 = 1.5;

/// Models at least this share of the fastest model's speed are equally
/// fast: two builds of one architecture differ by timing noise and by
/// which languages they ran (up to 16% across the recorded runs), while
/// Whisper large-v3-turbo runs at under half of Parakeet's speed.
pub const SAME_SPEED: f64 = 0.75;

/// One model in one language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageRow {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The language code.
    pub language: String,
    /// The backend the engine loaded on.
    pub backend: String,
    /// Utterances transcribed.
    pub utterances: usize,
    /// Corpus word error rate in percent, one decimal.
    pub wer: f64,
    /// Audio over wall time.
    pub realtime_factor: f64,
    /// 1 for the model this language recommends.
    pub rank: usize,
}

/// One speech model on disk, with the languages it is scored in.
struct Candidate {
    provider: String,
    id: String,
    path: PathBuf,
    english_only: bool,
}

fn candidates(bench: &Bench) -> Result<Vec<Candidate>, String> {
    let path = bench.opts.repo_root.join(CATALOGUE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let catalogue: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
    let models_dir = bench
        .opts
        .models_dir
        .as_deref()
        .ok_or("no model directory")?;
    Ok(catalogue["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["kind"].as_str() == Some("stt"))
        .filter_map(|m| {
            let provider = m["provider"].as_str()?.to_string();
            let id = m["id"].as_str()?.to_string();
            let path = models_dir
                .join(&provider)
                .join(&id)
                .join(m["file_name"].as_str()?);
            path.is_file().then(|| Candidate {
                provider,
                id,
                path,
                english_only: m.get("english_only").and_then(toml::Value::as_bool) == Some(true),
            })
        })
        .collect())
}

/// The prepared languages: `<dir>/<code>/manifest.tsv` lines of
/// `<wav>\t<reference>`.
/// A language code and its utterances with their references.
type Language = (String, Vec<(PathBuf, String)>);

fn languages(dir: &Path) -> Result<Vec<Language>, String> {
    let mut out = Vec::new();
    let mut codes: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .filter(|e| e.path().join("manifest.tsv").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    codes.sort();
    for code in codes {
        let manifest = dir.join(&code).join("manifest.tsv");
        let text = std::fs::read_to_string(&manifest)
            .map_err(|e| format!("{}: {e}", manifest.display()))?;
        let items = text
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(wav, reference)| (dir.join(&code).join(wav), reference.to_string()))
            .collect();
        out.push((code, items));
    }
    Ok(out)
}

/// Transcribes one language on a loaded engine; `None` when the model
/// refuses the language.
fn score(
    engine: &mut EngineClient,
    code: &str,
    items: &[(PathBuf, String)],
) -> Result<Option<(f64, f64)>, String> {
    let (mut errors, mut reference_words, mut audio_ms, mut wall_ms) = (0.0, 0usize, 0u64, 0u64);
    for (wav, reference) in items {
        let pcm = read_pcm(wav)?;
        let (result, took) = match engine.recognize(&pcm, code) {
            Ok(r) => r,
            Err(e) if e.contains("bad_request") => return Ok(None),
            Err(e) => return Err(e),
        };
        let reference = words(reference);
        let hypothesis = words(result["text"].as_str().unwrap_or_default());
        errors += wer(&reference, &hypothesis) * reference.len() as f64;
        reference_words += reference.len();
        audio_ms += pcm.len() as u64 / 16;
        wall_ms += ms(took);
    }
    let rate = (errors / reference_words.max(1) as f64 * 1000.0).round() / 10.0;
    Ok(Some((rate, audio_ms as f64 / wall_ms.max(1) as f64)))
}

/// A model's speed: the median realtime factor over its languages, so one
/// language's timing noise never decides a tie.
fn speed(rows: &[LanguageRow], model: &str) -> f64 {
    let mut factors: Vec<f64> = rows
        .iter()
        .filter(|r| r.model == model)
        .map(|r| r.realtime_factor)
        .collect();
    factors.sort_by(f64::total_cmp);
    factors.get(factors.len() / 2).copied().unwrap_or(0.0)
}

/// Ranks the rows of each language: the lowest rate leads, and among the
/// models within `TIE_POINTS` of it the fastest takes the place. Models
/// within `SAME_SPEED` of the fastest count as equally fast, and the lower
/// rate among them wins.
pub fn rank(rows: &mut [LanguageRow]) {
    let speeds: Vec<f64> = rows.iter().map(|r| speed(rows, &r.model)).collect();
    let mut codes: Vec<String> = rows.iter().map(|r| r.language.clone()).collect();
    codes.sort();
    codes.dedup();
    for code in codes {
        let mut left: Vec<usize> = (0..rows.len())
            .filter(|i| rows[*i].language == code)
            .collect();
        let mut next = 1;
        while !left.is_empty() {
            let best = left
                .iter()
                .map(|i| rows[*i].wer)
                .fold(f64::INFINITY, f64::min);
            let tied: Vec<usize> = left
                .iter()
                .copied()
                .filter(|i| rows[*i].wer < best + TIE_POINTS)
                .collect();
            let fastest = tied.iter().map(|i| speeds[*i]).fold(0.0, f64::max);
            let pick = tied
                .into_iter()
                .filter(|i| speeds[*i] >= fastest * SAME_SPEED)
                .min_by(|a, b| rows[*a].wer.total_cmp(&rows[*b].wer))
                .expect("the fastest tied row is its own peer");
            rows[pick].rank = next;
            next += 1;
            left.retain(|i| *i != pick);
        }
    }
}

fn measure(bench: &Bench, dir: &Path) -> Result<Vec<LanguageRow>, String> {
    let sets = languages(dir)?;
    let mut rows = Vec::new();
    for model in candidates(bench)? {
        let binary = format!("dettivo-engine-{}", model.provider);
        let Some(engine_path) = bench.engine(&binary) else {
            continue;
        };
        let log = bench.run_dir.join("language_wer-engine.log");
        let mut engine = EngineClient::spawn(engine_path, bench.opts.cpu, &log)?;
        let (loaded, _) = engine.load(&model.path, None)?;
        engine.warm_up()?;
        for (code, items) in &sets {
            if model.english_only && code != "en" {
                continue;
            }
            let Some((wer, realtime_factor)) = score(&mut engine, code, items)? else {
                continue;
            };
            rows.push(LanguageRow {
                provider: model.provider.clone(),
                model: model.id.clone(),
                language: code.clone(),
                backend: loaded["backend"].as_str().unwrap_or("unknown").to_string(),
                utterances: items.len(),
                wer,
                realtime_factor: (realtime_factor * 10.0).round() / 10.0,
                rank: 0,
            });
        }
    }
    rank(&mut rows);
    rows.sort_by(|a, b| a.language.cmp(&b.language).then(a.rank.cmp(&b.rank)));
    Ok(rows)
}

/// The step; skipped without `--languages`.
pub fn run(bench: &Bench) -> Step {
    let Some(dir) = bench.opts.languages_dir.as_deref() else {
        return Step::without(
            "language_wer",
            Status::Skipped,
            "no --languages directory; prepare one with scripts/qa/fetch-fleurs.sh <dir>".into(),
        );
    };
    match measure(bench, dir) {
        Ok(rows) => Step {
            name: "language_wer".into(),
            status: if rows.is_empty() {
                Status::Skipped
            } else {
                Status::Measured
            },
            reason: rows
                .is_empty()
                .then(|| "no speech model or language to score".into()),
            comparison: None,
            results: rows
                .iter()
                .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
                .collect(),
            duration_ms: 0,
        },
        Err(e) => Step::without("language_wer", Status::Failed, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(model: &str, language: &str, wer: f64, realtime_factor: f64) -> LanguageRow {
        LanguageRow {
            provider: "p".into(),
            model: model.into(),
            language: language.into(),
            backend: "vulkan".into(),
            utterances: 100,
            wer,
            realtime_factor,
            rank: 0,
        }
    }

    /// A manifest with nothing to score (empty, no tab-separated rows, or
    /// blank references) is refused rather than scored as a perfect 0.0.
    #[test]
    fn a_language_with_no_reference_words_is_refused() {
        for manifest in ["", "no tab on this line\n", "a.wav\t \n"] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir(dir.path().join("de")).unwrap();
            std::fs::write(dir.path().join("de/manifest.tsv"), manifest).unwrap();
            let error = languages(dir.path()).expect_err(manifest);
            assert!(error.contains("no reference words"), "{error}");
        }
    }

    #[test]
    fn a_tie_goes_to_the_faster_model_and_a_real_gap_to_the_better_one() {
        let mut rows = vec![
            row("slow-best", "de", 4.1, 40.0),
            row("fast-close", "de", 5.0, 120.0),
            row("fast-far", "de", 9.0, 200.0),
            row("slow-best", "en", 3.0, 40.0),
            row("fast-far", "en", 4.6, 200.0),
            // As fast as fast-close (within 25%): the lower rate wins.
            row("twin", "fr", 4.8, 115.0),
            row("fast-close", "fr", 5.9, 120.0),
            // fast-close's median speed is 120 whatever one language says.
            row("fast-close", "it", 2.9, 30.0),
            row("slow-best", "it", 2.4, 40.0),
        ];
        rank(&mut rows);
        let ranks: Vec<(&str, &str, usize)> = rows
            .iter()
            .map(|r| (r.language.as_str(), r.model.as_str(), r.rank))
            .collect();
        assert_eq!(
            ranks,
            [
                ("de", "slow-best", 2),
                ("de", "fast-close", 1),
                ("de", "fast-far", 3),
                ("en", "slow-best", 1),
                ("en", "fast-far", 2),
                ("fr", "twin", 1),
                ("fr", "fast-close", 2),
                ("it", "fast-close", 1),
                ("it", "slow-best", 2),
            ]
        );
    }
}

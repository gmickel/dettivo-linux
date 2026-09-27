//! The user's voiceprint (ADR 0075): the mean speaker embedding of the
//! microphone lines that carry the user's own voice, built up over every
//! meeting and kept in one file in the data directory (mode 600). It is
//! computed on this machine by the diarization engine's embedding model
//! and never leaves it; `[meetings.diarization] voiceprint = false` stops
//! the pass from reading or writing it.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file under the data directory.
pub const FILE: &str = "voiceprint.json";
/// The most weight the stored print carries against one new meeting, so
/// a changed microphone or voice moves it within a few dozen meetings.
const MAX_WEIGHT: u32 = 20;
/// The meeting ids the print remembers having folded, newest last.
const REMEMBERED: usize = 200;

/// The stored print.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voiceprint {
    /// The format; 1.
    pub schema: u32,
    /// The diarization model set whose embedding model made the vector; a
    /// print from another model is not comparable and starts over.
    pub model: String,
    /// Meetings folded into the vector.
    pub meetings: u32,
    /// The most recent meetings folded in, so a re-run of the speaker pass
    /// does not count a meeting twice.
    #[serde(default)]
    pub folded: Vec<String>,
    /// The unit-length mean embedding.
    pub vector: Vec<f32>,
}

/// `v` scaled to unit length; None for a zero vector.
pub fn normalised(mut v: Vec<f32>) -> Option<Vec<f32>> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm <= f32::EPSILON || !norm.is_finite() {
        return None;
    }
    v.iter_mut().for_each(|x| *x /= norm);
    Some(v)
}

/// The cosine similarity of two vectors; 0 when their lengths differ.
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    f64::from(dot / (norm(a) * norm(b)).max(f32::EPSILON))
}

/// The unit-length weighted mean of `(vector, weight)` pairs; None when
/// there is nothing to average or the lengths differ.
pub fn centroid<'a>(vectors: impl IntoIterator<Item = (&'a [f32], f32)>) -> Option<Vec<f32>> {
    let mut sum: Option<Vec<f32>> = None;
    for (v, w) in vectors {
        let acc = sum.get_or_insert_with(|| vec![0.0; v.len()]);
        if acc.len() != v.len() {
            return None;
        }
        acc.iter_mut().zip(v).for_each(|(a, x)| *a += x * w);
    }
    normalised(sum?)
}

impl Voiceprint {
    /// The print under `data_dir`, when one exists for `model`; a missing,
    /// unreadable or foreign file reads as none.
    pub fn load(data_dir: &Path, model: &str) -> Option<Self> {
        let text = std::fs::read_to_string(data_dir.join(FILE)).ok()?;
        let print: Self = serde_json::from_str(&text).ok()?;
        (print.schema == 1 && print.model == model && !print.vector.is_empty()).then_some(print)
    }

    /// Folds meeting `id`'s centroid into `previous` (a first print when
    /// there is none or its length differs); None when `previous` already
    /// holds that meeting.
    pub fn fold(previous: Option<Self>, model: &str, id: &str, meeting: &[f32]) -> Option<Self> {
        let previous = previous.filter(|p| p.vector.len() == meeting.len());
        if previous
            .as_ref()
            .is_some_and(|p| p.folded.iter().any(|f| f == id))
        {
            return None;
        }
        let mut folded = previous
            .as_ref()
            .map(|p| p.folded.clone())
            .unwrap_or_default();
        folded.push(id.to_string());
        let excess = folded.len().saturating_sub(REMEMBERED);
        folded.drain(..excess);
        let (meetings, vector) = match &previous {
            None => (1, normalised(meeting.to_vec())?),
            Some(p) => {
                let weight = p.meetings.clamp(1, MAX_WEIGHT) as f32;
                (
                    p.meetings.saturating_add(1),
                    centroid([(p.vector.as_slice(), weight), (meeting, 1.0)])?,
                )
            }
        };
        Some(Self {
            schema: 1,
            model: model.to_string(),
            meetings,
            folded,
            vector,
        })
    }

    /// Writes the print under `data_dir` atomically, readable by the user
    /// alone.
    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::create_dir_all(data_dir)?;
        let tmp = data_dir.join(format!("{FILE}.tmp"));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(serde_json::to_string(self)?.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, data_dir.join(FILE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_average_compare_and_fold_into_a_bounded_print() {
        assert_eq!(normalised(vec![3.0, 4.0]), Some(vec![0.6, 0.8]));
        assert_eq!(normalised(vec![0.0, 0.0]), None);
        assert!((cosine(&[1.0, 0.0], &[2.0, 0.0]) - 1.0).abs() < 1e-6);
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 0.0]), 0.0, "lengths differ");
        let c = centroid([([1.0, 0.0].as_slice(), 1.0), ([0.0, 1.0].as_slice(), 1.0)]).unwrap();
        assert!((c[0] - c[1]).abs() < 1e-6 && (c[0] - 0.707).abs() < 1e-3);
        assert_eq!(
            centroid([([1.0].as_slice(), 1.0), ([1.0, 0.0].as_slice(), 1.0)]),
            None
        );

        let m = "diarization-en";
        let first = Voiceprint::fold(None, m, "a", &[2.0, 0.0]).unwrap();
        assert_eq!((first.meetings, first.vector.clone()), (1, vec![1.0, 0.0]));
        let again = Voiceprint::fold(Some(first.clone()), m, "a", &[0.0, 1.0]);
        assert_eq!(again, None, "a re-run folds nothing");
        let mut print = first;
        for i in 0..240 {
            print = Voiceprint::fold(Some(print), m, &format!("m{i}"), &[0.0, 1.0]).unwrap();
        }
        assert_eq!(print.meetings, 241);
        assert_eq!(print.folded.len(), REMEMBERED);
        assert!(print.vector[1] > 0.99, "a new voice wins: {print:?}");
        let other = Voiceprint::fold(Some(print), m, "b", &[1.0, 0.0, 0.0]).unwrap();
        assert_eq!(other.meetings, 1, "a vector of another length starts over");
    }

    #[test]
    fn the_print_is_stored_for_the_user_alone_and_read_back_for_its_model() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Voiceprint::load(dir.path(), "m"), None);
        let print = Voiceprint::fold(None, "m", "a", &[0.0, 1.0]).unwrap();
        print.save(dir.path()).unwrap();
        let mode = std::fs::metadata(dir.path().join(FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(Voiceprint::load(dir.path(), "m"), Some(print));
        assert_eq!(Voiceprint::load(dir.path(), "other"), None);
        std::fs::write(dir.path().join(FILE), "not json").unwrap();
        assert_eq!(Voiceprint::load(dir.path(), "m"), None);
    }
}

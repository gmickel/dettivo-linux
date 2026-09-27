//! The user's voiceprint (ADR 0075): the mean speaker embedding of the
//! microphone lines that carry the user's own voice, built up over every
//! meeting and kept in one file in the data directory (mode 600). It is
//! computed on this machine by the diarization engine's embedding model
//! and never leaves it; `[meetings.diarization] voiceprint = false` stops
//! the pass from reading or writing it.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

/// The file under the data directory.
pub const FILE: &str = "voiceprint.json";
/// The most weight the stored print carries against one new meeting, so
/// a changed microphone or voice moves it within a few dozen meetings.
const MAX_WEIGHT: u32 = 20;
/// The meeting ids the print remembers having folded, newest last.
const REMEMBERED: usize = 200;
/// Held across every load, fold and save of the print, so two meetings'
/// passes finishing together both land in it.
static UPDATE: Mutex<()> = Mutex::new(());
/// Numbers each write's temporary file, so no two writes share one.
static WRITES: AtomicU64 = AtomicU64::new(0);

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

    /// Folds meeting `id`'s centroid into the print under `data_dir` and
    /// stores the result; false when the print already holds that meeting.
    /// One process-wide lock covers the whole read, fold and write.
    pub fn enrol(data_dir: &Path, model: &str, id: &str, meeting: &[f32]) -> std::io::Result<bool> {
        let _guard = UPDATE.lock().unwrap_or_else(PoisonError::into_inner);
        match Self::fold(Self::load(data_dir, model), model, id, meeting) {
            Some(print) => print.save(data_dir).map(|()| true),
            None => Ok(false),
        }
    }

    /// This print averaged with one new meeting's centroid, the print
    /// weighing as many meetings as it holds, up to 20 to 1; None when the
    /// lengths differ.
    pub fn blend(&self, meeting: &[f32]) -> Option<Vec<f32>> {
        let weight = self.meetings.clamp(1, MAX_WEIGHT) as f32;
        centroid([(self.vector.as_slice(), weight), (meeting, 1.0)])
    }

    /// Folds meeting `id`'s centroid into `previous` (a first print when
    /// there is none or its length differs); None when `previous` already
    /// holds that meeting.
    fn fold(previous: Option<Self>, model: &str, id: &str, meeting: &[f32]) -> Option<Self> {
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
            Some(p) => (p.meetings.saturating_add(1), p.blend(meeting)?),
        };
        Some(Self {
            schema: 1,
            model: model.to_string(),
            meetings,
            folded,
            vector,
        })
    }

    /// Writes the print under `data_dir` atomically through a temporary
    /// file of its own, readable by the user alone.
    fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(data_dir)?;
        let n = WRITES.fetch_add(1, Ordering::Relaxed);
        let tmp = data_dir.join(format!("{FILE}.{}.{n}.tmp", std::process::id()));
        write_private(&tmp, serde_json::to_string(self)?.as_bytes())?;
        std::fs::rename(&tmp, data_dir.join(FILE))
    }
}

/// Writes `bytes` to a new file at `path` readable by the user alone. A file
/// left there by an earlier process (a crash, then a reused pid) is removed
/// first, so its permissions never carry into the file this writes.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_temporary_file_never_lends_its_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stale.tmp");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

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

    #[test]
    fn the_stored_print_outweighs_one_meeting_up_to_twenty_to_one() {
        let print = |meetings| Voiceprint {
            schema: 1,
            model: "m".into(),
            meetings,
            folded: Vec::new(),
            vector: vec![1.0, 0.0],
        };
        let tan = |v: Vec<f32>| v[1] / v[0];
        assert!((tan(print(1).blend(&[0.0, 1.0]).unwrap()) - 1.0).abs() < 1e-6);
        assert!((tan(print(5).blend(&[0.0, 1.0]).unwrap()) - 0.2).abs() < 1e-6);
        assert!((tan(print(500).blend(&[0.0, 1.0]).unwrap()) - 0.05).abs() < 1e-6);
        assert_eq!(print(5).blend(&[0.0, 1.0, 0.0]), None, "lengths differ");
    }

    #[test]
    fn meetings_enrolling_at_once_all_land_in_the_print() {
        let dir = tempfile::tempdir().unwrap();
        let ids: Vec<String> = (0..16).map(|i| format!("meeting-{i}")).collect();
        std::thread::scope(|scope| {
            for id in &ids {
                let dir = dir.path();
                scope.spawn(move || Voiceprint::enrol(dir, "m", id, &[0.0, 1.0]).unwrap());
            }
        });
        let print = Voiceprint::load(dir.path(), "m").unwrap();
        assert_eq!(print.meetings, 16);
        let mut folded = print.folded.clone();
        folded.sort();
        let mut expected = ids.clone();
        expected.sort();
        assert_eq!(folded, expected);
        assert!(!Voiceprint::enrol(dir.path(), "m", &ids[0], &[0.0, 1.0]).unwrap());
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [FILE], "no temporary file is left behind");
    }
}

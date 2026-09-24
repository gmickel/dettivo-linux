//! One background job per meeting (the analysis, the speaker pass) with
//! its identity: a result is committed only by the job that still owns
//! the meeting, under the same lock a delete takes to invalidate every
//! job for it, so a job can neither restore what a delete cleared nor
//! remove a newer job's entry when it finishes late.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// One job in flight.
#[derive(Debug, Clone)]
pub struct Job {
    /// The job id (`job_analysis_<n>`, `job_diarize_<n>`).
    pub job_id: String,
    /// Set to stop the run.
    pub cancel: Arc<AtomicBool>,
    /// Chunks done and total, for the progress the job reports.
    pub chunks: (u32, u32),
}

impl Job {
    /// A fresh job with its cancel flag clear.
    pub fn new(job_id: String) -> Self {
        Self {
            job_id,
            cancel: Arc::new(AtomicBool::new(false)),
            chunks: (0, 0),
        }
    }
}

/// The jobs in flight, one per meeting.
#[derive(Default)]
pub struct MeetingJobs {
    jobs: Mutex<HashMap<String, Job>>,
}

impl MeetingJobs {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Job>> {
        self.jobs.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The job on `meeting_id`, when one runs.
    pub fn get(&self, meeting_id: &str) -> Option<Job> {
        self.lock().get(meeting_id).cloned()
    }

    /// Registers `job` on `meeting_id`; the caller checked none runs.
    pub fn insert(&self, meeting_id: &str, job: Job) {
        self.lock().insert(meeting_id.to_string(), job);
    }

    /// Removes the entry when it is still `job_id`'s; a later job's entry
    /// stays. True when removed.
    pub fn remove(&self, meeting_id: &str, job_id: &str) -> bool {
        let mut g = self.lock();
        if g.get(meeting_id).is_some_and(|j| j.job_id == job_id) {
            g.remove(meeting_id);
            return true;
        }
        false
    }

    /// Asks the job on `meeting_id` to stop and keeps its entry, so the
    /// job records how it ended. True when one ran.
    pub fn cancel(&self, meeting_id: &str) -> bool {
        match self.lock().get(meeting_id) {
            Some(j) => {
                j.cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// Stops the job on `meeting_id` and forgets it: its result is
    /// dropped at commit, never written. Waits for a commit in progress,
    /// so what the caller does next sees the job's write or none. True
    /// when one ran.
    pub fn invalidate(&self, meeting_id: &str) -> bool {
        match self.lock().remove(meeting_id) {
            Some(j) => {
                j.cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// Runs `write` under the lock when `job_id` still owns
    /// `meeting_id`, then forgets the job; `None` when another job owns
    /// the meeting or the job was invalidated, and the result is dropped.
    pub fn commit<T>(
        &self,
        meeting_id: &str,
        job_id: &str,
        write: impl FnOnce() -> T,
    ) -> Option<T> {
        let mut g = self.lock();
        if g.get(meeting_id).is_none_or(|j| j.job_id != job_id) {
            return None;
        }
        let out = write();
        g.remove(meeting_id);
        Some(out)
    }

    /// Records the job's progress.
    pub fn progress(&self, meeting_id: &str, chunks: (u32, u32)) {
        if let Some(j) = self.lock().get_mut(meeting_id) {
            j.chunks = chunks;
        }
    }

    /// How many jobs run.
    pub fn len(&self) -> usize {
        self.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::sync::atomic::AtomicUsize;

    /// meetings/F4 (fn-43): a job that finishes late neither removes nor
    /// commits over the newer job on the same meeting.
    #[test]
    fn an_old_job_neither_removes_nor_commits_over_a_newer_one() {
        let jobs = MeetingJobs::default();
        jobs.insert("m", Job::new("job_1".into()));
        assert!(jobs.invalidate("m"));
        assert!(!jobs.invalidate("m"));
        jobs.insert("m", Job::new("job_2".into()));
        assert!(
            !jobs.remove("m", "job_1"),
            "the old job cannot remove the new entry"
        );
        assert_eq!(jobs.get("m").unwrap().job_id, "job_2");
        assert!(
            jobs.commit("m", "job_1", || ()).is_none(),
            "the old result is dropped"
        );
        assert!(jobs.commit("m", "job_2", || ()).is_some());
        assert!(jobs.get("m").is_none(), "a commit forgets the job");
        assert_eq!(jobs.len(), 0);
    }

    /// meetings/F4 (fn-43): a cancel keeps the entry so the job records
    /// its end; an invalidation before the commit drops the result.
    #[test]
    fn a_cancel_keeps_the_entry_and_an_invalidation_drops_the_result() {
        let jobs = MeetingJobs::default();
        let job = Job::new("job_1".into());
        jobs.insert("m", job.clone());
        assert!(jobs.cancel("m"));
        assert!(job.cancel.load(Ordering::SeqCst));
        assert!(
            jobs.commit("m", "job_1", || 7) == Some(7),
            "a cancelled job still records its end"
        );
        jobs.insert("m", Job::new("job_2".into()));
        jobs.progress("m", (1, 4));
        assert_eq!(jobs.get("m").unwrap().chunks, (1, 4));
        assert!(jobs.invalidate("m"));
        assert!(jobs.commit("m", "job_2", || 7).is_none());
    }

    /// meetings/F4 (fn-43): a delete that lands while a job commits waits
    /// for the write, so it clears after it; one that lands first makes
    /// the commit drop the result.
    #[test]
    fn an_invalidation_during_a_commit_waits_for_the_write() {
        let jobs = Arc::new(MeetingJobs::default());
        jobs.insert("m", Job::new("job_1".into()));
        let inside = Arc::new(Barrier::new(2));
        let writes = Arc::new(AtomicUsize::new(0));
        let worker = {
            let (jobs, inside, writes) = (jobs.clone(), inside.clone(), writes.clone());
            std::thread::spawn(move || {
                jobs.commit("m", "job_1", || {
                    inside.wait();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    writes.fetch_add(1, Ordering::SeqCst);
                })
            })
        };
        inside.wait();
        // The commit holds the meeting: this waits until the write is done.
        assert!(
            !jobs.invalidate("m"),
            "the job was already committed and forgotten"
        );
        assert_eq!(writes.load(Ordering::SeqCst), 1);
        assert!(worker.join().unwrap().is_some());
    }
}

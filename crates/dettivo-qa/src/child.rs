//! Ownership for asynchronous QA helper children across normal and error returns.

use std::ops::{Deref, DerefMut};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub(crate) struct ChildOwner(Child);

impl ChildOwner {
    pub(crate) fn new(child: Child) -> Self {
        Self(child)
    }
}

impl Deref for ChildOwner {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.0
    }
}

impl DerefMut for ChildOwner {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for ChildOwner {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.id().to_string()])
            .output();
        let deadline = Instant::now() + Duration::from_secs(5);
        while matches!(self.0.try_wait(), Ok(None)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

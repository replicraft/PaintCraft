//! Retire the entire native writer, including all earlier wake callbacks.
use super::*;
pub(in crate::clipboard) fn finish(
    worker: Option<std::thread::JoinHandle<()>>,
    state: &State,
) -> Result<(), String> {
    let panicked = if let Some(worker) = worker {
        if worker.thread().id() == std::thread::current().id() {
            return Err("Clipboard service cannot join its own worker".into());
        }
        worker.join().is_err()
    } else {
        false
    };
    if panicked
        || state.poisoned.load(Ordering::Acquire)
        || state.shutdown_failed.load(Ordering::Acquire)
    {
        Err("Clipboard worker did not retire cleanly".into())
    } else {
        Ok(())
    }
}
impl ClipboardService {
    /// Stop admission, cancel queued/cooperative work, and join the actual worker.
    /// Call from a drain thread. Opaque native calls may block until they return.
    /// Job results and escaped image aliases still belong to their callers.
    /// Ordinary Drop keeps its nonblocking cancellation-only behavior.
    pub fn shutdown(mut self) -> Result<(), String> {
        self.state.closed.store(true, Ordering::Release);
        self.sender.take();
        finish(self.worker.take(), &self.state)
    }
}
#[cfg(test)]
#[path = "shutdown/tests.rs"]
mod tests;

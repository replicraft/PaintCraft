//! Explicit join of the encoded reader and all its prior wake callbacks.
use super::*;
impl ReadService {
    /// Stop admission and join the actual native reader before replacement.
    /// Call from the drain worker. Opaque OS calls cannot be preempted here.
    /// Pending Job/encoded-image owners must also be retired by their callers.
    /// Default Drop remains nonblocking.
    pub fn shutdown(mut self) -> Result<(), String> {
        self.state.closed.store(true, Ordering::Release);
        self.sender.take();
        crate::clipboard::shutdown::finish(self.worker.take(), &self.state)
    }
}
#[cfg(test)]
#[path = "shutdown/tests.rs"]
mod tests;

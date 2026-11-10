//! Blocking retirement is explicit and runs outside the UI thread.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
impl NativeDialogs {
    /// Wait for the actual dialog thread and its wake callback, then discard
    /// undispatched completions. An open OS dialog cannot be preempted here.
    /// Ordinary Drop remains nonblocking; call from the update drain worker.
    ///
    /// # Errors
    /// Returns an invariant error if this or a prior dialog worker panicked.
    pub fn shutdown(mut self) -> Result<()> {
        if let Some(worker) = self.worker.take() {
            if worker.thread().id() == std::thread::current().id() {
                return Err(failed());
            }
            self.failed |= worker.join().is_err();
        }
        while self.receiver.try_recv().is_ok() {}
        if self.failed { Err(failed()) } else { Ok(()) }
    }
}
fn failed() -> Error {
    Error::new(
        ErrorCode::InvariantViolation,
        "native dialog worker did not retire cleanly",
    )
}
#[cfg(test)]
#[path = "shutdown/tests.rs"]
mod tests;

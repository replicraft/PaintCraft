//! Explicit observation-thread retirement, separate from a stop request.
use super::*;
use std::sync::MutexGuard;

fn failed() -> Error {
    Error::new(
        ErrorCode::InvariantViolation,
        "GPU completion observer did not retire cleanly",
    )
}
fn status(state: &State) -> MutexGuard<'_, Status> {
    match state.status.lock() {
        Ok(status) => status,
        Err(poison) => {
            let mut status = poison.into_inner();
            status.failed = true;
            status
        }
    }
}
impl CompletionDriver {
    /// Stop the shared observer and join its actual thread, including captured
    /// device/worker owners. All concurrent calls wait for that same completed join.
    /// Call this from the shutdown worker after render executors have retired.
    ///
    /// Unlike ordinary Drop, this operation blocks. It closes all driver aliases
    /// to new tracking. `progress().closed` alone remains only a stop-request flag.
    /// This does not independently wait for GPU idle or arbitrary external tickets.
    ///
    /// # Errors
    /// Returns an invariant error after retirement for worker panic or poisoned
    /// coordination. Calling from the observer itself refuses without losing its
    /// join handle; another thread must complete retirement.
    pub fn shutdown(self) -> Result<()> {
        let state = &self.0.0;
        {
            status(state).closed = true;
            state.changed.notify_all();
        }
        let worker = {
            let mut worker = match self.0.1.lock() {
                Ok(worker) => worker,
                Err(poison) => {
                    status(state).failed = true;
                    poison.into_inner()
                }
            };
            if worker
                .as_ref()
                .is_some_and(|worker| worker.thread().id() == std::thread::current().id())
            {
                status(state).failed = true;
                return Err(failed());
            }
            worker.take()
        };
        if let Some(worker) = worker {
            let panicked = worker.join().is_err();
            let mut current = status(state);
            current.failed |= panicked;
            // The ACK belongs to the joining caller, never the exiting worker:
            // a second alias cannot return before captures and thread exit retire.
            current.retired = true;
            state.changed.notify_all();
        } else {
            let mut current = status(state);
            while !current.retired {
                current = match state.changed.wait(current) {
                    Ok(current) => current,
                    Err(poison) => {
                        let mut current = poison.into_inner();
                        current.failed = true;
                        current
                    }
                };
            }
        }
        if status(state).failed {
            Err(failed())
        } else {
            Ok(())
        }
    }
}
#[cfg(test)]
#[path = "shutdown/tests.rs"]
mod tests;

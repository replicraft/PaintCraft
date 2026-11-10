//! Explicit task, wake, result and Rayon exit-callback retirement.
use super::*;
use std::sync::{Condvar, Mutex, MutexGuard};

#[derive(Debug, Default)]
struct State {
    active: usize,
    exited: usize,
    failed: bool,
}

#[derive(Debug)]
pub(super) struct Completion {
    state: Mutex<State>,
    changed: Condvar,
    workers: usize,
}

pub(super) struct Guard(Arc<Completion>);

fn failure() -> Error {
    Error::new(
        ErrorCode::InvariantViolation,
        "worker shutdown encountered a panic or failed bookkeeping",
    )
}

impl Completion {
    pub(super) fn new(workers: usize) -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            workers,
        }
    }

    // No user code runs under this mutex. A poisoned coordinator still drains
    // existing work; it records failure rather than granting shutdown success.
    fn lock(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poison) => {
                let mut state = poison.into_inner();
                state.failed = true;
                state
            }
        }
    }

    pub(super) fn begin(self: &Arc<Self>) -> Result<Guard> {
        let mut state = self.lock();
        state.active = state.active.checked_add(1).ok_or_else(failure)?;
        Ok(Guard(self.clone()))
    }

    pub(super) fn worker_exited(&self) {
        let mut state = self.lock();
        if state.exited < self.workers {
            state.exited += 1;
        } else {
            state.failed = true;
        }
        self.changed.notify_all();
    }

    fn failed(&self) {
        self.lock().failed = true;
    }

    fn wait(&self, workers: bool) {
        let mut state = self.lock();
        while if workers {
            state.exited < self.workers
        } else {
            state.active != 0
        } {
            state = match self.changed.wait(state) {
                Ok(state) => state,
                Err(poison) => {
                    let mut state = poison.into_inner();
                    state.failed = true;
                    state
                }
            };
        }
    }

    fn drop_checked(&self, value: impl Sized) {
        if catch_unwind(AssertUnwindSafe(|| drop(value))).is_err() {
            self.failed();
        }
    }
}

impl Guard {
    pub(super) fn failed(&self) {
        self.0.failed();
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        if state.active == 0 {
            state.failed = true;
        } else {
            state.active -= 1;
        }
        self.0.changed.notify_all();
    }
}

impl<T: Send + 'static> Executor<T> {
    /// Cancel all admitted jobs and wait for submitted tasks and wake callbacks,
    /// drop their retained results, then wait for the pool's exit callbacks.
    ///
    /// This consumes the executor and does not publish results. Original byte
    /// admission is released through the ordinary owners; no budget is reset.
    /// The boundary is Rayon's exit callback, not an OS-thread join or arbitrary
    /// thread-local destructor completion. An uncooperative task or wake callback
    /// can block this operation; installation must not proceed while it waits.
    /// Ordinary executor drop remains cancellation-only.
    ///
    /// # Errors
    /// Returns an invariant error after retirement if a task, wake callback,
    /// retained result destructor, or internal synchronization panicked.
    pub fn shutdown(mut self) -> Result<()> {
        for job in self.jobs.values() {
            job.cancellation.0.store(true, Ordering::Release);
        }
        self.completion.wait(false);
        // Workers send before releasing their active guard, so every packet is
        // available now. Drop payloads before their paired admission leases.
        while let Ok(packet) = self.receiver.try_recv() {
            self.completion.drop_checked(packet);
        }
        while let Some(key) = self.jobs.keys().next() {
            let job = self.jobs.remove(key).expect("selected live job");
            self.completion.drop_checked(job);
        }
        drop(self.pool.take());
        self.completion.wait(true);
        if self.completion.lock().failed {
            Err(failure())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;

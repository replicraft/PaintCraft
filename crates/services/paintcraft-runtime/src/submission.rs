//! Admission failure returns the original unexecuted task and wake callback.
use super::*;

pub struct RejectedSubmission<F, W> {
    pub error: Error,
    pub task: F,
    pub wake: W,
}
impl<F, W> std::fmt::Debug for RejectedSubmission<F, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RejectedSubmission")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl<T: Send + 'static> Executor<T> {
    /// Atomically admit work, or return both closures without running or losing them.
    /// Rejection charges no bytes and creates no job handle. The caller owns retry policy.
    pub fn submit_retained<F, W>(
        &mut self,
        bytes: u64,
        task: F,
        wake: W,
    ) -> std::result::Result<JobHandle, RejectedSubmission<F, W>>
    where
        F: FnOnce(Cancellation) -> Result<T> + Send + 'static,
        W: FnOnce() + Send + 'static,
    {
        if self
            .local_budget
            .usage()
            .reserved
            .checked_add(bytes)
            .is_none()
        {
            return Err(RejectedSubmission {
                error: Error::new(ErrorCode::ResourceLimit, "worker reservation overflows"),
                task,
                wake,
            });
        }
        if self.jobs.len() >= self.limits.pending_jobs || !self.local_budget.can_reserve(bytes) {
            return Err(RejectedSubmission {
                error: Error::new(
                    ErrorCode::ResourceLimit,
                    "worker queue or memory budget is full",
                ),
                task,
                wake,
            });
        }
        let local = match self.local_budget.reserve(bytes) {
            Ok(lease) => lease,
            Err(error) => return Err(RejectedSubmission { error, task, wake }),
        };
        let lease = match self.budget.reserve(bytes) {
            Ok(lease) => lease,
            Err(error) => return Err(RejectedSubmission { error, task, wake }),
        };
        let lease = admission::Lease::pair(local, lease);
        let cancellation = Cancellation(Arc::new(AtomicBool::new(false)), lease.downgrade());
        let key = self.jobs.insert(Reservation {
            cancellation: cancellation.clone(),
            ready: None,
            abandoned: false,
            _lease: lease.clone(),
        });
        let sender = self.sender.clone();
        self.pool.spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                cancellation.check()?;
                let result = task(cancellation.clone());
                cancellation.check()?;
                result
            }))
            .unwrap_or_else(|_| {
                Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "worker panicked before publication",
                ))
            });
            let _ = sender.send((key, result, lease));
            let _ = catch_unwind(AssertUnwindSafe(wake));
        });
        Ok(JobHandle {
            owner: self.owner,
            key,
        })
    }
}

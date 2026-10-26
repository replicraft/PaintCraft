//! Bounded workers prepare results; only the caller may publish them.
//! Reservations remain charged until completion is consumed, including cancellation.
mod admission;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;
mod submission;
pub use admission::{BudgetUsage, SharedBudget};
pub use submission::RejectedSubmission;

#[cfg(feature = "native-io")]
pub mod io;

use paintcraft_core::types::{Error, ErrorCode, Result};
use slotmap::{SlotMap, new_key_type};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
use uuid::Uuid;

new_key_type! { struct JobKey; }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JobHandle {
    owner: Uuid,
    key: JobKey,
}
#[derive(Clone, Debug)]
pub struct Cancellation(
    Arc<AtomicBool>,
    paintcraft_core::types::resources::WeakResourceLease,
);
impl Cancellation {
    /// Retain this admitted charge for external work that can outlive the task.
    /// Token clones alone do not extend the reservation.
    pub fn resource_lease(&self) -> Option<paintcraft_core::types::resources::ResourceLease> {
        self.1.upgrade()
    }

    pub fn check(&self) -> Result<()> {
        if self.0.load(Ordering::Acquire) {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "job was cancelled",
            ))
        } else {
            Ok(())
        }
    }
}
#[derive(Debug)]
struct Reservation<T> {
    cancellation: Cancellation,
    ready: Option<Result<T>>,
    abandoned: bool,
    _lease: admission::Lease,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub workers: usize,
    pub pending_jobs: usize,
    pub transient_bytes: u64,
}
#[derive(Debug)]
pub struct Executor<T> {
    owner: Uuid,
    pool: rayon::ThreadPool,
    limits: Limits,
    jobs: SlotMap<JobKey, Reservation<T>>,
    local_budget: SharedBudget,
    budget: SharedBudget,
    sender: mpsc::Sender<(JobKey, Result<T>, admission::Lease)>,
    receiver: mpsc::Receiver<(JobKey, Result<T>, admission::Lease)>,
}
impl<T: Send + 'static> Executor<T> {
    pub fn new(limits: Limits) -> Result<Self> {
        Self::with_budget(limits, SharedBudget::new(limits.transient_bytes))
    }
    /// Share byte admission across pools while retaining each pool's local cap.
    pub fn with_budget(limits: Limits, budget: SharedBudget) -> Result<Self> {
        if limits.workers == 0 || limits.pending_jobs == 0 || limits.transient_bytes == 0 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "worker, queue and memory limits must be positive",
            ));
        }
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(limits.workers)
            .thread_name(|i| format!("paintcraft-worker-{i}"))
            .build()
            .map_err(|e| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    format!("cannot create worker pool: {e}"),
                )
            })?;
        let (sender, receiver) = mpsc::channel();
        Ok(Self {
            owner: Uuid::new_v4(),
            pool,
            limits,
            jobs: SlotMap::with_key(),
            local_budget: SharedBudget::new(limits.transient_bytes),
            budget,
            sender,
            receiver,
        })
    }
    pub fn pending(&self) -> usize {
        self.jobs.len()
    }
    pub fn reserved_bytes(&self) -> u64 {
        self.local_budget.usage().reserved
    }
    /// Whether a request can ever fit, independent of current contention.
    pub fn request_fits(&self, bytes: u64) -> bool {
        bytes <= self.limits.transient_bytes && bytes <= self.budget.usage().limit
    }
    /// A readiness hint only; submission atomically reserves the shared bytes.
    pub fn can_submit(&self, bytes: u64) -> bool {
        self.jobs.len() < self.limits.pending_jobs
            && self.local_budget.can_reserve(bytes)
            && self.budget.can_reserve(bytes)
    }
    pub fn submit(
        &mut self,
        bytes: u64,
        task: impl FnOnce(Cancellation) -> Result<T> + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<JobHandle> {
        self.submit_retained(bytes, task, wake)
            .map_err(|rejected| rejected.error)
    }

    pub fn cancel(&self, handle: JobHandle) -> bool {
        if handle.owner != self.owner {
            return false;
        }
        let Some(job) = self.jobs.get(handle.key) else {
            return false;
        };
        job.cancellation.0.store(true, Ordering::Release);
        true
    }
    /// Cancel a result whose consumer has gone away. Its reservation remains
    /// charged until worker completion is drained by a poll operation.
    pub fn abandon(&mut self, handle: JobHandle) -> bool {
        if !self.cancel(handle) {
            return false;
        }
        let job = self
            .jobs
            .get_mut(handle.key)
            .expect("cancel validated handle");
        job.abandoned = true;
        if job.ready.is_some() {
            self.jobs.remove(handle.key).expect("live job");
        }
        true
    }

    /// Drain notifications and release abandoned completions. Results belonging
    /// to live consumers remain available and retain their reservations.
    pub fn reap_abandoned(&mut self) {
        self.receive_ready();
    }

    fn receive_ready(&mut self) {
        while let Ok((key, result, _lease)) = self.receiver.try_recv() {
            if let Some(job) = self.jobs.get_mut(key) {
                if job.abandoned {
                    self.jobs.remove(key).expect("live job");
                } else {
                    job.ready = Some(result);
                }
            }
        }
    }

    fn take_ready(&mut self, key: JobKey) -> Option<Result<T>> {
        self.jobs.get(key)?.ready.as_ref()?;
        let job = self.jobs.remove(key)?;
        Some(
            job.cancellation
                .check()
                .and(job.ready.expect("completed job")),
        )
    }

    /// Poll one consumer without consuming another consumer's result. Other
    /// completed results remain charged until their owners consume or abandon them.
    pub fn poll_for(&mut self, handle: JobHandle) -> Option<Result<T>> {
        if handle.owner != self.owner {
            return None;
        }
        self.receive_ready();
        self.take_ready(handle.key)
    }

    pub fn poll(&mut self) -> Option<(JobHandle, Result<T>)> {
        self.receive_ready();
        let key = self
            .jobs
            .iter()
            .find(|(_, job)| job.ready.is_some())
            .map(|(key, _)| key)?;
        let result = self.take_ready(key)?;
        Some((
            JobHandle {
                owner: self.owner,
                key,
            },
            result,
        ))
    }
}
impl<T> Drop for Executor<T> {
    fn drop(&mut self) {
        for job in self.jobs.values() {
            job.cancellation.0.store(true, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn executor() -> Executor<u64> {
        Executor::new(Limits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 32,
        })
        .unwrap()
    }
    fn completion(executor: &mut Executor<u64>) -> (JobHandle, Result<u64>) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = executor.poll() {
                return result;
            }
            assert!(Instant::now() < deadline, "worker stalled");
            std::thread::yield_now();
        }
    }
    #[test]
    fn cancelling_running_work_keeps_its_reservation_until_completion() {
        let mut executor = executor();
        let (started, wait_started) = mpsc::sync_channel(1);
        let (release, wait_release) = mpsc::sync_channel(1);
        let handle = executor
            .submit(
                24,
                move |_| {
                    started.send(()).unwrap();
                    wait_release.recv().unwrap();
                    Ok(9)
                },
                || {},
            )
            .unwrap();
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(executor.cancel(handle));
        assert_eq!(executor.reserved_bytes(), 24);
        assert!(executor.submit(9, |_| Ok(1), || {}).is_err());
        release.send(()).unwrap();
        let (finished, result) = completion(&mut executor);
        assert_eq!(finished, handle);
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(executor.reserved_bytes(), 0);
        assert!(!executor.cancel(handle));
    }
    #[test]
    fn handles_cannot_cross_owners_or_resurrect_after_slot_reuse() {
        let mut first = executor();
        let mut second = executor();
        let old = first.submit(8, |_| Ok(7), || {}).unwrap();
        assert!(!second.cancel(old));
        assert_eq!(completion(&mut first).1.unwrap(), 7);
        let current = first.submit(8, |_| Ok(8), || {}).unwrap();
        assert_ne!(old, current);
        assert!(!first.cancel(old));
        assert_eq!(completion(&mut first).1.unwrap(), 8);
        let other = second.submit(8, |_| Ok(3), || {}).unwrap();
        assert!(!first.cancel(other));
        assert_eq!(completion(&mut second).1.unwrap(), 3);
    }
    #[test]
    fn panics_are_failed_preparations_and_release_admission_on_consumption() {
        let mut executor = executor();
        executor
            .submit(32, |_| panic!("injected kernel failure"), || {})
            .unwrap();
        assert_eq!(
            completion(&mut executor).1.unwrap_err().code,
            ErrorCode::InvariantViolation
        );
        assert_eq!(executor.reserved_bytes(), 0);
        executor.submit(32, |_| Ok(42), || {}).unwrap();
        assert_eq!(completion(&mut executor).1.unwrap(), 42);
    }
    #[test]
    fn targeted_consumers_retain_other_completed_results_and_their_reservations() {
        let mut executor = executor();
        let (finished, wait_finished) = mpsc::sync_channel(2);
        let first_wake = finished.clone();
        let first = executor
            .submit(16, |_| Ok(11), move || first_wake.send(()).unwrap())
            .unwrap();
        let second = executor
            .submit(16, |_| Ok(22), move || finished.send(()).unwrap())
            .unwrap();
        for _ in 0..2 {
            wait_finished.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert_eq!(executor.poll_for(second).unwrap().unwrap(), 22);
        assert_eq!(executor.reserved_bytes(), 16);
        assert_eq!(executor.pending(), 1);
        assert!(executor.submit(17, |_| Ok(0), || {}).is_err());
        assert!(executor.cancel(first));
        assert_eq!(
            executor.poll_for(first).unwrap().unwrap_err().code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(executor.reserved_bytes(), 0);
        assert!(executor.poll_for(second).is_none());
    }

    #[test]
    fn abandoning_running_work_releases_admission_only_after_actual_completion() {
        let mut executor = executor();
        let (started, wait_started) = mpsc::sync_channel(1);
        let (release, wait_release) = mpsc::sync_channel(1);
        let (finished, wait_finished) = mpsc::sync_channel(1);
        let handle = executor
            .submit(
                32,
                move |_| {
                    started.send(()).unwrap();
                    wait_release.recv().unwrap();
                    Ok(7)
                },
                move || finished.send(()).unwrap(),
            )
            .unwrap();
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(executor.abandon(handle));
        assert_eq!(executor.reserved_bytes(), 32);
        assert!(executor.submit(1, |_| Ok(0), || {}).is_err());
        assert!(executor.poll().is_none());
        release.send(()).unwrap();
        wait_finished.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(executor.poll().is_none());
        assert_eq!(executor.reserved_bytes(), 0);
        assert!(!executor.abandon(handle));
        executor.submit(32, |_| Ok(9), || {}).unwrap();
        assert_eq!(completion(&mut executor).1.unwrap(), 9);
    }
    #[test]
    fn abandonment_of_buffered_results_and_foreign_handles_preserves_other_owners() {
        let mut executor = executor();
        let mut foreign = super::Executor::<u64>::new(Limits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 32,
        })
        .unwrap();
        let (finished, wait_finished) = mpsc::sync_channel(1);
        let handle = executor
            .submit(32, |_| Ok(17), move || finished.send(()).unwrap())
            .unwrap();
        wait_finished.recv_timeout(Duration::from_secs(5)).unwrap();
        executor.reap_abandoned();
        assert_eq!(executor.reserved_bytes(), 32);
        assert!(foreign.poll_for(handle).is_none());
        assert!(!foreign.abandon(handle));
        assert!(executor.abandon(handle));
        assert_eq!(executor.reserved_bytes(), 0);
        assert!(executor.poll_for(handle).is_none());
    }
}

#[cfg(test)]
#[path = "admission/tests.rs"]
mod admission_tests;

#[cfg(test)]
mod submission_tests;

#[cfg(test)]
mod resource_tests;

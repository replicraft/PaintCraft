//! Bounded workers prepare results; only the caller may publish them.
//! Reservations remain charged until completion is consumed, including cancellation.
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
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
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
struct Reservation {
    bytes: u64,
    cancellation: Cancellation,
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
    jobs: SlotMap<JobKey, Reservation>,
    reserved: u64,
    sender: mpsc::Sender<(JobKey, Result<T>)>,
    receiver: mpsc::Receiver<(JobKey, Result<T>)>,
}
impl<T: Send + 'static> Executor<T> {
    pub fn new(limits: Limits) -> Result<Self> {
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
            reserved: 0,
            sender,
            receiver,
        })
    }
    pub fn pending(&self) -> usize {
        self.jobs.len()
    }
    pub fn reserved_bytes(&self) -> u64 {
        self.reserved
    }
    pub fn submit(
        &mut self,
        bytes: u64,
        task: impl FnOnce(Cancellation) -> Result<T> + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<JobHandle> {
        let reserved = self
            .reserved
            .checked_add(bytes)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "worker reservation overflows"))?;
        if self.jobs.len() >= self.limits.pending_jobs || reserved > self.limits.transient_bytes {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "worker queue or memory budget is full",
            ));
        }
        let cancellation = Cancellation(Arc::new(AtomicBool::new(false)));
        let key = self.jobs.insert(Reservation {
            bytes,
            cancellation: cancellation.clone(),
        });
        self.reserved = reserved;
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
            let _ = sender.send((key, result));
            let _ = catch_unwind(AssertUnwindSafe(wake));
        });
        Ok(JobHandle {
            owner: self.owner,
            key,
        })
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
    pub fn poll(&mut self) -> Option<(JobHandle, Result<T>)> {
        let (key, result) = self.receiver.try_recv().ok()?;
        let reservation = self.jobs.remove(key)?;
        self.reserved -= reservation.bytes;
        let result = reservation.cancellation.check().and(result);
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
}

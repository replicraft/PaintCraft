//! Retain actual app worker handles after result delivery, through wake completion.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::{io, thread::JoinHandle};

#[derive(Debug, Default)]
pub(super) struct Threads {
    workers: Vec<JoinHandle<()>>,
    failed: bool,
}
impl Threads {
    fn join(worker: JoinHandle<()>) -> bool {
        if worker.thread().id() == std::thread::current().id() {
            return false;
        }
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.join().is_ok()))
            .unwrap_or(false)
    }
    fn reap(&mut self) {
        let mut index = 0;
        while index < self.workers.len() {
            if self.workers[index].is_finished() {
                let worker = self.workers.swap_remove(index);
                self.failed |= !Self::join(worker);
            } else {
                index += 1;
            }
        }
    }
    pub(super) fn busy(&self) -> bool {
        self.workers.iter().any(|worker| !worker.is_finished())
    }
    pub(super) fn spawn(
        &mut self,
        name: &str,
        work: impl FnOnce() + Send + 'static,
    ) -> io::Result<()> {
        self.reap();
        // Cancellation can release an owner's normal admission before its old
        // wake returns. Keep that handle while accepting the authorized retry.
        self.workers.try_reserve(1).map_err(|_| {
            io::Error::new(io::ErrorKind::OutOfMemory, "cannot retain worker handle")
        })?;
        let worker = std::thread::Builder::new().name(name.into()).spawn(work)?;
        self.workers.push(worker);
        Ok(())
    }
    pub(super) fn take(&mut self) -> Self {
        std::mem::take(self)
    }
    pub(super) fn shutdown(mut self) -> Result<()> {
        for worker in self.workers.drain(..) {
            self.failed |= !Self::join(worker);
        }
        if self.failed {
            Err(Error::new(
                ErrorCode::InvariantViolation,
                "application worker did not retire cleanly",
            ))
        } else {
            Ok(())
        }
    }
}

pub(super) type Shutdown = Box<dyn FnOnce() -> Result<()> + Send>;

fn run_all(jobs: Vec<Shutdown>) -> Result<()> {
    let mut failure = None;
    for job in jobs {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).unwrap_or_else(|_| {
                Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "shutdown owner panicked",
                ))
            });
        if let Err(error) = result {
            failure.get_or_insert(error);
        }
    }
    failure.map_or(Ok(()), Err)
}

pub(super) struct Drain {
    worker: Threads,
    outcome: std::sync::Arc<std::sync::Mutex<Option<Result<()>>>>,
}
impl Drain {
    pub(super) fn start(jobs: Vec<Shutdown>, ctx: &eframe::egui::Context) -> Result<Self> {
        let jobs = std::sync::Arc::new(std::sync::Mutex::new(Some(jobs)));
        let work = jobs.clone();
        let outcome = std::sync::Arc::new(std::sync::Mutex::new(None));
        let result = outcome.clone();
        let wake = ctx.clone();
        let mut worker = Threads::default();
        let spawned = worker.spawn("paintcraft-update-drain", move || {
            let jobs = work
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take()
                .unwrap_or_default();
            let value = run_all(jobs);
            *result.lock().unwrap_or_else(|p| p.into_inner()) = Some(value);
            wake.request_repaint();
        });
        if spawned.is_err() {
            // A failed spawn must not drop unretired owners. Rare synchronous
            // cleanup blocks safely; installation still refuses afterward.
            let jobs = jobs
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take()
                .unwrap_or_default();
            let _ = run_all(jobs);
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "could not start shutdown worker",
            ));
        }
        Ok(Self { worker, outcome })
    }
    pub(super) fn poll(&mut self) -> Option<Result<()>> {
        if self.worker.busy() {
            return None;
        }
        if let Err(error) = self.worker.take().shutdown() {
            return Some(Err(error));
        }
        Some(
            self.outcome
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take()
                .unwrap_or_else(|| {
                    Err(Error::new(
                        ErrorCode::InvariantViolation,
                        "shutdown worker lost its outcome",
                    ))
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};
    const DEADLINE: Duration = Duration::from_secs(5);
    struct Retained(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for Retained {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    #[test]
    fn cancelled_old_tail_and_immediate_retry_are_both_retained_until_join() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };
        let mut threads = Threads::default();
        let drops = Arc::new(AtomicUsize::new(0));
        let cancelled = Arc::new(AtomicBool::new(false));
        let observed = Arc::new(AtomicBool::new(false));
        let (old_started, old_ready) = mpsc::channel();
        let (old_release, old_blocked) = mpsc::channel();
        let (old_done, old_retired) = mpsc::channel();
        let first = Retained(drops.clone());
        let flag = cancelled.clone();
        let seen = observed.clone();
        threads
            .spawn("test-old-cancelled-tail", move || {
                let capture = first;
                old_started.send(()).unwrap();
                old_blocked.recv_timeout(DEADLINE).unwrap();
                seen.store(flag.load(Ordering::SeqCst), Ordering::SeqCst);
                drop(capture);
                old_done.send(()).unwrap();
            })
            .unwrap();
        old_ready.recv_timeout(DEADLINE).unwrap();
        let old_identity = threads.workers[0].thread().id();
        cancelled.store(true, Ordering::SeqCst);
        let (new_started, new_ready) = mpsc::channel();
        let (new_release, new_blocked) = mpsc::channel();
        let second = Retained(drops.clone());
        threads
            .spawn("test-authorized-retry", move || {
                let _capture = second;
                new_started.send(()).unwrap();
                new_blocked.recv_timeout(DEADLINE).unwrap();
            })
            .unwrap();
        new_ready.recv_timeout(DEADLINE).unwrap();
        assert_eq!(threads.workers.len(), 2);
        assert!(
            threads
                .workers
                .iter()
                .any(|worker| worker.thread().id() == old_identity)
        );
        assert!(threads.busy());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let (done, ended) = mpsc::channel();
        let waiter = std::thread::spawn(move || done.send(threads.shutdown()).unwrap());
        old_release.send(()).unwrap();
        old_retired.recv_timeout(DEADLINE).unwrap();
        assert!(observed.load(Ordering::SeqCst));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(matches!(ended.try_recv(), Err(mpsc::TryRecvError::Empty)));
        new_release.send(()).unwrap();
        ended.recv_timeout(DEADLINE).unwrap().unwrap();
        waiter.join().unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn panicking_worker_does_not_skip_joining_another_retained_callback() {
        let mut threads = Threads::default();
        threads
            .spawn("test-first-panic", || panic!("test-only tracked panic"))
            .unwrap();
        let (started, ready) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        threads
            .spawn("test-later-blocked", move || {
                started.send(()).unwrap();
                blocked.recv_timeout(DEADLINE).unwrap();
            })
            .unwrap();
        ready.recv_timeout(DEADLINE).unwrap();
        let (done, result) = mpsc::channel();
        let waiter = std::thread::spawn(move || done.send(threads.shutdown()).unwrap());
        assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
        release.send(()).unwrap();
        assert_eq!(
            result.recv_timeout(DEADLINE).unwrap().unwrap_err().code,
            ErrorCode::InvariantViolation
        );
        waiter.join().unwrap();
    }
    #[test]
    fn own_worker_shutdown_refuses_instead_of_attempting_self_join() {
        let mut threads = Threads::default();
        let (send, receive) = mpsc::channel::<Threads>();
        let (done, result) = mpsc::channel();
        threads
            .spawn("test-self-shutdown", move || {
                let owner = receive.recv_timeout(DEADLINE).unwrap();
                done.send(owner.shutdown()).unwrap();
            })
            .unwrap();
        send.send(threads).unwrap();
        assert_eq!(
            result.recv_timeout(DEADLINE).unwrap().unwrap_err().code,
            ErrorCode::InvariantViolation
        );
        // This refusal is not a successful retirement ACK. Production invokes
        // shutdown only on the distinct update drain worker.
    }
    #[test]
    fn caught_panic_stays_failed_for_shutdown_without_blocking_later_admission() {
        let mut threads = Threads::default();
        threads
            .spawn("test-worker-panic", || panic!("test-only worker panic"))
            .unwrap();
        while threads.busy() {
            std::thread::yield_now();
        }
        threads.spawn("test-recovery", || {}).unwrap();
        assert_eq!(
            threads.shutdown().unwrap_err().code,
            ErrorCode::InvariantViolation
        );
    }
    #[test]
    fn drain_continues_after_error_and_panic_before_reporting_failure() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let retired = Arc::new(AtomicUsize::new(0));
        let first = retired.clone();
        let last = retired.clone();
        let jobs: Vec<Shutdown> = vec![
            Box::new(move || {
                first.fetch_add(1, Ordering::SeqCst);
                Err(Error::new(ErrorCode::StorageFailed, "test refusal"))
            }),
            Box::new(|| panic!("test-only shutdown panic")),
            Box::new(move || {
                last.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        ];
        assert_eq!(run_all(jobs).unwrap_err().code, ErrorCode::StorageFailed);
        assert_eq!(retired.load(Ordering::SeqCst), 2);
    }
}

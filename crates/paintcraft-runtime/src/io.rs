//! One admitted FIFO actor for native blocking I/O. It serializes every accepted
//! job, including aliases of the same destination. Results retain their admission
//! until consumed. Shutdown drains accepted work rather than cancelling writes.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    thread,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ticket {
    owner: Uuid,
    sequence: u64,
}
impl Ticket {
    pub fn sequence(self) -> u64 {
        self.sequence
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub pending_jobs: usize,
    /// Snapshot retention plus scratch as conservatively charged by the consumer.
    pub retained_bytes: u64,
}
type Work<T> = Box<dyn FnOnce() -> Result<T> + Send>;
struct Request<T> {
    ticket: Ticket,
    work: Work<T>,
    wake: Box<dyn FnOnce() + Send>,
}
#[derive(Debug)]
pub struct OrderedIo<T> {
    owner: Uuid,
    next_sequence: u64,
    limits: Limits,
    pending: HashMap<Ticket, u64>,
    retained_bytes: u64,
    requests: Option<tokio::sync::mpsc::Sender<Request<T>>>,
    completed: mpsc::Receiver<(Ticket, Result<T>)>,
    worker: Option<thread::JoinHandle<()>>,
}
fn unavailable(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
impl<T: Send + 'static> OrderedIo<T> {
    pub fn new(limits: Limits) -> Result<Self> {
        if limits.pending_jobs == 0 || limits.retained_bytes == 0 {
            return Err(unavailable(
                "I/O queue and retention limits must be positive",
            ));
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .thread_name("paintcraft-io-worker")
            .build()
            .map_err(|e| unavailable(format!("cannot start I/O runtime: {e}")))?;
        let (requests, mut receiver) =
            tokio::sync::mpsc::channel::<Request<T>>(limits.pending_jobs);
        let (sender, completed) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("paintcraft-io".into())
            .spawn(move || {
                runtime.block_on(async move {
                    while let Some(request) = receiver.recv().await {
                        let result = tokio::task::spawn_blocking(request.work)
                            .await
                            .unwrap_or_else(|_| {
                                Err(Error::new(
                                    ErrorCode::StorageFailed,
                                    "I/O preparation panicked",
                                ))
                            });
                        let _ = sender.send((request.ticket, result));
                        let _ = catch_unwind(AssertUnwindSafe(request.wake));
                    }
                });
            })
            .map_err(|e| unavailable(format!("cannot start I/O coordinator: {e}")))?;
        Ok(Self {
            owner: Uuid::new_v4(),
            next_sequence: 1,
            limits,
            pending: HashMap::new(),
            retained_bytes: 0,
            requests: Some(requests),
            completed,
            worker: Some(worker),
        })
    }
    pub fn admit(&self, jobs: usize, bytes: u64) -> Result<()> {
        if jobs > self.limits.pending_jobs.saturating_sub(self.pending.len())
            || bytes
                > self
                    .limits
                    .retained_bytes
                    .saturating_sub(self.retained_bytes)
        {
            Err(unavailable("native I/O admission exhausted"))
        } else {
            Ok(())
        }
    }
    pub fn submit(
        &mut self,
        bytes: u64,
        work: impl FnOnce() -> Result<T> + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<Ticket> {
        self.admit(1, bytes)?;
        let sequence = self.next_sequence;
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or_else(|| unavailable("I/O sequence exhausted"))?;
        let ticket = Ticket {
            owner: self.owner,
            sequence,
        };
        let request = Request {
            ticket,
            work: Box::new(work),
            wake: Box::new(wake),
        };
        self.requests
            .as_ref()
            .expect("live queue")
            .try_send(request)
            .map_err(|error| match error {
                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    unavailable("native I/O queue unavailable")
                }
                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    Error::new(ErrorCode::StorageFailed, "native I/O service stopped")
                }
            })?;
        self.pending.insert(ticket, bytes);
        self.retained_bytes += bytes;
        Ok(ticket)
    }
    pub fn poll(&mut self) -> Option<(Ticket, Result<T>)> {
        let (ticket, result) = self.completed.try_recv().ok()?;
        self.retained_bytes -= self.pending.remove(&ticket).expect("admitted completion");
        Some((ticket, result))
    }
    pub fn pending_jobs(&self) -> usize {
        self.pending.len()
    }
    pub fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }
}
impl<T> Drop for OrderedIo<T> {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    #[test]
    fn fifo_backpressure_and_completion_retention() {
        let mut io = OrderedIo::new(Limits {
            pending_jobs: 2,
            retained_bytes: 12,
        })
        .unwrap();
        let order = Arc::new(Mutex::new(Vec::new()));
        let (release, hold) = mpsc::channel();
        let (wake, awake) = mpsc::channel();
        let ordered = order.clone();
        let first = io
            .submit(
                6,
                move || {
                    hold.recv().unwrap();
                    ordered.lock().unwrap().push(1);
                    Ok(1)
                },
                {
                    let wake = wake.clone();
                    move || wake.send(()).unwrap()
                },
            )
            .unwrap();
        let ordered = order.clone();
        let second = io
            .submit(
                6,
                move || {
                    ordered.lock().unwrap().push(2);
                    Ok(2)
                },
                move || wake.send(()).unwrap(),
            )
            .unwrap();
        assert!(second.sequence() > first.sequence());
        assert!(io.submit(1, || Ok(3), || {}).is_err());
        release.send(()).unwrap();
        awake.recv_timeout(Duration::from_secs(5)).unwrap();
        awake.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(*order.lock().unwrap(), [1, 2]);
        assert_eq!(
            io.retained_bytes(),
            12,
            "completed but unconsumed work stays charged"
        );
        assert_eq!(io.poll().map(|(id, r)| (id, r.unwrap())), Some((first, 1)));
        assert_eq!(io.poll().map(|(id, r)| (id, r.unwrap())), Some((second, 2)));
        assert_eq!(io.retained_bytes(), 0);
    }
    #[test]
    fn task_and_wake_panics_do_not_break_the_queue_and_drop_drains() {
        let mut io = OrderedIo::<()>::new(Limits {
            pending_jobs: 2,
            retained_bytes: 12,
        })
        .unwrap();
        let (wake, awake) = mpsc::channel();
        io.submit(
            1,
            || panic!("injected task panic"),
            move || {
                wake.send(()).unwrap();
                panic!("injected wake panic")
            },
        )
        .unwrap();
        awake.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            io.poll().unwrap().1.unwrap_err().code,
            ErrorCode::StorageFailed
        );
        let (done, received) = mpsc::channel();
        io.submit(
            1,
            move || {
                done.send(()).unwrap();
                Ok(())
            },
            || {},
        )
        .unwrap();
        drop(io);
        received.recv_timeout(Duration::from_secs(5)).unwrap();
    }
}

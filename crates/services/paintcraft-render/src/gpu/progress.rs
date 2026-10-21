//! Native completion observation independent of canvas repainting.
use super::*;
use std::sync::{
    Arc, Condvar, Weak,
    atomic::{AtomicUsize, Ordering},
};

static WORKERS: AtomicUsize = AtomicUsize::new(0);
const WORKER_LIMIT: usize = 16;
struct Slot(&'static AtomicUsize);
impl Slot {
    fn acquire(counter: &'static AtomicUsize, limit: usize) -> Result<Self> {
        counter
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < limit).then_some(n + 1)
            })
            .map(|_| Self(counter))
            .map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "GPU completion observer limit reached",
                )
            })
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Default)]
struct Status {
    pending: usize,
    closed: bool,
    polls: u64,
}
pub(super) struct State {
    identity: Arc<()>,
    status: Mutex<Status>,
    changed: Condvar,
}
struct Owner(Arc<State>);
impl Drop for Owner {
    fn drop(&mut self) {
        self.0
            .status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .closed = true;
        self.0.changed.notify_all();
    }
}
/// Owner-scoped native observer. Drop signals shutdown without joining on the UI.
/// The process admits at most 16 observer workers; this is not a physical budget.
#[derive(Clone)]
pub struct CompletionDriver(Arc<Owner>);
impl std::fmt::Debug for CompletionDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CompletionDriver")
            .field(&self.progress())
            .finish()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletionProgress {
    pub pending: usize,
    pub polls: u64,
    pub closed: bool,
}
impl CompletionDriver {
    pub fn new(gpu: &GpuRenderer) -> Result<Self> {
        let slot = Slot::acquire(&WORKERS, WORKER_LIMIT)?;
        let state = Arc::new(State {
            identity: gpu.submission_identity.clone(),
            status: Mutex::new(Status::default()),
            changed: Condvar::new(),
        });
        let worker_state = state.clone();
        let device = gpu.device.clone();
        std::thread::Builder::new()
            .name("paintcraft-gpu-completion".into())
            .spawn(move || {
                let _slot = slot;
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    observe(&device, &worker_state)
                }));
                worker_state
                    .status
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .closed = true;
                worker_state.changed.notify_all();
            })
            .map_err(|error| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    format!("cannot start GPU completion observer: {error}"),
                )
            })?;
        Ok(Self(Arc::new(Owner(state))))
    }
    pub fn progress(&self) -> CompletionProgress {
        let state = self.0.0.status.lock().unwrap_or_else(|p| p.into_inner());
        CompletionProgress {
            pending: state.pending,
            polls: state.polls,
            closed: state.closed,
        }
    }
    pub(super) fn weak(&self) -> Weak<State> {
        Arc::downgrade(&self.0.0)
    }
}
pub(super) struct Ticket(Weak<State>);
impl State {
    pub(super) fn track(self: &Arc<Self>, identity: &Arc<()>) -> Option<Ticket> {
        if !Arc::ptr_eq(&self.identity, identity) {
            return None;
        }
        let mut state = self.status.lock().unwrap_or_else(|p| p.into_inner());
        if state.closed {
            return None;
        }
        state.pending = state.pending.checked_add(1)?;
        drop(state);
        self.changed.notify_one();
        Some(Ticket(Arc::downgrade(self)))
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        if let Some(state) = self.0.upgrade() {
            state
                .status
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .pending -= 1;
            state.changed.notify_all();
        }
    }
}
fn observe(device: &wgpu::Device, state: &State) {
    loop {
        let mut status = state.status.lock().unwrap_or_else(|p| p.into_inner());
        while !status.closed && status.pending == 0 {
            status = state
                .changed
                .wait(status)
                .unwrap_or_else(|p| p.into_inner());
        }
        if status.closed {
            return;
        }
        status.polls = status.polls.saturating_add(1);
        drop(status);
        if device.poll(wgpu::PollType::Poll).is_err() {
            return;
        }
        let status = state.status.lock().unwrap_or_else(|p| p.into_inner());
        if !status.closed && status.pending != 0 {
            drop(
                state
                    .changed
                    .wait_timeout(status, Duration::from_millis(16))
                    .unwrap_or_else(|p| p.into_inner()),
            );
        }
    }
}

#[cfg(test)]
mod limits {
    use super::*;
    #[test]
    fn observer_slots_are_bounded_and_return_on_drop() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let first = Slot::acquire(&COUNT, 2).unwrap();
        let second = Slot::acquire(&COUNT, 2).unwrap();
        assert!(Slot::acquire(&COUNT, 2).is_err());
        drop(first);
        let replacement = Slot::acquire(&COUNT, 2).unwrap();
        drop(second);
        drop(replacement);
        assert_eq!(COUNT.load(Ordering::Acquire), 0);
    }
}

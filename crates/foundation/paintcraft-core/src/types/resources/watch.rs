//! Weak registrations, bounded fanout and one wake per consumer observation.
use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Weak,
        atomic::{AtomicBool, Ordering},
    },
};
const MAX_WATCHERS: usize = 64;
pub(super) struct Observer {
    budget: SharedBudget,
    headroom: u64,
    pending: AtomicBool,
    waiting: AtomicBool,
    wake: Box<dyn Fn() + Send + Sync>,
}
impl std::fmt::Debug for Observer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BudgetObserver")
            .field("headroom", &self.headroom)
            .finish_non_exhaustive()
    }
}
#[must_use = "retain the watcher while wake notifications are needed"]
#[derive(Debug)]
pub struct BudgetWatch(Arc<Observer>);
impl BudgetWatch {
    /// Acknowledge coalesced notifications, then sample the live counters.
    pub fn observe(&self) -> BudgetUsage {
        self.0.pending.store(false, Ordering::Release);
        self.0.budget.usage()
    }
    /// Include releases above the pressure threshold while a consumer is waiting.
    pub fn wait_for_release(&self, waiting: bool) {
        self.0.waiting.store(waiting, Ordering::Release);
    }
}
impl SharedBudget {
    /// Register a wake-only callback. Registrations are weak and capped at 64.
    /// Callbacks run outside the counter lock and should enqueue a consumer poll.
    pub fn watch(
        &self,
        headroom: u64,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<BudgetWatch> {
        if headroom == 0 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "budget watch headroom must be positive",
            ));
        }
        let observer = Arc::new(Observer {
            budget: self.clone(),
            headroom,
            pending: AtomicBool::new(false),
            waiting: AtomicBool::new(false),
            wake: Box::new(wake),
        });
        let free = {
            let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
            state.observers.retain(|weak| weak.strong_count() != 0);
            if state.observers.len() >= MAX_WATCHERS {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "byte budget watcher limit reached",
                ));
            }
            state.observers.try_reserve(1).map_err(|_| {
                Error::new(ErrorCode::ResourceLimit, "cannot register budget watcher")
            })?;
            state.observers.push(Arc::downgrade(&observer));
            state.limit - state.reserved
        };
        if free < headroom {
            signal(&observer);
        }
        Ok(BudgetWatch(observer))
    }
}
fn signal(observer: &Observer) {
    if !observer.pending.swap(true, Ordering::AcqRel) {
        let _ = catch_unwind(AssertUnwindSafe(|| (observer.wake)()));
    }
}
// The fixed array allocates no heap while releasing a resource. Every upgraded
// observer is dropped after unlocking, including irrelevant/coalesced observers:
// arbitrary callback captures may have reentrant destructors.
pub(super) struct Wakes {
    observers: [Option<Arc<Observer>>; MAX_WATCHERS],
    before: u64,
    after: u64,
    denied: bool,
    released: bool,
}
pub(super) type Registrations = Vec<Weak<Observer>>;
pub(super) fn collect(
    state: &mut State,
    before: u64,
    denied: bool,
    released: bool,
) -> Option<Wakes> {
    if state.observers.is_empty() {
        return None;
    }
    state.observers.retain(|weak| weak.strong_count() != 0);
    let mut observers = std::array::from_fn(|_| None);
    for (slot, weak) in observers.iter_mut().zip(&state.observers) {
        *slot = weak.upgrade();
    }
    Some(Wakes {
        observers,
        before,
        after: state.limit - state.reserved,
        denied,
        released,
    })
}
pub(super) fn notify(wakes: Option<Wakes>) {
    let Some(wakes) = wakes else { return };
    for observer in wakes.observers.into_iter().flatten() {
        let entered = wakes.before >= observer.headroom && wakes.after < observer.headroom;
        let released = wakes.released
            && (wakes.before < observer.headroom || observer.waiting.load(Ordering::Acquire));
        if entered || released || wakes.denied {
            signal(&observer);
        }
    }
}

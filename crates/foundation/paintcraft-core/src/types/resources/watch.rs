//! Weak registrations, bounded fanout and one wake per consumer observation.
use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
const MAX_WATCHERS: usize = 64;
pub(super) struct Observer {
    budget: SharedBudget,
    headroom: u64,
    pending: AtomicBool,
    waiting: AtomicBool,
    requested: AtomicU64,
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
/// Advisory shared demand; no capacity is reserved by this observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetPressure {
    pub usage: BudgetUsage,
    pub requested_capacity: u64,
}
impl Drop for BudgetWatch {
    fn drop(&mut self) {
        // Collected wakes may temporarily retain the observer after unregistering.
        self.0.requested.store(0, Ordering::Release);
    }
}
impl BudgetWatch {
    /// Publish one known feasible demand in this exact realm, or clear it.
    /// Demand is bounded by the existing weak watcher registration and owns no bytes.
    pub fn wait_for_capacity(&self, denial: Option<&AllocationDenial>) {
        let requested = denial
            .filter(|denial| denial.is_for(&self.0.budget) && denial.is_possible())
            .map_or(0, AllocationDenial::required_bytes);
        if self.0.requested.swap(requested, Ordering::AcqRel) == requested {
            return;
        }
        let wakes = {
            let mut state = self
                .0
                .budget
                .0
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let free = state.limit - state.reserved;
            collect(&mut state, free, true, false)
        };
        notify(wakes);
    }
    /// Observe counters and the maximum known demand across live consumers.
    /// Concurrent changes can supersede this snapshot; actual allocation still reserves.
    pub fn observe_pressure(&self) -> BudgetPressure {
        self.0.pending.store(false, Ordering::Release);
        let (usage, observers) = {
            let mut state = self
                .0
                .budget
                .0
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let usage = BudgetUsage {
                limit: state.limit,
                reserved: state.reserved,
                peak: state.peak,
            };
            let observers = collect(&mut state, usage.limit - usage.reserved, false, false);
            (usage, observers)
        };
        let requested_capacity = observers.as_ref().map_or(0, requested_capacity);
        BudgetPressure {
            usage,
            requested_capacity,
        }
    }

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
            requested: AtomicU64::new(0),
            wake: Box::new(wake),
        });
        {
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
        }
        let watch = BudgetWatch(observer);
        let pressure = watch.observe_pressure();
        if pressure.usage.limit - pressure.usage.reserved
            < headroom.max(pressure.requested_capacity)
        {
            signal(&watch.0);
        }
        Ok(watch)
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
fn requested_capacity(wakes: &Wakes) -> u64 {
    wakes
        .observers
        .iter()
        .flatten()
        .map(|observer| observer.requested.load(Ordering::Acquire))
        .max()
        .unwrap_or(0)
}
pub(super) fn notify(wakes: Option<Wakes>) {
    let Some(wakes) = wakes else { return };
    let requested = requested_capacity(&wakes);
    for observer in wakes.observers.into_iter().flatten() {
        let headroom = observer.headroom.max(requested);
        let entered = wakes.before >= headroom && wakes.after < headroom;
        let released =
            wakes.released && (wakes.before < headroom || observer.waiting.load(Ordering::Acquire));
        if entered || released || wakes.denied {
            signal(&observer);
        }
    }
}

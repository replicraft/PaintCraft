//! Unique resizable declarations; storage owners release bytes after storage drops.
use super::*;

/// One movable, non-clonable resource declaration. Sharing a storage owner may
/// share this account; copying its allocation requires a separate account.
#[derive(Debug)]
pub struct BudgetAccount {
    budget: SharedBudget,
    class: AllocationClass,
    bytes: u64,
    scope: Option<Arc<Mutex<ScopeFootprint>>>,
    reservation: Option<std::sync::Weak<reservation::Reserved>>,
}
/// Attempt-local declarations, including the largest observed pending request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScopeFootprint {
    pub live: u64,
    pub peak: u64,
}
/// Clone this factory to group independently owned accounts in one attempt.
/// Ordinary scopes own no capacity. Reserved scopes retain unused capacity until
/// their last factory drops; surviving accounts retain only their own bytes.
#[derive(Clone, Debug)]
pub struct BudgetScope {
    budget: SharedBudget,
    class: AllocationClass,
    state: Arc<Mutex<ScopeFootprint>>,
    reservation: Option<BudgetReservation>,
}
impl BudgetScope {
    pub fn account(&self) -> BudgetAccount {
        BudgetAccount {
            budget: self.budget.clone(),
            class: self.class,
            bytes: 0,
            scope: Some(self.state.clone()),
            reservation: self.reservation.as_ref().map(BudgetReservation::downgrade),
        }
    }
    pub fn budget(&self) -> SharedBudget {
        self.budget.clone()
    }
    pub fn footprint(&self) -> ScopeFootprint {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}
impl SharedBudget {
    pub fn scope(&self, class: AllocationClass) -> BudgetScope {
        BudgetScope {
            budget: self.clone(),
            class,
            state: Arc::new(Mutex::new(ScopeFootprint::default())),
            reservation: None,
        }
    }
    pub fn account(&self, class: AllocationClass) -> BudgetAccount {
        BudgetAccount {
            budget: self.clone(),
            class,
            bytes: 0,
            scope: None,
            reservation: None,
        }
    }
}
impl BudgetReservation {
    /// Transfer capacity into resizable, independently owned declarations. While
    /// a factory survives, accounts cannot borrow beyond the reservation. Account
    /// shrink/drop recycles capacity; after factories drop, it releases globally.
    pub fn scope(self, class: AllocationClass) -> BudgetScope {
        BudgetScope {
            budget: self.budget(),
            class,
            state: Arc::new(Mutex::new(ScopeFootprint::default())),
            reservation: Some(self),
        }
    }
}
impl BudgetAccount {
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn budget(&self) -> SharedBudget {
        self.budget.clone()
    }
    /// Create an empty declaration in this account's realm and attempt scope.
    pub fn sibling(&self) -> Self {
        Self {
            budget: self.budget.clone(),
            class: self.class,
            bytes: 0,
            scope: self.scope.clone(),
            reservation: self.reservation.clone(),
        }
    }
    /// Grow atomically before allocating, or shrink after freeing storage. A
    /// rejected growth keeps the old charge. Release callbacks run outside locks.
    pub fn resize(&mut self, bytes: u64) -> Result<()> {
        if bytes > self.bytes {
            let delta = bytes - self.bytes;
            let required = if let Some(scope) = &self.scope {
                let mut scope = scope.lock().unwrap_or_else(|e| e.into_inner());
                scope.live = scope.live.checked_add(delta).ok_or_else(|| {
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "resource scope footprint overflow",
                    )
                })?;
                scope.peak = scope.peak.max(scope.live);
                scope.peak
            } else {
                bytes
            };
            // No scope lock survives budget callbacks. Failure rolls back the
            // pending live declaration, while preserving its known demand peak.
            let result = match self.reservation.as_ref().and_then(std::sync::Weak::upgrade) {
                Some(reservation) => reservation.take(delta),
                None => self.budget.admit(delta),
            };
            if let Err(error) = result {
                self.release_scope(delta);
                return Err(error.with_allocation_requirement(self.class, required));
            }
            self.bytes = bytes;
        } else if bytes < self.bytes {
            let released = self.bytes - bytes;
            self.bytes = bytes;
            self.release_scope(released);
            self.release_capacity(released);
        }
        Ok(())
    }
    fn release_scope(&self, bytes: u64) {
        if let Some(scope) = &self.scope {
            scope.lock().unwrap_or_else(|e| e.into_inner()).live -= bytes;
        }
    }
    fn release_capacity(&self, bytes: u64) {
        if let Some(reservation) = self.reservation.as_ref().and_then(std::sync::Weak::upgrade) {
            reservation.recycle(bytes);
        } else {
            self.budget.release(bytes);
        }
    }
}
impl Drop for BudgetAccount {
    fn drop(&mut self) {
        if self.bytes != 0 {
            self.release_scope(self.bytes);
            self.release_capacity(self.bytes);
        }
    }
}

#[cfg(test)]
#[path = "reserved_account_tests.rs"]
mod reserved_account_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_growth_is_atomic_and_classified_independently_of_decoded_tiles() {
        let budget = SharedBudget::new(16);
        let mut account = budget.account(AllocationClass::PaintMetadata);
        account.resize(8).unwrap();
        let blocker = budget.reserve(8).unwrap();
        let error = account.resize(12).unwrap_err();
        let denial = error.allocation_denial().unwrap();
        assert_eq!(account.bytes(), 8);
        assert_eq!(budget.usage().reserved, 16);
        assert_eq!(denial.class(), AllocationClass::PaintMetadata);
        assert_eq!(denial.required_bytes(), 12);
        assert!(denial.is_possible());
        account.resize(8).unwrap();
        drop(blocker);
        account.resize(12).unwrap();
        account.resize(3).unwrap();
        assert_eq!(budget.usage().reserved, 3);
        drop(account);
        assert_eq!(budget.usage().reserved, 0);
        assert!(denial.can_retry());
    }
    #[test]
    fn independently_owned_accounts_compete_in_one_atomic_realm() {
        use std::{sync::mpsc, time::Duration};
        let budget = SharedBudget::new(16);
        let (send, receive) = mpsc::channel();
        let mut joins = Vec::new();
        let mut releases = Vec::new();
        for _ in 0..2 {
            let budget = budget.clone();
            let send = send.clone();
            let (release, wait) = mpsc::channel();
            releases.push(release);
            joins.push(std::thread::spawn(move || {
                let mut account = budget.account(AllocationClass::PaintMetadata);
                send.send(account.resize(12).is_ok()).unwrap();
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
            }));
        }
        let successes = (0..2)
            .filter(|_| receive.recv_timeout(Duration::from_secs(5)).unwrap())
            .count();
        assert_eq!(successes, 1);
        assert_eq!(budget.usage().reserved, 12);
        for release in releases {
            release.send(()).unwrap();
        }
        for join in joins {
            join.join().unwrap();
        }
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn storage_drops_before_reentrant_release_notifications() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Owner {
            _storage: Arc<Vec<u8>>,
            _charge: BudgetAccount,
        }
        let budget = SharedBudget::new(8);
        let mut account = budget.account(AllocationClass::PaintMetadata);
        account.resize(8).unwrap();
        let storage = Arc::new(vec![1; 8]);
        let weak = Arc::downgrade(&storage);
        let released = Arc::new(AtomicBool::new(false));
        let observed = released.clone();
        let probe = budget.clone();
        let watch = budget
            .watch(1, move || {
                if probe.usage().reserved == 0 {
                    observed.store(weak.upgrade().is_none(), Ordering::Release);
                }
            })
            .unwrap();
        watch.observe();
        drop(Owner {
            _storage: storage,
            _charge: account,
        });
        assert!(released.load(Ordering::Acquire));
        assert_eq!(budget.usage().reserved, 0);
    }
}

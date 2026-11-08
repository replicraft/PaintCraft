use super::owner::{SharedOwner, arc_bytes};
use super::*;
use crate::spatial::WorkspaceAdmission;
use paintcraft_core::types::resources::{BudgetAccount, BudgetScope};
use std::sync::Mutex;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseJobPolicy {
    BrowserV1,
    NativeV1,
}
impl NoiseJobPolicy {
    /// Pure authority ceiling for endpoint-owned negotiation; no counter/account.
    pub const fn max_limits(self) -> NoiseLimits {
        NoiseLimits {
            bytes: BYTE_MAX,
            base_work: match self {
                Self::BrowserV1 => 268_435_456,
                Self::NativeV1 => 1_000_000_000,
            },
            noise_work: NOISE_WORK_MAX,
        }
    }
    /// Endpoint selects policy internally. Never accept NativeV1 from browser wire.
    pub fn validate(self, requested: NoiseLimits) -> Result<()> {
        requested.checked_total()?;
        let ceiling = self.max_limits();
        if requested.bytes > ceiling.bytes
            || requested.base_work > ceiling.base_work
            || requested.noise_work > ceiling.noise_work
        {
            return Err(limit());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseLimits {
    pub bytes: u64,
    pub base_work: u64,
    pub noise_work: u64,
}
impl NoiseLimits {
    pub fn checked_total(self) -> Result<u64> {
        self.base_work
            .checked_add(self.noise_work)
            .ok_or_else(limit)
    }
}
/// Same-target logical sizes; caller retained structs additionally price handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseControlQuote {
    /// Both private shared control allocations, including logical Arc headers.
    pub counter_bytes: u64,
    /// Full retained factory handle, including BOTH shared-owner pointers.
    pub handle_bytes: u64,
}
#[derive(Debug)]
struct Counter {
    used: Mutex<u64>,
}
#[derive(Clone, Debug)]
pub struct NoiseAdmission {
    base: WorkspaceAdmission,
    counter: SharedOwner<Counter>,
    // Every clone retains both owners. This field MUST follow counter: the last
    // counter storage drops before the last charge/account can notify reentry.
    _charge: SharedOwner<BudgetAccount>,
    limits: NoiseLimits,
}
impl NoiseAdmission {
    pub fn control_quote() -> Result<NoiseControlQuote> {
        // Same-target logical Arc convention, INCLUDING both owned allocations.
        // Allocator metadata/slack and scalar stack remain explicit exclusions.
        let counter_bytes = arc_bytes::<Counter>()?
            .checked_add(arc_bytes::<BudgetAccount>()?)
            .ok_or_else(limit)?;
        Ok(NoiseControlQuote {
            counter_bytes,
            handle_bytes: std::mem::size_of::<Self>() as u64,
        })
    }
    /// ONLY an explicit new root may mint the paired counter; never an upgrade.
    /// The supplied scope is the process/job reservation, never a new byte pool.
    pub fn new_root(
        scope: BudgetScope,
        policy: NoiseJobPolicy,
        requested: NoiseLimits,
    ) -> Result<Self> {
        if crate::spatial::current().is_some() || super::context::current().is_some() {
            return Err(missing());
        }
        policy.validate(requested)?;
        let limits = requested;
        let bytes = Self::control_quote()?.counter_bytes;
        if bytes > limits.bytes {
            return Err(limit());
        }
        let mut charge = scope.account();
        charge.resize(bytes)?;
        // Charge new sidecar storage BEFORE either sidecar allocation or publication.
        // Legacy base factory bookkeeping keeps its existing documented exclusion.
        let base = WorkspaceAdmission::new(scope, limits.bytes, limits.base_work);
        Ok(Self {
            base,
            counter: SharedOwner::new(Counter {
                used: Mutex::new(0),
            }),
            _charge: SharedOwner::new(charge),
            limits,
        })
    }
    pub fn limits(&self) -> NoiseLimits {
        self.limits
    }
    /// Borrowing the legacy base does not grant another Noise counter.
    pub fn workspace(&self) -> &WorkspaceAdmission {
        &self.base
    }
    pub fn same_job(&self, other: &Self) -> bool {
        self.counter.same(&other.counter)
            && self._charge.same(&other._charge)
            && self.base.same_work_owner(&other.base)
    }
    pub fn child(&self, requested: NoiseLimits) -> Result<Self> {
        requested.checked_total()?;
        let limits = NoiseLimits {
            bytes: self.limits.bytes.min(requested.bytes),
            base_work: self.limits.base_work.min(requested.base_work),
            noise_work: self.limits.noise_work.min(requested.noise_work),
        };
        let value = Self {
            base: self.base.tightened(limits.bytes, limits.base_work)?,
            counter: self.counter.clone(),
            _charge: self._charge.clone(),
            limits,
        };
        value.check_available(WorkComponents::default())?;
        Ok(value)
    }
    pub(crate) fn for_workspace(&self, base: &WorkspaceAdmission) -> Result<Self> {
        if !self.base.same_work_owner(base) {
            return Err(missing());
        }
        let limits = NoiseLimits {
            bytes: self.limits.bytes.min(base.bytes_limit()),
            base_work: self.limits.base_work.min(base.work_limit()),
            noise_work: self.limits.noise_work,
        };
        let value = Self {
            base: base.tightened(limits.bytes, limits.base_work)?,
            counter: self.counter.clone(),
            _charge: self._charge.clone(),
            limits,
        };
        value.check_available(WorkComponents::default())?;
        Ok(value)
    }
    pub fn used(&self) -> Result<WorkComponents> {
        let noise = self.counter.used.lock().unwrap_or_else(|e| e.into_inner());
        let value = WorkComponents {
            base: self.base.work_used(),
            noise: *noise,
        };
        value.checked_total()?;
        Ok(value)
    }
    fn next(&self, current_noise: u64, delta: WorkComponents) -> Result<WorkComponents> {
        let value = WorkComponents {
            base: self
                .base
                .work_used()
                .checked_add(delta.base)
                .ok_or_else(limit)?,
            noise: current_noise.checked_add(delta.noise).ok_or_else(limit)?,
        };
        value.checked_total()?;
        if value.base > self.limits.base_work || value.noise > self.limits.noise_work {
            return Err(limit());
        }
        Ok(value)
    }
    pub fn check_available(&self, quote: WorkComponents) -> Result<()> {
        // A child cannot claim zero appearance bytes while retaining this new
        // charged sidecar. Other retained frame handles are priced by its owner.
        if Self::control_quote()?.counter_bytes > self.limits.bytes {
            return Err(limit());
        }
        let used = self.counter.used.lock().unwrap_or_else(|e| e.into_inner());
        self.next(*used, quote).map(|_| ())
    }
    /// Spend codec or conversion work on this same paired ledger. Both captured
    /// component ceilings remain effective; this grants no additional Noise work.
    pub fn reserve_base_work(&self, work: u64) -> Result<()> {
        self.reserve_components(WorkComponents {
            base: work,
            noise: 0,
        })
    }
    /// Consume a bounded remote child grant on this SAME paired counter once.
    /// This only debits; it does not construct, refund or reset an authority.
    pub fn reserve_transferred_work(&self, work: WorkComponents) -> Result<()> {
        self.reserve_components(work)
    }
    pub(crate) fn reserve_components(&self, delta: WorkComponents) -> Result<()> {
        // All combined operations lock Noise BEFORE base; never reverse that order.
        let mut noise = self.counter.used.lock().unwrap_or_else(|e| e.into_inner());
        let next = self.next(*noise, delta)?;
        self.base.reserve_work(delta.base)?;
        // No fallible operation follows base debit. A denied pair changes neither.
        *noise = next.noise;
        Ok(())
    }
}

impl NoiseAdmission {
    /// Actual already-retained control declaration, not a new reservation.
    pub fn retained_control_bytes(&self) -> u64 {
        self._charge.bytes()
    }
}

#[cfg(test)]
pub(super) fn counter_payload_bytes() -> u64 {
    std::mem::size_of::<Counter>() as u64
}

#[cfg(test)]
mod source7_transfer_tests {
    use super::*;
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    #[test]
    fn transferred_pair_is_atomic_and_cannot_be_copied_from_remaining_limits() {
        let budget = SharedBudget::new(4096);
        let parent = NoiseAdmission::new_root(
            budget.scope(AllocationClass::SharedBudget),
            NoiseJobPolicy::BrowserV1,
            NoiseLimits {
                bytes: 4096,
                base_work: 11,
                noise_work: 17,
            },
        )
        .unwrap();
        parent.reserve_base_work(3).unwrap();
        let copy = parent.clone();
        parent
            .reserve_transferred_work(WorkComponents { base: 8, noise: 17 })
            .unwrap();
        assert_eq!(
            parent.used().unwrap(),
            WorkComponents {
                base: 11,
                noise: 17
            }
        );
        assert!(
            copy.reserve_transferred_work(WorkComponents { base: 1, noise: 0 })
                .is_err()
        );
        assert!(
            copy.reserve_transferred_work(WorkComponents { base: 0, noise: 1 })
                .is_err()
        );
        assert_eq!(
            parent.used().unwrap(),
            WorkComponents {
                base: 11,
                noise: 17
            }
        );
    }
}

//! Private per-contact authority. A completed contact is not a physical hold.
use super::*;
use paintcraft_input::tablet::Identity;
use std::sync::atomic::{AtomicBool, Ordering};
const LIMIT: usize = 32;
const CELL_BYTES: usize =
    std::mem::size_of::<Cell>() + 2 * std::mem::size_of::<usize>() + std::mem::align_of::<Cell>()
        - 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Ink,
    Ui,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub(super) installation: u64,
    pub(super) identity: Identity,
    pub(super) begin: u64,
    pub(super) target: Target,
}
#[derive(Debug)]
struct Cell {
    valid: AtomicBool,
    _storage: BudgetAccount,
}
#[derive(Clone, Debug)]
pub(super) struct Lease {
    pub(super) key: Key,
    cell: Arc<Cell>,
}
impl Lease {
    pub(super) fn current(&self) -> bool {
        self.cell.valid.load(Ordering::Acquire)
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        self.key == other.key && Arc::ptr_eq(&self.cell, &other.cell)
    }
}
#[derive(Debug)]
struct Slot {
    cell: Arc<Cell>,
    key: Option<Key>,
    active: bool,
    owner: Option<Owner>,
}
#[derive(Debug)]
pub(super) struct Bank {
    slots: [Slot; LIMIT],
}
impl Bank {
    pub(super) fn new(budget: &SharedBudget) -> Result<Self> {
        // Reserve the complete declaration before constructing any cells. Each
        // cell owns its account, including after the bank itself is retired.
        let scope = budget
            .reserve_capacity((LIMIT * CELL_BYTES) as u64)?
            .scope(AllocationClass::PaintMetadata);
        let mut error = None;
        let slots: [Option<Slot>; LIMIT] = std::array::from_fn(|_| {
            let mut storage = scope.account();
            if let Err(e) = storage.resize(CELL_BYTES as u64) {
                error = Some(e);
                return None;
            }
            Some(Slot {
                cell: Arc::new(Cell {
                    valid: AtomicBool::new(false),
                    _storage: storage,
                }),
                key: None,
                active: false,
                owner: None,
            })
        });
        if let Some(error) = error {
            return Err(error);
        }
        Ok(Self {
            slots: slots.map(|slot| slot.expect("complete token admission")),
        })
    }
    pub(super) fn begin(&mut self, key: Key) -> std::result::Result<Lease, Invalid> {
        if matches!(key.installation, 0 | u64::MAX)
            || key.begin == u64::MAX
            || !key.identity.kind.is_pen()
        {
            return Err(Invalid::Owner);
        }
        if self.slots.iter().any(|slot| slot.key == Some(key)) {
            return Err(Invalid::Lifecycle);
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| !slot.active && Arc::strong_count(&slot.cell) == 1)
            .ok_or(Invalid::Capacity)?;
        slot.key = Some(key);
        slot.active = true;
        slot.owner = None;
        slot.cell.valid.store(true, Ordering::Release);
        Ok(Lease {
            key,
            cell: slot.cell.clone(),
        })
    }
    pub(super) fn complete(&mut self, lease: &Lease) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|slot| {
            slot.key == Some(lease.key) && Arc::ptr_eq(&slot.cell, &lease.cell) && slot.active
        }) else {
            return false;
        };
        slot.active = false;
        true
    }
    pub(super) fn bind_owner(&mut self, lease: &Lease, owner: Owner) {
        if let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.key == Some(lease.key) && Arc::ptr_eq(&slot.cell, &lease.cell))
        {
            slot.owner = Some(owner);
        }
    }
    pub(super) fn context_matches(&self, leases: &Leases, owner: Owner) -> bool {
        leases.0.iter().flatten().all(|lease| {
            self.slots.iter().any(|slot| {
                slot.key == Some(lease.key)
                    && Arc::ptr_eq(&slot.cell, &lease.cell)
                    && slot.owner.is_some_and(|captured| match lease.key.target {
                        Target::Ink => captured == owner,
                        Target::Ui => {
                            captured.session == owner.session
                                && captured.interface == owner.interface
                        }
                    })
            })
        })
    }
    pub(super) fn acknowledge(&mut self, leases: &Leases, owner: Owner) {
        for slot in &mut self.slots {
            if leases
                .0
                .iter()
                .flatten()
                .any(|lease| slot.key == Some(lease.key) && Arc::ptr_eq(&slot.cell, &lease.cell))
            {
                slot.owner = Some(owner);
            }
        }
    }
    pub(super) fn follows(&self, leases: &Leases, sequence: u64) -> bool {
        leases
            .0
            .iter()
            .flatten()
            .all(|lease| lease.key.begin > sequence)
    }
    pub(super) fn retire_active(&mut self) -> [Option<Cancellation>; LIMIT] {
        std::array::from_fn(|i| {
            let slot = &mut self.slots[i];
            if !slot.active {
                return None;
            }
            slot.active = false;
            slot.cell.valid.store(false, Ordering::Release);
            Some(Cancellation(Lease {
                key: slot.key.expect("active slot has a key"),
                cell: slot.cell.clone(),
            }))
        })
    }
}
impl Drop for Bank {
    fn drop(&mut self) {
        for slot in &self.slots {
            slot.cell.valid.store(false, Ordering::Release);
        }
    }
}
/// Negative exact authority only. Caller-visible identity equality cannot match
/// a newer contact, another bank, or another retained allocation generation.
#[derive(Debug)]
pub(super) struct Cancellation(Lease);
impl Cancellation {
    pub(super) fn matches(&self, lease: &Lease) -> bool {
        self.0.same(lease)
    }
}
#[derive(Clone, Debug)]
pub(super) struct Leases([Option<Lease>; LIMIT]);
impl Default for Leases {
    fn default() -> Self {
        Self::new()
    }
}
impl Leases {
    pub(super) fn new() -> Self {
        Self(std::array::from_fn(|_| None))
    }
    pub(super) fn insert(&mut self, lease: &Lease) -> std::result::Result<(), Invalid> {
        if self.0.iter().flatten().any(|old| old.same(lease)) {
            return Ok(());
        }
        let slot = self
            .0
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(Invalid::Capacity)?;
        *slot = Some(lease.clone());
        Ok(())
    }
    pub(super) fn current(&self) -> bool {
        self.0.iter().flatten().all(Lease::current)
    }
    pub(super) fn remove(&mut self, lease: &Lease) {
        for slot in &mut self.0 {
            if slot.as_ref().is_some_and(|old| old.same(lease)) {
                *slot = None;
            }
        }
    }
    pub(super) fn remove_retired(&mut self) {
        for slot in &mut self.0 {
            if slot.as_ref().is_some_and(|lease| !lease.current()) {
                *slot = None;
            }
        }
    }
    pub(super) fn matches(&self, cancellation: &Cancellation) -> bool {
        self.0
            .iter()
            .flatten()
            .any(|lease| cancellation.matches(lease))
    }
}
#[cfg(test)]
mod tests;

//! Fallibly allocated, logically admitted fixed native state. The allocation
//! outlives its contents and is freed before its budget admission is released.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
};
use std::ops::{Deref, DerefMut};

pub(super) struct Resident<T> {
    value: Vec<T>,
    _account: BudgetAccount,
}
impl<T> Resident<T> {
    pub fn new(budget: &SharedBudget, construct: impl FnOnce() -> T) -> Result<Self> {
        let mut account = budget.account(AllocationClass::PaintMetadata);
        account.resize(size_of::<T>() as u64)?;
        let mut value = Vec::new();
        value.try_reserve_exact(1).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Native fixed state allocation refused",
            )
        })?;
        let actual = value
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| {
                Error::new(ErrorCode::ResourceLimit, "Native fixed state size overflow")
            })?;
        account.resize(actual as u64)?;
        value.push(construct());
        Ok(Self {
            value,
            _account: account,
        })
    }
}
impl<T> Deref for Resident<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value[0]
    }
}
impl<T> DerefMut for Resident<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn fixed_state_is_not_constructed_when_admission_is_denied() {
        let budget = SharedBudget::new(1);
        let constructed = Cell::new(false);
        assert!(
            Resident::new(&budget, || {
                constructed.set(true);
                [0u8; 8192]
            })
            .is_err()
        );
        assert!(!constructed.get());
        assert_eq!(budget.usage().reserved, 0);
    }

    #[test]
    fn fixed_state_and_its_allocation_retire_before_the_admission() {
        struct ObservedDrop(SharedBudget, [u8; 8192]);
        impl Drop for ObservedDrop {
            fn drop(&mut self) {
                assert!(self.0.usage().reserved >= size_of::<Self>() as u64);
                assert_eq!(self.1[19], 42);
            }
        }
        let budget = SharedBudget::new(1 << 20);
        let mut value = Resident::new(&budget, || ObservedDrop(budget.clone(), [0; 8192])).unwrap();
        value.1[19] = 42;
        assert_eq!(budget.usage().reserved, value._account.bytes());
        assert_eq!(
            value._account.bytes(),
            (value.value.capacity() * size_of::<ObservedDrop>()) as u64
        );
        drop(value);
        assert_eq!(budget.usage().reserved, 0);
    }
}

//! Fallible, capacity-priced storage for private compiler records.
use super::*;
use paintcraft_core::types::resources::BudgetAccount;
use std::mem::size_of;

#[derive(Debug)]
pub(in crate::execute) struct Store<T> {
    pub values: Vec<T>,
    charge: BudgetAccount,
    limit: usize,
}
impl<T> Store<T> {
    pub fn new(admission: &Admission) -> Self {
        Self {
            values: Vec::new(),
            charge: admission.paint_metadata_account(),
            limit: admission.paint_metadata_limit(),
        }
    }
    pub fn push(&mut self, value: T) -> Result<()> {
        if self.values.len() == self.values.capacity() {
            let before = self.values.capacity();
            let width = size_of::<T>();
            let used = before.checked_mul(width).ok_or_else(exhausted)?;
            let available = self.limit.saturating_sub(used) / width;
            let extra = before.max(4).min(available);
            if extra == 0 {
                return Err(exhausted());
            }
            let bytes = used
                .checked_add(extra.checked_mul(width).ok_or_else(exhausted)?)
                .ok_or_else(exhausted)?;
            self.charge.resize(bytes as u64)?;
            if self.values.try_reserve_exact(extra).is_err() {
                self.charge.resize(used as u64)?;
                return Err(exhausted());
            }
            let actual = self
                .values
                .capacity()
                .checked_mul(width)
                .ok_or_else(exhausted)?;
            if actual > self.limit {
                drop(std::mem::take(&mut self.values));
                self.charge.resize(0)?;
                return Err(exhausted());
            }
            if let Err(error) = self.charge.resize(actual as u64) {
                drop(std::mem::take(&mut self.values));
                self.charge.resize(0)?;
                return Err(error);
            }
        }
        self.values.push(value);
        Ok(())
    }
    pub fn into_parts(self) -> (BudgetAccount, Vec<T>) {
        (self.charge, self.values)
    }
}
fn exhausted() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "batch preparation metadata budget exceeded",
    )
}

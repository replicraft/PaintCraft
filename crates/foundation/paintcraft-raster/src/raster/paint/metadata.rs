//! Capacity payload admission, excluding allocator/hash controls and transient reallocations.
use super::*;
use paintcraft_core::types::resources::BudgetAccount;
use std::mem::size_of;

pub(super) fn default_budget() -> SharedBudget {
    static DEFAULT: OnceLock<SharedBudget> = OnceLock::new();
    DEFAULT
        .get_or_init(|| SharedBudget::new(DEFAULT_RESIDENT_PAINT_METADATA_BYTES))
        .clone()
}
pub(super) fn exhausted() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "paint plan metadata budget exceeded",
    )
}
pub(super) struct Metadata {
    pub used: usize,
    limit: usize,
}
impl Metadata {
    pub fn new(used: usize, limit: usize) -> Result<Self> {
        let budget = Self { used, limit };
        budget.check(0)?;
        Ok(budget)
    }
    fn check(&self, additional: usize) -> Result<()> {
        if additional > self.limit.saturating_sub(self.used) || self.used > self.limit {
            Err(exhausted())
        } else {
            Ok(())
        }
    }
    pub fn vector<T>(&mut self, values: &mut Vec<T>, charge: &mut BudgetAccount) -> Result<()> {
        if values.len() < values.capacity() {
            return Ok(());
        }
        let width = size_of::<T>();
        let available = (self.limit - self.used) / width;
        if available == 0 {
            return Err(exhausted());
        }
        let extra = values.capacity().max(4).min(available);
        let old = values.capacity();
        let before = charge.bytes();
        let predicted = extra.checked_mul(width).ok_or_else(exhausted)? as u64;
        charge.resize(before.checked_add(predicted).ok_or_else(exhausted)?)?;
        if values.try_reserve_exact(extra).is_err() {
            charge.resize(before)?;
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate paint metadata",
            ));
        }
        let actual = (values.capacity() - old)
            .checked_mul(width)
            .ok_or_else(exhausted);
        let result = actual.and_then(|bytes| {
            charge.resize(before.checked_add(bytes as u64).ok_or_else(exhausted)?)?;
            self.check(bytes)?;
            Ok(bytes)
        });
        match result {
            Ok(bytes) => {
                self.used += bytes;
                Ok(())
            }
            Err(error) => {
                // A returned capacity may exceed the request. A failed builder
                // must not retain uncharged storage; free it before notifying release.
                drop(std::mem::take(values));
                charge.resize(before - old as u64 * width as u64)?;
                Err(error)
            }
        }
    }
    pub fn table<T>(
        &mut self,
        tiles: &mut HashMap<TileCoord, T>,
        charge: &mut BudgetAccount,
    ) -> Result<()> {
        if tiles.len() < tiles.capacity() {
            return Ok(());
        }
        let width = size_of::<(TileCoord, T)>();
        self.check(width)?;
        let old = tiles.capacity();
        let before = charge.bytes();
        charge.resize(before.checked_add(width as u64).ok_or_else(exhausted)?)?;
        if tiles.try_reserve(1).is_err() {
            charge.resize(before)?;
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate paint tile plan",
            ));
        }
        let actual = (tiles.capacity() - old)
            .checked_mul(width)
            .ok_or_else(exhausted);
        let result = actual.and_then(|bytes| {
            charge.resize(before.checked_add(bytes as u64).ok_or_else(exhausted)?)?;
            self.check(bytes)?;
            Ok(bytes)
        });
        match result {
            Ok(bytes) => {
                self.used += bytes;
                Ok(())
            }
            Err(error) => {
                // This account owns the map and all of its index vectors.
                drop(std::mem::take(tiles));
                charge.resize(0)?;
                Err(error)
            }
        }
    }
}

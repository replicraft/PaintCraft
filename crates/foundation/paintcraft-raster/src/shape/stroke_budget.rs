use super::*;
use paintcraft_core::types::resources::{BudgetAccount, BudgetScope};
use std::{collections::BinaryHeap, hash::Hash, mem::size_of};
#[derive(Debug)]
pub(super) struct Budget {
    pub used: usize,
    limit: usize,
    charge: Option<BudgetAccount>,
    _scope: Option<BudgetScope>,
}
impl Budget {
    pub fn new(limit: usize) -> Self {
        Self {
            used: 0,
            limit,
            charge: None,
            _scope: None,
        }
    }
    pub fn admitted(limit: usize, scope: BudgetScope) -> Self {
        Self {
            used: 0,
            limit,
            charge: Some(scope.account()),
            _scope: Some(scope),
        }
    }
    fn prepare(&mut self, bytes: usize) -> Result<()> {
        let required = self.used.checked_add(bytes).ok_or_else(Self::error)?;
        if required > self.limit {
            return Err(Self::error());
        }
        if let Some(charge) = &mut self.charge {
            charge.resize(u64::try_from(required).map_err(|_| Self::error())?)?;
        }
        Ok(())
    }
    pub fn release(&mut self, bytes: usize) -> Result<()> {
        self.used = self.used.checked_sub(bytes).ok_or_else(Self::error)?;
        if let Some(charge) = &mut self.charge {
            charge.resize(u64::try_from(self.used).map_err(|_| Self::error())?)?;
        }
        Ok(())
    }
    pub fn admitted_account(&self) -> bool {
        self.charge.is_some()
    }
    pub fn clear(&mut self) -> Result<()> {
        self.release(self.used)
    }
    fn error() -> Error {
        Error::new(
            ErrorCode::ResourceLimit,
            "stroke union scratch budget exceeded",
        )
    }
    fn charge(&mut self, bytes: usize) -> Result<()> {
        if bytes > self.limit.saturating_sub(self.used) {
            return Err(Self::error());
        }
        self.prepare(bytes)?;
        self.used += bytes;
        Ok(())
    }
    pub fn vector<T>(&mut self, values: &mut Vec<T>, check: &dyn Fn() -> Result<()>) -> Result<()> {
        if values.len() < values.capacity() {
            return Ok(());
        }
        check()?;
        let width = size_of::<T>();
        let available = self.limit.saturating_sub(self.used) / width;
        if available == 0 {
            return Err(Self::error());
        }
        let old = values.capacity();
        let extra = old.max(4).min(available);
        self.prepare(extra.checked_mul(width).ok_or_else(Self::error)?)?;
        values.try_reserve_exact(extra).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate stroke union scratch",
            )
        })?;
        self.charge(
            (values.capacity() - old)
                .checked_mul(width)
                .ok_or_else(Self::error)?,
        )
    }
    pub fn table<K: Eq + Hash, V>(
        &mut self,
        map: &mut HashMap<K, V>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if map.len() < map.capacity() {
            return Ok(());
        }
        check()?;
        let width = size_of::<(K, V)>();
        if width > self.limit.saturating_sub(self.used) {
            return Err(Self::error());
        }
        let old = map.capacity();
        self.prepare(width)?;
        map.try_reserve(1).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate stroke union scratch",
            )
        })?;
        self.charge(
            (map.capacity() - old)
                .checked_mul(width)
                .ok_or_else(Self::error)?,
        )
    }
    pub fn heap<T: Ord>(
        &mut self,
        heap: &mut BinaryHeap<T>,
        needed: usize,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if needed <= heap.capacity() {
            return Ok(());
        }
        check()?;
        let old = heap.capacity();
        let width = size_of::<T>();
        let bytes = (needed - old).checked_mul(width).ok_or_else(Self::error)?;
        if bytes > self.limit.saturating_sub(self.used) {
            return Err(Self::error());
        }
        self.prepare(bytes)?;
        heap.try_reserve_exact(needed - heap.len()).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate stroke union scratch",
            )
        })?;
        self.charge(
            (heap.capacity() - old)
                .checked_mul(width)
                .ok_or_else(Self::error)?,
        )
    }
}

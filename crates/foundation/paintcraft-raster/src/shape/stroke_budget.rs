use super::*;
use std::{collections::BinaryHeap, hash::Hash, mem::size_of};
#[derive(Debug)]
pub(super) struct Budget {
    pub used: usize,
    limit: usize,
}
impl Budget {
    pub fn new(limit: usize) -> Self {
        Self { used: 0, limit }
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

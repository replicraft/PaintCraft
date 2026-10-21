//! Bounds declared vector capacities and index payload, not allocator/RSS overhead.
use super::*;
use std::mem::size_of;

pub(super) fn exhausted() -> Error {
    // Fixed policy failure, deliberately without a retryable decoded allocation denial.
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
    pub fn vector<T>(&mut self, values: &mut Vec<T>) -> Result<()> {
        if values.len() < values.capacity() {
            return Ok(());
        }
        let width = size_of::<T>();
        let available = (self.limit - self.used) / width;
        if available == 0 {
            return Err(exhausted());
        }
        // Keep amortized growth, but clamp it to the remaining declared allowance.
        let extra = values.capacity().max(4).min(available);
        let old = values.capacity();
        values
            .try_reserve_exact(extra)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot allocate paint metadata"))?;
        let bytes = (values.capacity() - old)
            .checked_mul(width)
            .ok_or_else(exhausted)?;
        self.check(bytes)?;
        self.used += bytes;
        Ok(())
    }
    pub fn table(&mut self, tiles: &mut HashMap<TileCoord, Vec<usize>>) -> Result<()> {
        if tiles.len() < tiles.capacity() {
            return Ok(());
        }
        let width = size_of::<(TileCoord, Vec<usize>)>();
        self.check(width)?;
        let old = tiles.capacity();
        tiles
            .try_reserve(1)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot allocate paint tile plan"))?;
        // std exposes usable capacity, not allocator bucket layout. Account that
        // payload exactly; hash controls/allocator overhead remain outside this cap.
        let bytes = (tiles.capacity() - old)
            .checked_mul(width)
            .ok_or_else(exhausted)?;
        self.check(bytes)?;
        self.used += bytes;
        Ok(())
    }
}

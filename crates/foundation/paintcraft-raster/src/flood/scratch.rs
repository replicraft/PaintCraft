//! Per-call capacity declarations; no allocator/RSS or global-budget claim.
use super::*;
use std::mem::size_of;
pub(super) struct Scratch {
    used: usize,
    limit: usize,
}
impl Scratch {
    pub fn new(limit: usize) -> Self {
        Self { used: 0, limit }
    }
    fn exhausted() -> Error {
        Error::new(
            ErrorCode::ResourceLimit,
            "flood-fill scratch budget exceeded",
        )
    }
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        required: usize,
        exact: bool,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if required <= values.capacity() {
            return Ok(());
        }
        check()?;
        let width = size_of::<T>();
        let available = self.limit.saturating_sub(self.used) / width;
        let old = values.capacity();
        let needed = required - old;
        if needed > available {
            return Err(Self::exhausted());
        }
        let extra = if exact {
            needed
        } else {
            old.max(4).min(available).max(needed)
        };
        let target = old.checked_add(extra).ok_or_else(Self::exhausted)?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "cannot allocate flood-fill scratch",
                )
            })?;
        let bytes = (values.capacity() - old)
            .checked_mul(width)
            .ok_or_else(Self::exhausted)?;
        if bytes > self.limit.saturating_sub(self.used) {
            return Err(Self::exhausted());
        }
        self.used += bytes;
        Ok(())
    }
    pub fn bitmap(&mut self, words: usize, check: &dyn Fn() -> Result<()>) -> Result<Vec<u64>> {
        let mut values = Vec::new();
        self.reserve(&mut values, words, true, check)?;
        while values.len() < words {
            check()?;
            values.resize((values.len() + INTERVAL).min(words), 0);
        }
        Ok(values)
    }
    pub fn push<T>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let required = values.len().checked_add(1).ok_or_else(Self::exhausted)?;
        self.reserve(values, required, false, check)?;
        values.push(value);
        Ok(())
    }
}
// A work-count policy, not a measured wall-clock cancellation bound.
pub(super) const INTERVAL: usize = 1024;
pub(super) struct Work<'a> {
    remaining: usize,
    check: &'a dyn Fn() -> Result<()>,
}
impl<'a> Work<'a> {
    pub fn new(check: &'a dyn Fn() -> Result<()>) -> Self {
        Self {
            remaining: INTERVAL,
            check,
        }
    }
    pub fn step(&mut self) -> Result<()> {
        self.remaining -= 1;
        if self.remaining == 0 {
            (self.check)()?;
            self.remaining = INTERVAL;
        }
        Ok(())
    }
}

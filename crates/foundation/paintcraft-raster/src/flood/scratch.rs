//! Fixed per-call bounds and optional original caller metadata admission.
//! No allocator/reallocation/RSS accounting claim.
use super::*;
use std::mem::size_of;
pub(super) struct Scratch {
    used: usize,
    limit: usize,
    charge: Option<BudgetAccount>,
    _scope: Option<BudgetScope>,
}
impl Scratch {
    pub fn new(limit: usize, scope: Option<BudgetScope>) -> Self {
        Self {
            used: 0,
            limit,
            charge: scope.as_ref().map(BudgetScope::account),
            _scope: scope,
        }
    }
    fn declare(&mut self, bytes: usize) -> Result<()> {
        if let Some(charge) = &mut self.charge {
            charge.resize(u64::try_from(bytes).map_err(|_| Self::exhausted())?)?;
        }
        Ok(())
    }
    pub fn finish(mut self, values: Vec<Span>) -> Result<AdmittedSpans> {
        // Caller has already freed bitmap/frontier. Move the SAME account;
        // charging a sibling here would double-admit the returned payload.
        self.declare(
            values
                .capacity()
                .checked_mul(size_of::<Span>())
                .ok_or_else(Self::exhausted)?,
        )?;
        Ok(AdmittedSpans {
            values,
            _charge: self.charge,
            _scope: self._scope,
        })
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
        let before = self.used;
        self.declare(
            before
                .checked_add(
                    (target - old)
                        .checked_mul(width)
                        .ok_or_else(Self::exhausted)?,
                )
                .ok_or_else(Self::exhausted)?,
        )?;
        if values.try_reserve_exact(target - values.len()).is_err() {
            self.declare(before)?;
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate flood-fill scratch",
            ));
        }
        let actual = values
            .capacity()
            .checked_mul(width)
            .ok_or_else(Self::exhausted);
        let result = actual.and_then(|actual| {
            let bytes = actual
                .checked_sub(old * width)
                .ok_or_else(Self::exhausted)?;
            if bytes > self.limit.saturating_sub(before) {
                return Err(Self::exhausted());
            }
            self.declare(before.checked_add(bytes).ok_or_else(Self::exhausted)?)?;
            Ok(bytes)
        });
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => {
                drop(std::mem::take(values));
                self.used = before - old * width;
                self.declare(self.used)?;
                return Err(error);
            }
        };
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

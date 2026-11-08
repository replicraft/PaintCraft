//! Borrowed cooperative metadata copy; not an owned browser source lease.
use crate::{Entry, History, archive};
use paintcraft_core::types::{
    Error, Result,
    resources::{BudgetAccount, BudgetScope},
};
use std::{future::Future, mem::size_of};
mod quote;
use quote::{Quote, visit_work};

const CHUNK: usize = 64;
/// A copied timeline and its original metadata admission, retired in that order.
/// No mutable or consuming escape can detach storage from the account.
#[derive(Debug)]
pub struct ForkedHistory {
    history: History,
    account: BudgetAccount,
    _scope: BudgetScope,
}
impl ForkedHistory {
    pub fn history(&self) -> &History {
        &self.history
    }
    /// Declared copied storage/control bytes, excluding shared source payloads.
    pub fn copied_bytes(&self) -> u64 {
        self.account.bytes()
    }
    /// Move actual storage/account together, then retire only the unused factory.
    /// No detached History or replacement reservation escapes this handoff.
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns original owned storage without allocating an error box"
    )]
    pub fn into_retained_storage(
        self,
    ) -> std::result::Result<crate::RetainedHistoryStorage, crate::SourceRejected<Self>> {
        let Self {
            history,
            account,
            _scope,
        } = self;
        match crate::source::retained::from_fork_parts(history, account) {
            Ok(storage) => {
                drop(_scope);
                Ok(storage)
            }
            Err(rejected) => Err(crate::SourceRejected {
                original: Self {
                    history: rejected.history,
                    account: rejected.account,
                    _scope,
                },
                error: rejected.error,
            }),
        }
    }
}
// Field order also protects errors and incomplete/cancelled future destruction.
struct Building {
    entries: Vec<Entry>,
    memo: archive::Memo,
    cleanup: Option<Error>,
    packing: Option<Error>,
    account: BudgetAccount,
    scope: BudgetScope,
}
impl Building {
    fn new(scope: BudgetScope) -> Result<Self> {
        let mut account = scope.account();
        account.resize(size_of::<ForkedHistory>() as u64)?;
        Ok(Self {
            entries: Vec::new(),
            memo: archive::Memo::new(),
            cleanup: None,
            packing: None,
            account,
            scope,
        })
    }
    fn finish(self, source: &History) -> ForkedHistory {
        ForkedHistory {
            history: History {
                entries: self.entries,
                cursor: source.cursor,
                budget: source.budget,
                trimmed_total: source.trimmed_total,
                last_trimmed: source.last_trimmed,
                memo: self.memo,
                packing: source.packing,
                retention: source.retention,
                compression: source.compression,
                spill_store: source.spill_store.clone(),
                last_cleanup_error: self.cleanup,
                restore_pixels: source.restore_pixels,
                last_packing_error: self.packing,
            },
            account: self.account,
            _scope: self.scope,
        }
    }
}
impl History {
    /// Fallibly fork with the caller's exact scope and cumulative work debit.
    /// The future borrows this History across yields; it is not transferable
    /// browser history ownership. Old synchronous fork/publication paths are unchanged.
    ///
    /// Iter::next may scan an entire sparse table between live entries; scans are
    /// fully precharged, not preemptible. Reserve and cancellation drop are also
    /// indivisible. No hard per-poll or wall-clock bound is established here.
    pub async fn cooperative_fork<Y, F>(
        &self,
        scope: BudgetScope,
        spend: &dyn Fn(u64) -> Result<()>,
        check: &dyn Fn() -> Result<()>,
        mut yield_now: Y,
    ) -> Result<ForkedHistory>
    where
        Y: FnMut() -> F,
        F: Future<Output = ()>,
    {
        check()?;
        spend(64)?;
        check()?;
        let mut building = Building::new(scope)?;
        // Both complete physical scan bounds are priced independently. The
        // iterator lives inside this pinned future, without restart/skip cursors.
        spend(self.memo.scan_work_upper()?)?;
        check()?;
        let mut survivors = 0usize;
        for (index, (key, _)) in self.memo.iter().enumerate() {
            check()?;
            spend(visit_work())?;
            check()?;
            if key.resident_bytes() != 0 {
                survivors = survivors
                    .checked_add(1)
                    .ok_or_else(|| archive::limit("history survivor count overflow"))?;
            }
            if (index + 1).is_multiple_of(CHUNK) {
                yield_now().await;
                check()?;
            }
        }
        yield_now().await;
        check()?;
        let quote = Quote::new(self, survivors)?;
        building.account.resize(quote.retained)?;
        spend(quote.initialization)?;
        check()?;
        building.memo.try_reserve(survivors)?;
        if building.memo.capacity() != quote.memo_capacity {
            return Err(archive::limit("history fork fresh memo layout changed"));
        }
        building
            .entries
            .try_reserve_exact(quote.entry_capacity)
            .map_err(|_| archive::limit("cannot prepare reversible history"))?;
        if building.entries.capacity() != quote.entry_capacity {
            return Err(archive::limit("history fork entry capacity changed"));
        }
        // Error clones preserve allocation-denial diagnostics and share their
        // original budget handles. Only their message Strings allocate new bytes.
        spend(quote.error_bytes)?;
        check()?;
        building.cleanup = self.last_cleanup_error.clone();
        building.packing = self.last_packing_error.clone();
        quote.check_errors(building.cleanup.as_ref(), building.packing.as_ref())?;
        yield_now().await;
        check()?;
        spend(self.memo.scan_work_upper()?)?;
        check()?;
        for (index, (key, value)) in self.memo.iter().enumerate() {
            check()?;
            spend(visit_work())?;
            check()?;
            if key.resident_bytes() != 0 {
                spend(
                    quote::clone_work()
                        .checked_add(quote::insertion_work(quote.memo_buckets)?)
                        .ok_or_else(|| archive::limit("history fork insertion work overflow"))?,
                )?;
                check()?;
                let _ = building.memo.insert_reserved(key.clone(), value.clone())?;
            }
            if (index + 1).is_multiple_of(CHUNK) {
                yield_now().await;
                check()?;
            }
        }
        yield_now().await;
        check()?;
        for (index, entry) in self.entries.iter().enumerate() {
            check()?;
            spend(size_of::<Entry>() as u64)?;
            check()?;
            building.entries.push(entry.clone());
            if (index + 1).is_multiple_of(CHUNK) {
                yield_now().await;
                check()?;
            }
        }
        yield_now().await;
        check()?;
        Ok(building.finish(self))
    }
}
#[cfg(test)]
mod tests;

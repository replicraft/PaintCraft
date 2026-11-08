//! Opt-in owned immutable History source; no Session or browser activation.
use crate::{History, archive};
use paintcraft_core::types::{
    Error, Result,
    resources::{BudgetAccount, BudgetScope},
};
use std::future::Future;
mod quote;
pub use quote::ReadSourceQuote;
pub(crate) mod retained;
pub use retained::RetainedHistoryStorage;
use retained::SharedOwner;

/// Refusal returns the exact original owner, never an empty replacement timeline.
#[derive(Debug)]
pub struct SourceRejected<T> {
    pub original: T,
    pub error: Error,
}
/// Source Arc storage retires before either assigned account can notify reentry.
#[derive(Debug)]
struct SourceStorage {
    retained: RetainedHistoryStorage,
    control: BudgetAccount,
}
/// Owned across awaits, with no mutable/Weak/raw ownership or account escape.
/// Clone shares actual source storage/charges; containing handles are quoted
/// separately from the one shared allocation. No unused BudgetScope is retained.
#[derive(Clone, Debug)]
pub struct HistoryReadSource {
    inner: SharedOwner<SourceStorage>,
}
impl HistoryReadSource {
    /// O(1) true backing/control upper. Does not walk or clone source entries.
    pub fn quote_owned(history: &History) -> Result<ReadSourceQuote> {
        quote::source(history)
    }
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns original owned storage without allocating an error box"
    )]
    pub fn capture_owned(
        history: History,
        scope: BudgetScope,
        spend: &dyn Fn(u64) -> Result<()>,
        check: &dyn Fn() -> Result<()>,
    ) -> std::result::Result<Self, SourceRejected<History>> {
        let admitted = (|| {
            check()?;
            spend(64)?;
            check()?;
            let quote = quote::source(&history)?;
            let mut backing = scope.account();
            backing.resize(quote.retained_bytes)?;
            let mut control = scope.account();
            control.resize(quote.shared_control_bytes)?;
            check()?;
            Ok::<_, Error>((backing, control))
        })();
        match admitted {
            Err(error) => Err(SourceRejected {
                original: history,
                error,
            }),
            Ok((account, control)) => {
                // Every fallible quote/debit/admission/check precedes this move.
                let retained = RetainedHistoryStorage::from_admitted(history, account);
                Ok(Self {
                    inner: SharedOwner::new(SourceStorage { retained, control }),
                })
            }
        }
        // scope retires after the returned storage/accounts are coupled.
    }
    /// Reuse the actual retained backing account; admit only additional Arc/control
    /// storage in the injected scope. A numerical funded flag is never accepted.
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns original owned storage without allocating an error box"
    )]
    pub fn capture_retained(
        storage: RetainedHistoryStorage,
        scope: BudgetScope,
        spend: &dyn Fn(u64) -> Result<()>,
        check: &dyn Fn() -> Result<()>,
    ) -> std::result::Result<Self, SourceRejected<RetainedHistoryStorage>> {
        let admitted = (|| {
            check()?;
            spend(64)?;
            check()?;
            let quote = quote::source(storage.history())?;
            if storage.assigned_bytes() != quote.retained_bytes {
                return Err(archive::limit("retained History declaration changed"));
            }
            let mut control = scope.account();
            control.resize(quote.shared_control_bytes)?;
            check()?;
            Ok::<_, Error>(control)
        })();
        match admitted {
            Err(error) => Err(SourceRejected {
                original: storage,
                error,
            }),
            Ok(control) => Ok(Self {
                inner: SharedOwner::new(SourceStorage {
                    retained: storage,
                    control,
                }),
            }),
        }
    }
    pub fn history(&self) -> &History {
        self.inner.retained.history()
    }
    pub fn same_source(&self, other: &Self) -> bool {
        self.inner.same(&other.inner)
    }
    /// Actual combined declarations, excluding preexisting shared payloads and
    /// containing read-handle fields. This is not allocator/RSS accounting.
    pub fn assigned_bytes(&self) -> Result<u64> {
        self.inner
            .retained
            .assigned_bytes()
            .checked_add(self.inner.control.bytes())
            .ok_or_else(|| archive::limit("read-source assigned size overflow"))
    }
    /// Recover only a unique opaque retained owner. No source clone or mutable
    /// History escapes; shared refusal returns this same live source handle.
    pub fn try_into_retained(self) -> std::result::Result<RetainedHistoryStorage, Self> {
        match self.inner.try_into_inner() {
            Ok(SourceStorage { retained, control }) => {
                // Arc::try_unwrap has already released the private Arc block.
                drop(control);
                Ok(retained)
            }
            Err(inner) => Err(Self { inner }),
        }
    }
    /// The standard future owns its source before borrowing the qualified fork.
    /// Iterator/reserve/drop indivisibility remains; no UI latency is promised.
    pub async fn fork_owned<Y, F>(
        self,
        scope: BudgetScope,
        spend: &dyn Fn(u64) -> Result<()>,
        check: &dyn Fn() -> Result<()>,
        yield_now: Y,
    ) -> Result<RetainedHistoryStorage>
    where
        Y: FnMut() -> F,
        F: Future<Output = ()>,
    {
        let fork = self
            .history()
            .cooperative_fork(scope, spend, check, yield_now)
            .await?;
        fork.into_retained_storage()
            .map_err(|rejected| rejected.error)
    }
}
#[cfg(test)]
mod tests;

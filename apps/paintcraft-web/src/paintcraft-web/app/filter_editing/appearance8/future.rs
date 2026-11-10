//! Defining application future payloads, separate from runtime Task/Promise backing.
use super::{denied, factory::Factory8};
use paintcraft_core::types::{Result, resources::BudgetAccount};
use std::{
    future::Future,
    mem::size_of,
    pin::Pin,
    task::{Context, Poll},
};
#[cfg(target_arch = "wasm32")]
#[path = "future/host.rs"]
pub(super) mod host;

#[derive(Clone, Copy, Debug)]
pub(super) struct Quote8 {
    pub bytes: u64,
    pub work: u64,
}
/// The inner F is independently pinned; moving this wrapper never moves F.
pub(super) struct SourceFuture8<F: Future> {
    inner: Pin<Box<F>>,
    _account: BudgetAccount,
    _factory: Factory8,
}
impl<F: Future> std::fmt::Debug for SourceFuture8<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainSourceFuture8")
            .field("payload_bytes", &size_of::<F>())
            .finish_non_exhaustive()
    }
}
impl<F: Future> Future for SourceFuture8<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().inner.as_mut().poll(cx)
    }
}
fn sum(parts: &[u64]) -> Result<u64> {
    parts
        .iter()
        .try_fold(0u64, |a, n| a.checked_add(*n).ok_or_else(denied))
}
pub(super) fn quote<A, F: Future>(_: fn(A) -> F) -> Result<Quote8> {
    if !matches!(size_of::<usize>(), 4 | 8)
        || size_of::<paintcraft_core::types::Error>() > 96
        || size_of::<Result<u64>>() > 112
        || size_of::<Result<Quote8>>() > 112
    {
        return Err(denied());
    }
    // F has its own box. The supplied application wrapper occupies its named
    // payload inside the runtime box; Task/adapter/Rc/Waker backing is separate.
    let bytes = sum(&[size_of::<F>() as u64, size_of::<SourceFuture8<F>>() as u64])?;
    let work = sum(&[
        2 * size_of::<A>() as u64,
        2 * size_of::<F>() as u64,
        4 * size_of::<SourceFuture8<F>>() as u64,
        4 * size_of::<SourceCredit8<A, F>>() as u64,
        4 * size_of::<Result<SourceCredit8<A, F>>>() as u64,
        4 * size_of::<std::result::Result<SourceFuture8<F>, (paintcraft_core::types::Error, A)>>()
            as u64,
        16 * size_of::<Result<()>>() as u64,
        256,
    ])?;
    Ok(Quote8 { bytes, work })
}
pub(super) struct SourceCredit8<A, F: Future> {
    account: BudgetAccount,
    factory: Factory8,
    _shape: std::marker::PhantomData<fn(A) -> F>,
}
impl<A, F: Future> std::fmt::Debug for SourceCredit8<A, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainFutureCredit8").finish_non_exhaustive()
    }
}
pub(super) fn reserve<A, F: Future>(
    factory: &Factory8,
    spend: &dyn Fn(u64) -> Result<()>,
    make: fn(A) -> F,
) -> Result<SourceCredit8<A, F>> {
    // Quote itself has new complete Result returns; fund them before calling it.
    spend(4096)?;
    let q = quote(make)?;
    if q.bytes > factory.cap() || factory.is_quarantined() {
        return Err(denied());
    }
    spend(q.work)?;
    let mut account = factory.scope().account();
    account.resize(q.bytes)?;
    if factory.is_quarantined() {
        return Err(denied());
    }
    Ok(SourceCredit8 {
        account,
        factory: factory.clone(),
        _shape: std::marker::PhantomData,
    })
}
impl<A, F: Future> SourceCredit8<A, F> {
    pub fn construct(
        self,
        args: A,
        make: fn(A) -> F,
    ) -> std::result::Result<SourceFuture8<F>, (paintcraft_core::types::Error, A)> {
        if self.factory.is_quarantined() {
            return Err((denied(), args));
        }
        Ok(SourceFuture8 {
            inner: Box::pin(make(args)),
            _account: self.account,
            _factory: self.factory,
        })
    }
}
/// Denial returns the unconsumed arguments; F is not constructed or boxed first.
pub(super) fn track<A, F: Future>(
    factory: &Factory8,
    spend: &dyn Fn(u64) -> Result<()>,
    args: A,
    make: fn(A) -> F,
) -> std::result::Result<SourceFuture8<F>, (paintcraft_core::types::Error, A)> {
    match reserve(factory, spend, make) {
        Ok(credit) => credit.construct(args, make),
        Err(error) => Err((error, args)),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "future/tests.rs"]
mod tests;

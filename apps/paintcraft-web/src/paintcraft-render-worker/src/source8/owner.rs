//! No domain, Arc, Weak or independently cloneable credit escapes this module.
use super::*;
use std::{marker::PhantomData, sync::Arc};
#[repr(C)]
pub(super) struct RetainedFactories8 {
    pub(super) _m: BudgetScope,
    pub(super) _a: BudgetScope,
    pub(super) _c: BudgetScope,
    pub(super) _i: BudgetReservation,
    pub(super) _q: BudgetReservation,
    pub(super) _raster: paintcraft_raster::ReservedDecodeFactories,
}
impl RetainedFactories8 {
    pub(super) fn capture_checked(
        realms: &Source8ReadRealms<'_>,
        auth: &entry::EntryAuth8<'_>,
    ) -> Result<Self> {
        auth.spend(38)?;
        Ok(Self {
            _m: realms.m.clone(),
            _a: realms.a.clone(),
            _c: realms.c.clone(),
            _i: realms.i.clone(),
            _q: realms.q.clone(),
            _raster: realms.raster.clone(),
        })
    }
}
#[repr(C)]
pub(super) struct SourcePayload8 {
    pub(super) _document: paintcraft_document::Document,
    pub(super) _adopted: paintcraft_formats::pcnd::layer_blend_appearance::AdoptedCoverage8,
    pub(super) inspection: Source8Inspection,
    pub(super) _m_guard: BudgetAccount,
    pub(super) _factories: RetainedFactories8,
}
#[repr(C)]
pub(super) struct CarrierCredit8 {
    _account: BudgetAccount,
    _factory: BudgetScope,
}
struct PrivateArc<T>(Option<Arc<T>>);
impl<T> PrivateArc<T> {
    fn new(value: T) -> Self {
        Self(Some(Arc::new(value)))
    }
    fn clear(&mut self) {
        if let Some(owner) = self.0.take()
            && let Some(value) = Arc::into_inner(owner)
        {
            // No Weak escapes: Arc block/implicit weak end before returned value.
            drop(value);
        }
    }
}
impl<T> Drop for PrivateArc<T> {
    fn drop(&mut self) {
        self.clear();
    }
}
#[repr(C)]
pub(super) struct ClosedSource8 {
    payload: PrivateArc<SourcePayload8>,
    credit: PrivateArc<CarrierCredit8>,
}
impl ClosedSource8 {
    pub(super) fn inspect_scalar(&self) -> Result<Source8Inspection> {
        Ok(self.payload.0.as_deref().ok_or_else(incomplete)?.inspection)
    }
    fn clear(&mut self) {
        self.payload.clear();
        self.credit.clear();
    }
    #[cfg(test)]
    pub(super) fn clone_for_retirement(&self) -> Retirement8<'_> {
        Retirement8 {
            source: Self {
                payload: PrivateArc(self.payload.0.clone()),
                credit: PrivateArc(self.credit.0.clone()),
            },
            _entry: PhantomData,
        }
    }
}
impl Drop for ClosedSource8 {
    fn drop(&mut self) {
        self.clear();
    }
}
/// Only scoped native qualification uses this drop-only type initially.
#[repr(C)]
pub(super) struct Retirement8<'entry> {
    source: ClosedSource8,
    _entry: PhantomData<&'entry ()>,
}
impl Drop for Retirement8<'_> {
    fn drop(&mut self) {
        self.source.clear();
    }
}
pub(super) fn close_checked(
    payload: SourcePayload8,
    auth: &entry::EntryAuth8<'_>,
    realms: &Source8ReadRealms<'_>,
) -> Result<ClosedSource8> {
    auth.spend(64)?;
    let bytes = owner_ledger::retained_bytes()?;
    if bytes > realms.caps.k {
        return Err(extent());
    }
    let mut account = realms.k.account();
    account.resize(bytes)?;
    auth.spend(64)?; // first paired handle's eventual retirement, before escape
    // Credit local exists before either Arc; payload always ends before credit.
    let credit = CarrierCredit8 {
        _account: account,
        _factory: realms.k.clone(),
    };
    let payload = PrivateArc::new(payload);
    let credit = PrivateArc::new(credit);
    Ok(ClosedSource8 { payload, credit })
}

#[cfg(test)]
#[path = "tests/payload_cases.rs"]
pub(super) mod payload_cases;

#[cfg(test)]
#[path = "tests/arc_cases.rs"]
mod arc_cases;

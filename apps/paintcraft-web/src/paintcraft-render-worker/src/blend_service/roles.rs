//! All original factories come from the one compiled service budget, never usage.
use super::manifest::*;
use paintcraft_core::types::{
    Result,
    resources::{AllocationClass, BudgetReservation, BudgetScope, SharedBudget},
};
#[repr(C)]
pub(super) struct Roles8 {
    pub encoded: BudgetScope,
    pub p: BudgetScope,
    pub m: BudgetScope,
    pub a: BudgetScope,
    pub c: BudgetScope,
    pub k: BudgetScope,
    pub w: BudgetScope,
    pub f: BudgetScope,
    pub o: BudgetScope,
    pub i: BudgetReservation,
    pub q: BudgetReservation,
    pub raster: paintcraft_raster::ReservedDecodeFactories,
}
pub(super) fn reserve(budget: &SharedBudget, n: u64) -> Result<BudgetScope> {
    Ok(budget
        .reserve_capacity(n)?
        .scope(AllocationClass::PaintMetadata))
}
impl Roles8 {
    /// H already funds every actual shared factory allocation; operation already paid formation.
    pub fn new(budget: &SharedBudget, open: &Open8, w: BudgetScope) -> Result<Self> {
        let r = &open.roles;
        let encoded = reserve(budget, r[E])?;
        let p = reserve(budget, r[P])?;
        let m = reserve(budget, r[M])?;
        let a = reserve(budget, r[A])?;
        let c = reserve(budget, r[C])?;
        let k = reserve(budget, r[K])?;
        let f = reserve(budget, r[F])?;
        let o = reserve(budget, r[O])?;
        let i = budget.reserve_capacity(r[I])?;
        let q = budget.reserve_capacity(r[Q])?;
        let raster = paintcraft_raster::ReservedDecodeFactories {
            selection_budget: budget.clone(),
            coverage_budget: budget.clone(),
            paint_metadata_scope: m.clone(),
        };
        Ok(Self {
            encoded,
            p,
            m,
            a,
            c,
            k,
            w,
            f,
            o,
            i,
            q,
            raster,
        })
    }
    pub fn read(&self, open: &Open8) -> crate::source8::Source8ReadRealms<'_> {
        crate::source8::Source8ReadRealms::new(
            crate::source8::Source8ReadScopes {
                p: &self.p,
                m: &self.m,
                a: &self.a,
                c: &self.c,
                k: &self.k,
                i: &self.i,
                q: &self.q,
                raster: &self.raster,
            },
            crate::source8::Source8ReadCaps {
                p: open.roles[P],
                m: open.roles[M],
                a: open.roles[A],
                i: open.roles[I],
                c: open.roles[C],
                q: open.roles[Q],
                k: open.roles[K],
                staged_pixels: open.roles[Q] / 4,
            },
        )
    }
    pub fn frame(&self, open: &Open8) -> crate::source8::Source8FrameRealms<'_> {
        crate::source8::Source8FrameRealms::new(
            crate::source8::Source8FrameScopes {
                control: &self.f,
                output: &self.o,
            },
            crate::source8::Source8FrameCaps {
                workspace: open.roles[W],
                control: open.roles[F],
                output: open.roles[O],
            },
        )
    }
}

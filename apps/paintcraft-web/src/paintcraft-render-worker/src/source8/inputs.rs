use super::*;
use paintcraft_raster::ReservedDecodeFactories;
use paintcraft_render::noise_work::NoiseLimits;
use std::fmt;

/// Exact original capacity declarations. Scalars create no capacity or authority.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Source8ReadCaps {
    pub p: u64,
    pub m: u64,
    pub a: u64,
    pub i: u64,
    pub c: u64,
    pub q: u64,
    pub k: u64,
    pub staged_pixels: u64,
}
/// Explicit original host factories; this is not proof of their ancestry.
#[repr(C)]
pub struct Source8ReadScopes<'r> {
    pub p: &'r BudgetScope,
    pub m: &'r BudgetScope,
    pub a: &'r BudgetScope,
    pub c: &'r BudgetScope,
    pub k: &'r BudgetScope,
    pub i: &'r BudgetReservation,
    pub q: &'r BudgetReservation,
    pub raster: &'r ReservedDecodeFactories,
}
impl fmt::Debug for Source8ReadScopes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Source8ReadScopes")
            .field("factories", &8)
            .finish()
    }
}
/// Groups borrowed originals without allocating, admitting or capturing a job.
/// The embedding Rust host must supply its attempt's selected original handles.
#[repr(C)]
pub struct Source8ReadRealms<'r> {
    pub(super) p: &'r BudgetScope,
    pub(super) m: &'r BudgetScope,
    pub(super) a: &'r BudgetScope,
    pub(super) c: &'r BudgetScope,
    pub(super) k: &'r BudgetScope,
    pub(super) i: &'r BudgetReservation,
    pub(super) q: &'r BudgetReservation,
    pub(super) raster: &'r ReservedDecodeFactories,
    pub(super) caps: Source8ReadCaps,
}
impl<'r> Source8ReadRealms<'r> {
    pub fn new(scopes: Source8ReadScopes<'r>, caps: Source8ReadCaps) -> Self {
        Self {
            p: scopes.p,
            m: scopes.m,
            a: scopes.a,
            c: scopes.c,
            k: scopes.k,
            i: scopes.i,
            q: scopes.q,
            raster: scopes.raster,
            caps,
        }
    }
    pub(super) fn validate_originals(&self, auth: &entry::EntryAuth8<'_>) -> Result<()> {
        for (scope, declared) in [
            (self.p, self.caps.p),
            (self.m, self.caps.m),
            (self.a, self.caps.a),
            (self.c, self.caps.c),
            (self.k, self.caps.k),
            (&self.raster.paint_metadata_scope, self.caps.m),
        ] {
            // Existing snapshot still reads available under its original lock.
            // Only immutable ceiling/reserved participate in this decision.
            auth.spend(112)?;
            let snapshot = scope.capacity_snapshot();
            if !snapshot.reserved || snapshot.ceiling != declared {
                return Err(realm_error());
            }
        }
        auth.spend(64)?; // original I/Q immutable capacity helpers
        if self.i.capacity() != self.caps.i || self.q.capacity() != self.caps.q {
            return Err(realm_error());
        }
        Ok(())
    }
    pub(super) fn declared_union(&self) -> Result<u64> {
        [
            self.caps.p,
            self.caps.m,
            self.caps.a,
            self.caps.i,
            self.caps.c,
            self.caps.q,
            self.caps.k,
        ]
        .into_iter()
        .try_fold(0u64, |sum, n| sum.checked_add(n).ok_or_else(extent))
    }
}
impl fmt::Debug for Source8ReadRealms<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Source8ReadRealms")
            .field("factories", &8)
            .field("caps", &self.caps)
            .finish()
    }
}
/// Selects which actual installed caller must already exist. No root is created.
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Source8InspectionMode {
    Base,
    Noise,
}
/// Plan work is local parser/materializer work, not total original job work.
/// Authentication, ownership, assembly and adoption debit that SAME job too.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Source8InspectionRequest {
    pub mode: Source8InspectionMode,
    pub caller: NoiseLimits,
    pub source: paintcraft_formats::pcnd::PortableSource8Limits,
}
fn realm_error() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source8 read realm is not original reserved storage",
    )
}

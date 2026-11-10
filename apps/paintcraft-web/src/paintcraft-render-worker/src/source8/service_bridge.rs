//! Private, owned loaded source. Real service/token dispatch remains separate.
use super::*;
use paintcraft_core::types::resources::BudgetScope;
#[path = "service_bridge/layout.rs"]
mod layout;
#[path = "service_bridge/view.rs"]
mod view;

#[repr(C)]
pub(super) struct OwnedReadFactories8 {
    _p: BudgetScope,
    _m: BudgetScope,
    _a: BudgetScope,
    c: BudgetScope,
    _k: BudgetScope,
    _i: BudgetReservation,
    _q: BudgetReservation,
    _raster: paintcraft_raster::ReservedDecodeFactories,
}
impl OwnedReadFactories8 {
    // Called only AFTER transfer phase/refcount/return prepayment and checks.
    pub(super) fn capture_prepaid(realms: &Source8ReadRealms<'_>) -> Self {
        Self {
            _p: realms.p.clone(),
            _m: realms.m.clone(),
            _a: realms.a.clone(),
            c: realms.c.clone(),
            _k: realms.k.clone(),
            _i: realms.i.clone(),
            _q: realms.q.clone(),
            _raster: realms.raster.clone(),
        }
    }
}
#[repr(C)]
pub(super) struct ServiceSource8 {
    source: owner::ClosedSource8,
    caps: Source8ReadCaps,
    source_sha256: [u8; 32],
    _control: BudgetAccount,
    // Original factories outlive source and its SAME transferred C account.
    factories: OwnedReadFactories8,
}
impl ServiceSource8 {
    pub(super) fn from_prepaid_parts(
        source: owner::ClosedSource8,
        caps: Source8ReadCaps,
        source_sha256: [u8; 32],
        control: BudgetAccount,
        factories: OwnedReadFactories8,
    ) -> Self {
        Self {
            source,
            caps,
            source_sha256,
            _control: control,
            factories,
        }
    }
    fn read_bytes(&self) -> Result<u64> {
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
        .try_fold(0u64, |a, b| a.checked_add(b).ok_or_else(extent))
    }
}
/// Private grouping of actual originals, not an authority or ancestry token.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct ServiceOriginals8<'a> {
    pub(super) encoded: &'a BudgetScope,
    pub(super) control: &'a BudgetScope,
    pub(super) encoded_capacity: u64,
    pub(super) control_capacity: u64,
}
#[repr(C)]
pub(super) struct ServiceView8 {
    caller: caller::RequiredCaller8,
    source_sha256: [u8; 32],
    _encoded: BudgetScope,
    encoded_capacity: u64,
    control_capacity: u64,
    failed: bool,
    control: BudgetAccount,
    _control_factory: BudgetScope,
}
// No capture, context installation, new counter, Vec clone or default fallback.
// At most16 Arc increments (two complete Noise handles), prepaid with retirement.
pub(super) fn clone_required_prepaid(value: &caller::RequiredCaller8) -> caller::RequiredCaller8 {
    match value {
        caller::RequiredCaller8::Base(base) => caller::RequiredCaller8::Base(base.clone()),
        caller::RequiredCaller8::Noise { selected, ambient } => caller::RequiredCaller8::Noise {
            selected: selected.clone(),
            ambient: ambient.clone(),
        },
    }
}
pub(super) fn transfer_work() -> Result<u64> {
    let named = layout::transfer_named()?;
    named
        .checked_mul(2)
        .and_then(|n| n.checked_add(50))
        .ok_or_else(extent)
}
pub(super) fn patch_auth_work() -> Result<u64> {
    let named = layout::view_named()?;
    named
        .checked_mul(2)
        .and_then(|n| n.checked_add(32))
        .ok_or_else(extent)
}
pub(super) fn validate_transfer() -> Result<()> {
    layout::validate()
}

#[cfg(test)]
#[path = "service_bridge/tests.rs"]
mod tests;

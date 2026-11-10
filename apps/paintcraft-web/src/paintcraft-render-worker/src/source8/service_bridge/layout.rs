//! Independent fixed containing-control slots; no observed-demand growth.
use super::*;
use std::mem::size_of;
pub(super) const VIEW_CONTROL: u64 = 4096;
fn native() -> bool {
    cfg!(target_pointer_width = "64")
}
pub(super) fn transfer_named() -> Result<u64> {
    let n = size_of::<entry::Entry8<'static>>()
        + 2 * size_of::<ServiceSource8>()
        + 2 * size_of::<OwnedReadFactories8>()
        + 2 * size_of::<Source8ReadCaps>()
        + 3 * size_of::<BudgetAccount>()
        + 2 * size_of::<entry::SourceSlot8>()
        + 4 * size_of::<owner::ClosedSource8>()
        + 2 * 384
        + 2 * size_of::<Source8Inspection>()
        + 160
        + 3 * 128
        + size_of::<paintcraft_core::types::resources::ScopeCapacity>()
        + size_of::<[u64; 16]>()
        + 3 * size_of::<&BudgetScope>();
    if n != if native() { 3888 } else { 3116 } || n as u64 > owner_ledger::ENTRY_BYTES {
        return Err(extent());
    }
    Ok(n as u64)
}
pub(super) fn begin_named() -> Result<u64> {
    let n = size_of::<entry::Entry8<'static>>()
        + size_of::<ServiceSource8>()
        + 2 * size_of::<ServiceView8>()
        + 2 * size_of::<caller::RequiredCaller8>()
        + 256
        + 2 * 448
        + 2 * size_of::<ServiceOriginals8<'static>>()
        + size_of::<Source8FrameRealms<'static>>()
        + size_of::<Source8FrameCaps>()
        + 2 * size_of::<Source8InspectionRequest>()
        + 3 * size_of::<paintcraft_core::types::resources::ScopeCapacity>()
        + 3 * size_of::<BudgetAccount>()
        + size_of::<[u64; 16]>()
        + 4 * 128
        + 64
        + 8 * size_of::<&BudgetScope>()
        // Added pre-H W/F/O snapshots, whole-result movements, tuple input
        // formation and selected-workspace references, BEFORE their helpers.
        + 3 * size_of::<paintcraft_core::types::resources::ScopeCapacity>()
        + 3 * 128
        + size_of::<[(&BudgetScope, u64); 3]>()
        + 2 * size_of::<&BudgetScope>();
    if n != if native() { 5192 } else { 4336 } || n as u64 > owner_ledger::ENTRY_BYTES {
        return Err(extent());
    }
    Ok(n as u64)
}
pub(super) fn view_named() -> Result<u64> {
    let n = size_of::<ServiceView8>()
        + 2 * size_of::<caller::RequiredCaller8>()
        + 2 * size_of::<entry::EntryAuth8<'static>>()
        + 2 * 448
        + 256
        + 288
        // One128 slot also bounds the inner patch Result<Image>; the112
        // slot bounds its outer public-to-private wrapper return.
        + 2 * 128
        + 112
        + size_of::<ServiceOriginals8<'static>>()
        + size_of::<Source8FrameRealms<'static>>()
        + size_of::<Source8DisplayPatch>()
        + size_of::<display_public::OriginalControl8<'static>>()
        + size_of::<paintcraft_core::types::resources::ScopeCapacity>()
        + size_of::<[u64; 16]>()
        + 2 * size_of::<BudgetAccount>()
        + 2 * size_of::<&ServiceSource8>()
        + 2 * size_of::<&dyn Fn() -> Result<()>>()
        + 8 * size_of::<&BudgetScope>()
        + 64;
    if n != if native() { 3808 } else { 3240 } || n as u64 > VIEW_CONTROL {
        return Err(extent());
    }
    Ok(n as u64)
}
pub(super) fn validate() -> Result<()> {
    if !(native() || cfg!(target_arch = "wasm32"))
        || size_of::<OwnedReadFactories8>() != if native() { 224 } else { 112 }
        || size_of::<ServiceSource8>() != if native() { 376 } else { 240 }
        || size_of::<ServiceView8>() != if native() { 408 } else { 280 }
        || size_of::<ServiceOriginals8<'static>>() != if native() { 32 } else { 24 }
        || size_of::<Result<ServiceSource8>>() > 384
        || size_of::<Result<ServiceView8>>() > 448
        || size_of::<Result<caller::RequiredCaller8>>() > 256
        || size_of::<Result<entry::EntryAuth8<'static>>>() > 288
        || size_of::<Result<Source8Inspection>>() > 160
        || size_of::<Result<u64>>() > 128
        || size_of::<Result<()>>() > 128
        || size_of::<Result<Source8RgbaImage>>() > 112
    {
        return Err(extent());
    }
    transfer_named()?;
    begin_named()?;
    view_named()?;
    Ok(())
}

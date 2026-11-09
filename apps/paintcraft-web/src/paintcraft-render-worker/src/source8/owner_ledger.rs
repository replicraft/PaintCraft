use super::*;
use std::mem::{align_of, size_of};
pub(super) const ENTRY_BYTES: u64 = 8192;
fn control_error() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source8 read controls exceed the target declaration",
    )
}
fn arc_bytes<T>() -> Result<u64> {
    let align = align_of::<T>().max(align_of::<usize>());
    let offset = (2 * size_of::<usize>())
        .checked_next_multiple_of(align_of::<T>())
        .ok_or_else(extent)?;
    offset
        .checked_add(size_of::<T>())
        .and_then(|n| n.checked_next_multiple_of(align))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(extent)
}
pub(super) fn retained_bytes() -> Result<u64> {
    arc_bytes::<owner::SourcePayload8>()?
        .checked_add(arc_bytes::<owner::CarrierCredit8>()?)
        .ok_or_else(extent)
}
pub(super) fn validate_layouts() -> Result<()> {
    let native = cfg!(target_pointer_width = "64");
    if !native && !cfg!(target_arch = "wasm32") {
        return Err(control_error());
    }
    if size_of::<Source8ReadCaps>() != 64
        || size_of::<Source8ReadScopes<'static>>() != if native { 64 } else { 32 }
        || size_of::<Source8ReadRealms<'static>>() != if native { 128 } else { 96 }
        || size_of::<Source8InspectionRequest>() != if native { 96 } else { 80 }
        || size_of::<caller::RequiredCaller8>() != if native { 248 } else { 168 }
        || size_of::<entry::EntryAuth8<'static>>() != if native { 264 } else { 176 }
        || size_of::<entry::SourceSlot8>() != if native { 24 } else { 12 }
        || size_of::<entry::Entry8<'static>>() != if native { 552 } else { 392 }
        || size_of::<owner::Retirement8<'static>>() != if native { 16 } else { 8 }
        || size_of::<owner::RetainedFactories8>() != if native { 160 } else { 80 }
        || size_of::<owner::SourcePayload8>() != if native { 2888 } else { 1672 }
        || size_of::<owner::CarrierCredit8>() != if native { 72 } else { 40 }
        || size_of::<Source8Inspection>() != 144
        || size_of::<build::GuardedBuild8>() != if native { 392 } else { 240 }
        || size_of::<build::FreshLayerMap8>() != if native { 48 } else { 32 }
        || size_of::<paintcraft_core::types::resources::ScopeCapacity>() != 24
        || size_of::<Error>() > 128
    {
        return Err(control_error());
    }
    // One declared scalar setup frame, no heap or dynamic quotation table.
    let named = 2 * size_of::<owner::SourcePayload8>()
        + size_of::<build::GuardedBuild8>()
        + size_of::<entry::Entry8<'static>>()
        + 2 * size_of::<owner::CarrierCredit8>()
        + size_of::<owner::Retirement8<'static>>()
        + size_of::<Source8Inspection>()
        + 256 // ONLY audited new static diagnostics/inline Error controls
        + size_of::<build::FreshLayerMap8>()
        + 4 * size_of::<usize>() // both logical Arc headers
        + size_of::<[u64; 16]>()
        + 2 * size_of::<&entry::EntryAuth8<'static>>()
        + size_of::<paintcraft_core::types::resources::ScopeCapacity>()
        + size_of::<Source8ReadScopes<'static>>();
    let named = named
        + 2 * size_of::<probe_ledger::Facts>()
        + size_of::<[u64; 12]>()
        + size_of::<[u64; 16]>()
        + 2 * size_of::<usize>()
        + 4 * size_of::<&[u64]>()
        + size_of::<paintcraft_formats::pcnd::RetainedSourceLayoutSummary8>();
    if named != if native { 8160 } else { 5232 } || named as u64 > ENTRY_BYTES {
        return Err(control_error());
    }
    Ok(())
}

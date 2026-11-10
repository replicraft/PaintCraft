//! Target declarations and prospective refusal only; no storage grant.
use super::super::{
    BlendPlanRecordUpperBounds, CountedBlendSnapshot, CountedBlendSnapshotBytes, limit,
};
use super::BlendSourceDemand;
use crate::pcnd::{
    CheckedContainer, PortableBlockView, PortableHeaderView, PortableLayerCursor,
    PortableRetainedSourceLayout8, PortableSource8ControlBytes, PortableSource8Limits,
    PortableSource8MaterializerControlBytes, PortableSource8Plan, RequirementsLimits,
    RetainedSourceLayoutSummary8,
    layer_blend_appearance::{self as appearance, Source8ProbeWorkFacts, SourceCoverage8},
    portable_source8_control_bytes, portable_source8_materializer_control_bytes,
};
use paintcraft_core::types::{
    Error, Result,
    resources::{BudgetAccount, ScopeCapacity, ScopeFootprint},
};
use std::mem::size_of;
pub(super) const QUERY_CONTROL: u64 = 32768;
pub(super) const MAIN_MAX: u64 = 64 << 20;
pub(super) fn exhausted() -> Error {
    limit("Source8 Main source forecast allowance")
}
pub(super) fn validate() -> Result<()> {
    let named = 2 * size_of::<BlendPlanRecordUpperBounds>()
        + 2 * size_of::<Result<BlendPlanRecordUpperBounds>>()
        + 2 * size_of::<Peaks>()
        + 2 * size_of::<Result<Peaks>>()
        + 2 * size_of::<PortableSource8Limits>()
        + 2 * size_of::<RequirementsLimits>()
        + 2 * size_of::<sha2::Sha256>()
        + 2 * size_of::<[u8; 32]>()
        + 2 * size_of::<Result<PortableSource8MaterializerControlBytes>>()
        + 2 * size_of::<PortableLayerCursor<'static, 'static>>()
        + 2 * size_of::<Result<PortableLayerCursor<'static, 'static>>>()
        + 2 * size_of::<PortableBlockView<'static>>()
        + 2 * size_of::<Result<PortableBlockView<'static>>>()
        + 2 * size_of::<BlendSourceDemand>()
        + 2 * size_of::<Result<(CountedBlendSnapshotBytes, BlendSourceDemand)>>()
        + 2 * size_of::<Result<SourceCoverage8>>()
        + size_of::<SourceCoverage8>()
        + 2 * size_of::<Result<PortableSource8Plan<'static>>>()
        + size_of::<PortableSource8Plan<'static>>()
        + 2 * size_of::<Result<PortableRetainedSourceLayout8<'static, 'static>>>()
        + size_of::<PortableRetainedSourceLayout8<'static, 'static>>()
        + 2 * size_of::<RetainedSourceLayoutSummary8>()
        + 2 * size_of::<CheckedContainer<'static>>()
        + 2 * size_of::<Result<(CheckedContainer<'static>, [u8; 32])>>()
        + 2 * size_of::<PortableHeaderView<'static>>()
        + 2 * size_of::<Result<PortableHeaderView<'static>>>()
        + 2 * size_of::<BudgetAccount>()
        + 4 * size_of::<ScopeCapacity>()
        + 2 * size_of::<ScopeFootprint>()
        + 4 * size_of::<PortableSource8ControlBytes>()
        + 2 * size_of::<PortableSource8MaterializerControlBytes>()
        + 2 * size_of::<Source8ProbeWorkFacts>()
        + size_of::<[u64; 16]>()
        + size_of::<[usize; 8]>()
        + 8 * size_of::<&[u8]>()
        + 8 * size_of::<&u8>()
        + 4 * size_of::<Result<u64>>()
        + 4 * size_of::<Result<()>>();
    if !matches!(size_of::<usize>(), 4 | 8)
        || named as u64 > QUERY_CONTROL
        || size_of::<Result<u64>>() > 112
        || size_of::<Result<()>>() > 112
        || size_of::<Result<PortableLayerCursor<'static, 'static>>>() > 112
        || size_of::<Result<PortableBlockView<'static>>>() > 112
        || size_of::<PortableLayerCursor<'static, 'static>>() > 64
        || size_of::<PortableBlockView<'static>>() > 64
    {
        return Err(exhausted());
    }
    Ok(())
}
pub(super) struct Peaks {
    pub records: u64,
    pub census: u64,
    pub probe: u64,
    pub peak: u64,
}
pub(super) fn peaks(layers: u64, tiles: u64) -> Result<Peaks> {
    let q = portable_source8_control_bytes();
    let m = portable_source8_materializer_control_bytes()?;
    let records = q
        .plan
        .checked_add(q.layer_record.checked_mul(layers).ok_or_else(exhausted)?)
        .and_then(|n| {
            q.block_record
                .checked_mul(tiles)
                .and_then(|b| n.checked_add(b))
        })
        .ok_or_else(exhausted)?;
    let census = records
        .checked_add(q.scan)
        .and_then(|n| n.checked_add(m.retained_layout))
        .ok_or_else(exhausted)?;
    let probe = records
        .checked_add(appearance::transient_quote(
            usize::try_from(layers).map_err(|_| exhausted())?,
        )?)
        .ok_or_else(exhausted)?;
    Ok(Peaks {
        records,
        census,
        probe,
        peak: census.max(probe),
    })
}
pub(super) fn ensure_room(fact: &CountedBlendSnapshot<'_>, extra: u64) -> Result<()> {
    // Current occupancy can only refuse prospective work, never form a new grant.
    (fact.spend)(224)?;
    let cap = fact.scope.capacity_snapshot();
    let live = fact.scope.footprint().live;
    if !cap.reserved
        || cap.ceiling > MAIN_MAX
        || live.checked_add(extra).is_none_or(|n| n > cap.ceiling)
    {
        return Err(exhausted());
    }
    Ok(())
}

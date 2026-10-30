//! Conservative logical admission before PCND can allocate decoded metadata.
use super::*;
use paintcraft_document::{FilterInstance, FilterStack, Layer, filters::FILTERS_MAX};
use std::mem::size_of;

fn overflow() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Browser filter workspace overflow",
    )
}
pub(crate) fn filter_storage_ceiling(layers: usize) -> Result<u64> {
    (layers as u64)
        .checked_mul(FILTERS_MAX as u64)
        .and_then(|n| n.checked_mul(size_of::<FilterInstance>() as u64))
        .ok_or_else(overflow)
}
/// The 8MiB is the existing PCND aggregate wire-metadata ceiling, not a new
/// profile limit. Filter Vec storage is separate: wire descriptors are smaller
/// than their owned Rust representation. The 1MiB slot covers existing bounded
/// parser/inventory/control scratch. Source attachment retention is added by the
/// caller's unchanged SourceMetadataContext, with its own dynamic budget realm.
/// This is a logical payload quote, not allocator/browser/GPU/RSS measurement.
pub(crate) fn pre_read_quote(source: usize, limits: Limits) -> Result<u64> {
    let inline = (limits.layers as u64)
        .checked_mul((size_of::<Layer>() + size_of::<FilterStack>()) as u64)
        .ok_or_else(overflow)?;
    (source as u64)
        .checked_add(limits.resident)
        .and_then(|n| n.checked_add(8 << 20))
        .and_then(|n| n.checked_add(1 << 20))
        .and_then(|n| n.checked_add(inline))
        .and_then(|n| n.checked_add(filter_storage_ceiling(limits.layers).ok()?))
        .ok_or_else(overflow)
}

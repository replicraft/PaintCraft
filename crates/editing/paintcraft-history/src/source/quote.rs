//! Constant-shape retention quote; never walk source snapshots or infer old maps.
use super::*;
use crate::Entry;
use std::{
    alloc::Layout,
    mem::{align_of, size_of},
    sync::atomic::AtomicUsize,
};
/// Target-specific logical declarations, not allocator/RSS interception.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadSourceQuote {
    /// Actual retained Vec/Memo/Error payload upper, excluding shared documents.
    pub backing_bytes: u64,
    /// Backing plus the opaque retained owner's actual inline control fields.
    pub retained_bytes: u64,
    /// New shared allocation bytes beyond the already-covered retained fields.
    pub shared_control_bytes: u64,
    /// Complete new declaration for capturing a legacy/unpriced History source.
    pub capture_bytes: u64,
    /// Caller containing State/Frame field, not an extra allocated wrapper.
    pub handle_bytes: u64,
}
fn limit() -> Error {
    archive::limit("History source retention quote overflow")
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
pub(super) fn backing(history: &History) -> Result<u64> {
    let entries = history
        .entries
        .capacity()
        .checked_mul(size_of::<Entry>())
        .ok_or_else(limit)?;
    let mut bytes = add(
        u64::try_from(entries).map_err(|_| limit())?,
        history.memo.retained_allocation_upper(),
    )?;
    if let Some(error) = &history.last_cleanup_error {
        bytes = add(bytes, error.message.capacity() as u64)?;
    }
    if let Some(error) = &history.last_packing_error {
        bytes = add(bytes, error.message.capacity() as u64)?;
    }
    Ok(bytes)
}
pub(super) fn retained(history: &History) -> Result<u64> {
    add(
        backing(history)?,
        size_of::<RetainedHistoryStorage>() as u64,
    )
}
pub(super) fn arc_allocation<T>() -> Result<u64> {
    // Pinned Rust1.99 ArcInner is repr(C,align(2)), strong/weak Atomic<usize>, T.
    // AtomicUsize has the same reviewed pointer-atomic layout on native64/wasm32.
    if size_of::<AtomicUsize>() != size_of::<usize>()
        || align_of::<AtomicUsize>() != align_of::<usize>()
        || align_of::<AtomicUsize>() < 2
    {
        return Err(limit());
    }
    let (layout, _) = Layout::new::<[AtomicUsize; 2]>()
        .extend(Layout::new::<T>())
        .map_err(|_| limit())?;
    u64::try_from(layout.pad_to_align().size()).map_err(|_| limit())
}
pub(super) fn source(history: &History) -> Result<ReadSourceQuote> {
    let backing_bytes = backing(history)?;
    let retained_bytes = add(backing_bytes, size_of::<RetainedHistoryStorage>() as u64)?;
    let shared = arc_allocation::<SourceStorage>()?;
    let shared_control_bytes = shared
        .checked_sub(size_of::<RetainedHistoryStorage>() as u64)
        .ok_or_else(limit)?;
    Ok(ReadSourceQuote {
        backing_bytes,
        retained_bytes,
        shared_control_bytes,
        capture_bytes: add(retained_bytes, shared_control_bytes)?,
        handle_bytes: size_of::<HistoryReadSource>() as u64,
    })
}

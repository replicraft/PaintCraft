//! Checked legacy and copied-allocation bounds; no source traversal or new realm.
use super::*;
use paintcraft_raster::{TileVersion, raster::metadata_clone::fresh_map_layout};
use std::sync::Weak;
const LEGACY: usize = 64 << 20;
type Pair = (TileVersion, Weak<archive::Blob>);
pub(super) fn visit_work() -> u64 {
    (size_of::<TileVersion>() + 8) as u64
}
pub(super) fn clone_work() -> u64 {
    (size_of::<Pair>() + size_of::<TileVersion>()) as u64
}
/// The unchanged absent-key helper performs get_mut THEN insert. Each hashes
/// the thin Tile pointer once and can probe the complete fresh bucket extent.
/// These are named structural input/probe slots, not hasher CPU/time bounds.
pub(super) fn insertion_work(buckets: u64) -> Result<u64> {
    buckets
        .checked_mul(2)
        .and_then(|n| n.checked_add((2 * size_of::<usize>()) as u64))
        .ok_or_else(|| archive::limit("history fork insertion work overflow"))
}
pub(super) struct Quote {
    pub retained: u64,
    pub initialization: u64,
    pub error_bytes: u64,
    pub memo_capacity: usize,
    pub memo_buckets: u64,
    pub entry_capacity: usize,
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| archive::limit("history fork size overflow"))
}
fn multiply(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| archive::limit("history fork size overflow"))
}
pub(super) fn legacy(survivors: usize, entries: usize) -> Result<(usize, usize)> {
    let memo = multiply(survivors, add(size_of::<Pair>(), 160)?)?;
    if memo > LEGACY {
        return Err(archive::limit("history version index admission exceeded"));
    }
    let count = entries
        .checked_add(1)
        .ok_or_else(|| archive::limit("history entry count overflow"))?;
    let bytes = multiply(count, size_of::<Entry>())?;
    if bytes > LEGACY - memo {
        return Err(archive::limit("history publication staging exceeded"));
    }
    Ok((count, bytes))
}
fn messages(first: Option<&Error>, second: Option<&Error>, capacity: bool) -> Result<u64> {
    [first, second]
        .into_iter()
        .flatten()
        .try_fold(0u64, |sum, error| {
            let bytes = if capacity {
                error.message.capacity()
            } else {
                error.message.len()
            } as u64;
            sum.checked_add(bytes)
                .ok_or_else(|| archive::limit("history error size overflow"))
        })
}
impl Quote {
    pub fn new(source: &History, survivors: usize) -> Result<Self> {
        let (entry_capacity, entries) = legacy(survivors, source.entries.len())?;
        let memo = fresh_map_layout::<TileVersion, Weak<archive::Blob>>(survivors)?;
        let error_bytes = messages(
            source.last_cleanup_error.as_ref(),
            source.last_packing_error.as_ref(),
            false,
        )?;
        let initialization = memo
            .bytes
            .checked_add(entries as u64)
            .and_then(|n| n.checked_add(error_bytes))
            .ok_or_else(|| archive::limit("history fork initialization overflow"))?;
        let retained = initialization
            .checked_add(size_of::<ForkedHistory>() as u64)
            .ok_or_else(|| archive::limit("history fork retained size overflow"))?;
        Ok(Self {
            retained,
            initialization,
            error_bytes,
            memo_capacity: memo.capacity,
            memo_buckets: if memo.capacity == 0 {
                0
            } else if memo.capacity <= 7 {
                (memo.capacity + 1) as u64
            } else {
                (memo.capacity / 7 * 8) as u64
            },
            entry_capacity,
        })
    }
    pub fn check_errors(&self, first: Option<&Error>, second: Option<&Error>) -> Result<()> {
        // Pinned String::clone allocates its length, never source spare capacity.
        // Refuse unexpected future policy rather than accepting an underquote.
        if messages(first, second, true)? != self.error_bytes {
            return Err(archive::limit("history fork error clone capacity changed"));
        }
        Ok(())
    }
}

//! Retained std table provenance; no raw structural mutation or Entry escape.
use super::*;
use std::{collections::hash_map, ops::Index};
/// Actual retained table facts, not a budget, compactness or source-validity proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedTileTableQuote {
    pub allocation_upper: u64,
    pub bucket_upper: usize,
    pub live_entries: usize,
    /// Named full retained extent allowance; per-entry/helper work is separate.
    pub scan_work_upper: u64,
}
#[derive(Clone, Copy, Debug, Default)]
struct Bound {
    bytes: u64,
    buckets: usize,
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "raster tile table extent")
}
fn fresh(entries: usize) -> Result<Bound> {
    let layout = metadata_clone::fresh_map_layout::<TileCoord, Arc<Tile>>(entries)?;
    let buckets = match layout.capacity {
        0 => 0,
        n if n <= 7 => n.checked_add(1).ok_or_else(limit)?,
        n => (n / 7).checked_mul(8).ok_or_else(limit)?,
    };
    Ok(Bound {
        bytes: layout.bytes,
        buckets,
    })
}
fn full_capacity(buckets: usize) -> usize {
    match buckets {
        0 => 0,
        1..=8 => buckets - 1,
        n => n / 8 * 7,
    }
}
/// repr(C) makes the new inline control declaration's field layout explicit.
#[derive(Debug)]
#[repr(C)]
pub(super) struct TileTable {
    values: HashMap<TileCoord, Arc<Tile>>,
    allocation_upper: u64,
    bucket_upper: usize,
}
impl Clone for TileTable {
    fn clone(&self) -> Self {
        // Pinned RawTable::clone copies source buckets/controls, even len==0.
        // A future compacting clone needs <=this bound plus growth requalification.
        Self {
            values: self.values.clone(),
            allocation_upper: self.allocation_upper,
            bucket_upper: self.bucket_upper,
        }
    }
}
impl Default for TileTable {
    fn default() -> Self {
        Self::new()
    }
}
impl TileTable {
    pub(super) fn new() -> Self {
        Self {
            values: HashMap::new(),
            allocation_upper: 0,
            bucket_upper: 0,
        }
    }
    pub(super) fn len(&self) -> usize {
        self.values.len()
    }
    pub(super) fn capacity(&self) -> usize {
        self.values.capacity()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub(super) fn get(&self, key: &TileCoord) -> Option<&Arc<Tile>> {
        self.values.get(key)
    }
    pub(super) fn get_mut(&mut self, key: &TileCoord) -> Option<&mut Arc<Tile>> {
        self.values.get_mut(key)
    }
    pub(super) fn contains_key(&self, key: &TileCoord) -> bool {
        self.values.contains_key(key)
    }
    pub(super) fn iter(&self) -> hash_map::Iter<'_, TileCoord, Arc<Tile>> {
        self.values.iter()
    }
    pub(super) fn keys(&self) -> hash_map::Keys<'_, TileCoord, Arc<Tile>> {
        self.values.keys()
    }
    pub(super) fn values(&self) -> hash_map::Values<'_, TileCoord, Arc<Tile>> {
        self.values.values()
    }
    pub(super) fn retained_quote(&self) -> Result<RetainedTileTableQuote> {
        Ok(RetainedTileTableQuote {
            allocation_upper: self.allocation_upper,
            bucket_upper: self.bucket_upper,
            live_entries: self.len(),
            scan_work_upper: self
                .allocation_upper
                .checked_add(self.bucket_upper as u64)
                .ok_or_else(limit)?,
        })
    }
    pub(super) fn try_reserve(&mut self, additional: usize) -> Result<()> {
        let spare = self.capacity().checked_sub(self.len()).ok_or_else(limit)?;
        if additional <= spare {
            return Ok(());
        }
        let required = self.len().checked_add(additional).ok_or_else(limit)?;
        let full = full_capacity(self.bucket_upper);
        let possible = if full != 0 && required <= full / 2 {
            Bound {
                bytes: self.allocation_upper,
                buckets: self.bucket_upper,
            }
        } else {
            fresh(required.max(full.checked_add(1).ok_or_else(limit)?))?
        };
        self.values.try_reserve(additional).map_err(|_| limit())?;
        // Only an actual successful reserve with additional>old growth_left
        // normalizes DELETED tags (in-place or fresh resize), restoring full C.
        let observed = fresh(self.values.capacity());
        match observed {
            Ok(bound)
                if full_capacity(bound.buckets) == self.values.capacity()
                    && bound.buckets >= self.bucket_upper
                    && bound.buckets <= possible.buckets
                    && bound.bytes <= possible.bytes =>
            {
                self.allocation_upper = bound.bytes;
                self.bucket_upper = bound.buckets;
                Ok(())
            }
            _ => {
                // The pinned precomputed branch still covers retained storage on
                // refusal; never derive a smaller old extent after deletion.
                self.allocation_upper = self.allocation_upper.max(possible.bytes);
                self.bucket_upper = self.bucket_upper.max(possible.buckets);
                Err(limit())
            }
        }
    }
    pub(super) fn insert_reserved(
        &mut self,
        key: TileCoord,
        value: Arc<Tile>,
    ) -> Result<Option<Arc<Tile>>> {
        // std insert's raw helper reserves BEFORE lookup, even on replacement.
        // Avoid that implicit growth when no growth_left remains.
        if let Some(old) = self.values.get_mut(&key) {
            return Ok(Some(std::mem::replace(old, value)));
        }
        if self.len() == self.capacity() {
            return Err(limit());
        }
        Ok(self.values.insert(key, value))
    }
    pub(super) fn get_or_try_insert_with(
        &mut self,
        key: TileCoord,
        make: impl FnOnce() -> Result<Arc<Tile>>,
    ) -> Result<&mut Arc<Tile>> {
        if self.len() == self.capacity() && !self.contains_key(&key) {
            self.try_reserve(1).map_err(|_| {
                Error::new(ErrorCode::ResourceLimit, "cannot allocate raster index")
            })?;
        }
        // No Entry/guard escapes: the mutable value cannot grow the index.
        match self.values.entry(key) {
            hash_map::Entry::Occupied(entry) => Ok(entry.into_mut()),
            hash_map::Entry::Vacant(entry) => Ok(entry.insert(make()?)),
        }
    }
    pub(super) fn remove(&mut self, key: &TileCoord) -> Option<Arc<Tile>> {
        self.values.remove(key)
    }
}
impl<'a> IntoIterator for &'a TileTable {
    type Item = (&'a TileCoord, &'a Arc<Tile>);
    type IntoIter = hash_map::Iter<'a, TileCoord, Arc<Tile>>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}
impl Index<&TileCoord> for TileTable {
    type Output = Arc<Tile>;
    fn index(&self, key: &TileCoord) -> &Self::Output {
        &self.values[key]
    }
}
impl RasterLayer {
    /// O(1) actual retained index upper; not a fresh-layout/admission grant.
    pub fn retained_tile_table_quote(&self) -> Result<RetainedTileTableQuote> {
        self.tiles.retained_quote()
    }
}
#[cfg(test)]
mod tests;

mod portable;

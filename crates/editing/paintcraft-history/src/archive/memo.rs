//! Private structural mutation boundary for the pinned std HashMap policy.
use super::{Blob, TileVersion, Weak, limit};
use paintcraft_core::types::Result;
use paintcraft_raster::raster::metadata_clone::fresh_map_layout;
use std::{
    collections::{
        HashMap,
        hash_map::{Iter, Keys, RandomState, ValuesMut},
    },
    hash::{BuildHasher, Hash},
};

pub(crate) type Memo = TrackedMap<TileVersion, Weak<Blob>>;
/// No raw-map constructor, mutable dereference, Entry or structural iterator escapes.
#[derive(Debug)]
pub(crate) struct TrackedMap<K, V, S = RandomState> {
    values: HashMap<K, V, S>,
    allocation_upper: u64,
    bucket_upper: usize,
}
#[derive(Clone, Copy)]
struct Bound {
    bytes: u64,
    buckets: usize,
}
fn bound<K, V>(reported_capacity: usize) -> Result<Bound> {
    let layout = fresh_map_layout::<K, V>(reported_capacity)?;
    let buckets = if layout.capacity == 0 {
        0
    } else if layout.capacity < 8 {
        layout.capacity + 1
    } else {
        (layout.capacity / 7)
            .checked_mul(8)
            .ok_or_else(|| limit("memo bucket extent"))?
    };
    Ok(Bound {
        bytes: layout.bytes,
        buckets,
    })
}
impl<K: Eq + Hash, V> Default for TrackedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: Eq + Hash, V> TrackedMap<K, V> {
    pub(crate) fn new() -> Self {
        Self {
            values: HashMap::new(),
            allocation_upper: 0,
            bucket_upper: 0,
        }
    }
}
impl<K: Eq + Hash, V, S: BuildHasher> TrackedMap<K, V, S> {
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
    /// Proven retained table extent, including tombstones and empty buckets.
    /// This is modeled traversal work, not a preemptible iterator quantum.
    pub(crate) fn scan_work_upper(&self) -> Result<u64> {
        self.allocation_upper
            .checked_add(self.bucket_upper as u64)
            .ok_or_else(|| limit("history memo scan work overflow"))
    }

    pub(crate) fn capacity(&self) -> usize {
        self.values.capacity()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.values.get(key)
    }
    pub(crate) fn iter(&self) -> Iter<'_, K, V> {
        self.values.iter()
    }
    pub(crate) fn keys(&self) -> Keys<'_, K, V> {
        self.values.keys()
    }
    pub(crate) fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        self.values.values_mut()
    }
    pub(crate) fn retain(&mut self, keep: impl FnMut(&K, &mut V) -> bool) {
        self.values.retain(keep);
        // Deletions can reduce reported capacity without releasing the table.
    }
    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.values.clear();
    }
    pub(crate) fn metadata_weight(&self, slot_weight: usize) -> u64 {
        let old = (self.capacity() as u64).saturating_mul(slot_weight as u64);
        // Keep the historical live weak/control allowance when the real table
        // bound dominates. Tuple fields already belong to allocation_upper.
        let live_slack = (self.len() as u64)
            .saturating_mul(slot_weight.saturating_sub(std::mem::size_of::<(K, V)>()) as u64);
        old.max(self.allocation_upper.saturating_add(live_slack))
    }
    fn full_capacity_upper(&self) -> usize {
        if self.bucket_upper == 0 {
            0
        } else if self.bucket_upper <= 8 {
            self.bucket_upper - 1
        } else {
            self.bucket_upper / 8 * 7
        }
    }
    pub(crate) fn needs_reclaim(&self) -> bool {
        let full = self.full_capacity_upper();
        full > 16 && self.len().saturating_mul(4) < full
    }
    pub(crate) fn try_reserve(&mut self, additional: usize) -> Result<()> {
        let required = self
            .len()
            .checked_add(additional)
            .ok_or_else(|| limit("memo reserve extent"))?;
        if required <= self.capacity() {
            // Pinned try_reserve cannot allocate or rehash when growth_left suffices.
            return Ok(());
        }
        let old = Bound {
            bytes: self.allocation_upper,
            buckets: self.bucket_upper,
        };
        let full = self.full_capacity_upper();
        // Precompute a safe fallback BEFORE allocation. Pinned reserve can rehash
        // in place or resize to max(required, old full_capacity + 1).
        let possible = bound::<K, V>(
            required.max(
                full.checked_add(1)
                    .ok_or_else(|| limit("memo growth extent"))?,
            ),
        )?;
        self.values
            .try_reserve(additional)
            .map_err(|_| limit("cannot reserve history memo"))?;
        // If a later policy reconciliation fails, the grown table remains covered.
        self.allocation_upper = old.bytes.max(possible.bytes);
        self.bucket_upper = old.buckets.max(possible.buckets);
        // This observation is ONLY immediately after a tracked successful reserve:
        // no growth => old proof remains; rehash/growth => no DELETED tags remain,
        // so reported capacity is full table capacity. Never infer an old table
        // allocation from a capacity observation made after arbitrary mutations.
        let observed = bound::<K, V>(self.capacity())?;
        self.allocation_upper = old.bytes.max(observed.bytes);
        self.bucket_upper = old.buckets.max(observed.buckets);
        Ok(())
    }
    pub(crate) fn insert_reserved(&mut self, key: K, value: V) -> Result<Option<V>> {
        if let Some(existing) = self.values.get_mut(&key) {
            return Ok(Some(std::mem::replace(existing, value)));
        }
        if self.len() == self.capacity() {
            return Err(limit("history memo insertion lacks reserved slot"));
        }
        // A distinct new key fits the already-admitted growth_left; no implicit reserve.
        Ok(self.values.insert(key, value))
    }
    pub(crate) fn ensure_default_reserved(&mut self, key: K) -> Result<()>
    where
        V: Default,
    {
        if !self.values.contains_key(&key) {
            let _ = self.insert_reserved(key, V::default())?;
        }
        Ok(())
    }
    pub(crate) fn move_from_reserved(&mut self, source: &mut Self) -> Result<()> {
        if !self.is_empty() || self.capacity() < source.len() {
            return Err(limit("history memo replacement lacks reserved slots"));
        }
        // Check the complete count BEFORE touching source. Existing keys are
        // unique and destination is empty, so every insertion fits this reserve.
        // Source drain keeps its old table/provenance until actual replacement.
        for (key, value) in source.values.drain() {
            self.values.insert(key, value);
        }
        Ok(())
    }
    #[cfg(test)]
    fn with_hasher(hasher: S) -> Self {
        Self {
            values: HashMap::with_hasher(hasher),
            allocation_upper: 0,
            bucket_upper: 0,
        }
    }
}
#[cfg(test)]
mod tests;

use super::*;
use std::{
    hash::{BuildHasherDefault, Hasher},
    mem::size_of,
};
#[derive(Default)]
struct ZeroHasher;
impl Hasher for ZeroHasher {
    fn write(&mut self, _: &[u8]) {}
    fn finish(&self) -> u64 {
        0
    }
}
type Cluster = TrackedMap<u64, u64, BuildHasherDefault<ZeroHasher>>;
fn cluster() -> Cluster {
    Cluster::with_hasher(BuildHasherDefault::default())
}
#[test]
fn deterministic_tombstones_keep_allocation_quote_when_reported_capacity_falls() {
    let mut memo = cluster();
    memo.try_reserve(1024).unwrap();
    let full = memo.capacity();
    for key in 0..full as u64 {
        let _ = memo.insert_reserved(key, key + 1).unwrap();
    }
    let allocation = memo.allocation_upper;
    let buckets = memo.bucket_upper;
    memo.retain(|key, _| *key == full as u64 / 2);
    assert_eq!(memo.len(), 1);
    assert!(
        memo.capacity() < full,
        "constant-hash long cluster must produce tombstones"
    );
    let slot = size_of::<(u64, u64)>() + 160;
    let old_weight = memo.capacity() as u64 * slot as u64;
    assert!(
        old_weight < allocation,
        "this case must expose actual old undercount"
    );
    assert_eq!(
        memo.metadata_weight(slot),
        allocation + memo.len() as u64 * 160
    );
    assert_eq!(
        (memo.allocation_upper, memo.bucket_upper),
        (allocation, buckets)
    );
    assert_eq!(memo.get(&(full as u64 / 2)), Some(&(full as u64 / 2 + 1)));
    memo.clear();
    assert!(memo.is_empty());
    assert_eq!(
        (memo.allocation_upper, memo.bucket_upper),
        (allocation, buckets)
    );
    assert!(memo.metadata_weight(slot) >= allocation);
}
#[test]
fn insertion_requires_reserved_slots_and_existing_value_replacement_does_not_grow() {
    let mut memo = cluster();
    assert!(memo.insert_reserved(1, 2).is_err());
    assert!(memo.ensure_default_reserved(1).is_err());
    assert_eq!(
        (memo.len(), memo.capacity(), memo.allocation_upper),
        (0, 0, 0)
    );
    memo.try_reserve(1).unwrap();
    let capacity = memo.capacity();
    for key in 0..capacity as u64 {
        let _ = memo.insert_reserved(key, key).unwrap();
    }
    let allocation = memo.allocation_upper;
    assert!(memo.insert_reserved(999, 1).is_err());
    assert_eq!(memo.insert_reserved(0, 7).unwrap(), Some(0));
    memo.ensure_default_reserved(0).unwrap();
    assert_eq!(memo.get(&0), Some(&7));
    assert_eq!(
        (memo.len(), memo.capacity(), memo.allocation_upper),
        (capacity, capacity, allocation)
    );
}
#[test]
fn tracked_reserve_keeps_old_bound_and_reconciles_growth_without_reset() {
    let mut memo = cluster();
    assert_eq!(memo.metadata_weight(176), 0);
    memo.try_reserve(3).unwrap();
    for key in 0..3 {
        let _ = memo.insert_reserved(key, key).unwrap();
    }
    let first = memo.allocation_upper;
    memo.try_reserve(100).unwrap();
    assert!(memo.allocation_upper >= first);
    let observed = bound::<u64, u64>(memo.capacity()).unwrap();
    assert!(memo.allocation_upper >= observed.bytes && memo.bucket_upper >= observed.buckets);
    let before = (
        memo.len(),
        memo.capacity(),
        memo.allocation_upper,
        memo.bucket_upper,
    );
    memo.try_reserve(0).unwrap();
    assert_eq!(
        (
            memo.len(),
            memo.capacity(),
            memo.allocation_upper,
            memo.bucket_upper
        ),
        before
    );
    assert!(memo.try_reserve(usize::MAX).is_err());
    assert_eq!(
        (
            memo.len(),
            memo.capacity(),
            memo.allocation_upper,
            memo.bucket_upper
        ),
        before
    );
    assert_eq!(memo.get(&2), Some(&2));
    assert!(memo.metadata_weight(176) >= memo.capacity() as u64 * 176);
}
#[test]
fn replacement_checks_complete_reserve_before_source_move_and_transfers_fresh_witness() {
    let mut source = cluster();
    source.try_reserve(1024).unwrap();
    for key in 0..128 {
        let _ = source.insert_reserved(key, key + 3).unwrap();
    }
    source.retain(|key, _| *key < 2);
    let source_allocation = source.allocation_upper;
    let mut small = cluster();
    assert!(small.move_from_reserved(&mut source).is_err());
    assert_eq!(source.len(), 2);
    assert_eq!(source.get(&1), Some(&4));
    assert_eq!(source.allocation_upper, source_allocation);
    small.try_reserve(source.len()).unwrap();
    let fresh = small.allocation_upper;
    small.move_from_reserved(&mut source).unwrap();
    assert!(source.is_empty());
    assert_eq!(
        source.allocation_upper, source_allocation,
        "drain still owns old allocation"
    );
    assert_eq!(small.len(), 2);
    assert_eq!(small.get(&1), Some(&4));
    assert_eq!(small.allocation_upper, fresh);
    assert!(fresh < source_allocation);
    source = small; // Same replacement used by reclaim_tables: old table actually drops.
    assert_eq!(source.allocation_upper, fresh);
}
#[test]
fn iterators_and_mutable_values_cannot_change_structural_allocation() {
    let mut memo = cluster();
    memo.try_reserve(2).unwrap();
    let _ = memo.insert_reserved(1, 5).unwrap();
    let _ = memo.insert_reserved(2, 6).unwrap();
    let before = (
        memo.len(),
        memo.capacity(),
        memo.allocation_upper,
        memo.bucket_upper,
    );
    for value in memo.values_mut() {
        *value += 10;
    }
    assert_eq!(memo.keys().count(), 2);
    assert_eq!(memo.iter().map(|(_, v)| *v).sum::<u64>(), 31);
    assert_eq!(
        (
            memo.len(),
            memo.capacity(),
            memo.allocation_upper,
            memo.bucket_upper
        ),
        before
    );
}

#[test]
fn tombstone_hidden_table_triggers_reclaim_and_fresh_replacement_reduces_weight() {
    let mut memo = cluster();
    memo.try_reserve(1024).unwrap();
    let full = memo.capacity();
    for key in 0..full as u64 {
        let _ = memo.insert_reserved(key, key + 1).unwrap();
    }
    memo.retain(|key, _| *key == full as u64 / 2);
    assert_eq!(memo.len(), 1);
    let old_predicate = memo.capacity() > 16 && memo.len().saturating_mul(4) < memo.capacity();
    assert!(
        !old_predicate,
        "the actual deletion cluster must defeat reported-capacity reclaim"
    );
    assert!(
        memo.needs_reclaim(),
        "tracked allocation remains large despite tombstones"
    );
    let slot = size_of::<(u64, u64)>() + 160;
    let before = memo.metadata_weight(slot);
    assert_eq!(
        before,
        memo.allocation_upper + 160,
        "retain live weak/control slack"
    );
    let mut small = cluster();
    small.try_reserve(memo.len()).unwrap();
    small.move_from_reserved(&mut memo).unwrap();
    memo = small;
    assert_eq!(memo.get(&(full as u64 / 2)), Some(&(full as u64 / 2 + 1)));
    assert_eq!(memo.len(), 1);
    assert!(!memo.needs_reclaim());
    assert!(memo.metadata_weight(slot) < before);
    assert!(memo.metadata_weight(slot) >= memo.capacity() as u64 * slot as u64);
}

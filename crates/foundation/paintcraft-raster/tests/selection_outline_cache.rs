use paintcraft_core::types::{Rect, resources::SharedBudget};
use paintcraft_raster::selection::{BinaryMask, OutlineCache};
use std::sync::Arc;
#[test]
fn geometry_is_reused_until_field_or_view_changes_and_readers_own_capacity() {
    let budget = SharedBudget::new(1 << 16);
    let mut cache = OutlineCache::with_budget(budget.clone());
    let rect = Rect::new(0, 0, 12, 9).unwrap();
    let mask = BinaryMask::uniform(rect, true).unwrap();
    let a = cache.get(&mask, rect).unwrap();
    let bytes = budget.usage().reserved;
    assert!(bytes > 0);
    let b = cache.get(&mask.clone(), rect).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(budget.usage().reserved, bytes);
    let c = cache.get(&mask, Rect::new(2, 2, 7, 4).unwrap()).unwrap();
    assert!(
        c.segments().is_empty(),
        "a viewport inside a full field has no invented edges"
    );
    assert!(!Arc::ptr_eq(&a, &c));
    cache.clear();
    assert_eq!(budget.usage().reserved, bytes);
    drop(a);
    drop(b);
    drop(c);
    assert_eq!(budget.usage().reserved, 0);
    let empty = cache.get(&mask.inverted(), rect).unwrap();
    assert!(empty.segments().is_empty());
}
#[test]
fn a_cached_resource_denial_retries_after_shared_capacity_is_released() {
    let budget = SharedBudget::new(4096);
    let held = budget.reserve(4096).unwrap();
    let mut cache = OutlineCache::with_budget(budget.clone());
    let rect = Rect::new(0, 0, 8, 8).unwrap();
    let mask = BinaryMask::uniform(rect, true).unwrap();
    assert!(cache.get(&mask, rect).is_err());
    assert!(cache.get(&mask, rect).is_err());
    drop(held);
    assert_eq!(cache.get(&mask, rect).unwrap().segments().len(), 4);
    cache.clear();
    assert_eq!(budget.usage().reserved, 0);
}

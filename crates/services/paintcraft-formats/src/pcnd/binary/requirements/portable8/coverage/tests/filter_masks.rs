//! Independent PCND8 filter-mask bytes; no encoder or generic read fallback.
use super::*;
use paintcraft_core::types::json;
use paintcraft_raster::coverage_mask::{canonical_a8_storage_arc_quote, retained_quote};
fn rows() -> Vec<json::Value> {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/canonical-source8-coverage-fixtures/filter-wires.json"
    )))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn bytes(row: &json::Value) -> Vec<u8> {
    row.get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn n(row: &json::Value, key: &str) -> u64 {
    row.get(key).unwrap().as_i64().unwrap() as u64
}
#[test]
fn nonempty_filter_enabled_disabled_empty_and_repeated_own_are_actual_bound_masks() {
    for row in rows() {
        let bytes = bytes(&row);
        let original = bytes.clone();
        let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(16 << 20);
        let plan_scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let coverage_scope = budget
            .reserve_capacity(1 << 20)
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let mut guard = budget.account(AllocationClass::PaintMetadata);
        // A repeated method below temporarily creates a second independent mask,
        // even if only one stored occurrence exists; both full guards are explicit.
        guard
            .resize(2 * canonical_a8_storage_arc_quote().unwrap())
            .unwrap();
        let calls = Cell::new(0u64);
        let used = Cell::new(0u64);
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let spend = |v| {
            used.set(used.get() + v);
            Ok(())
        };
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &plan_scope, &check, &spend)
            .unwrap();
        let layer = plan.layer(1).unwrap();
        let view = layer.filter(0).unwrap();
        let mask_view = view.mask.unwrap();
        assert_eq!(
            mask_view.bands(),
            n(&row, "bands") - u64::from(row.get("own_present").unwrap().as_bool().unwrap())
        );
        assert_eq!(
            view.enabled,
            row.get("filter_enabled").unwrap().as_bool().unwrap()
        );
        let owner = plan.coverage_materializer(&coverage_scope, &guard).unwrap();
        let (enabled, mask) = owner.filter(1, 0).unwrap().unwrap();
        assert_eq!(enabled, row.get("mask_enabled").unwrap().as_bool().unwrap());
        assert_eq!(mask.coverage(0, 0), n(&row, "mask_value") as u8);
        assert_eq!(mask.coverage(1, 0), n(&row, "mask_value") as u8);
        assert_eq!(mask.stats().runs, usize::from(n(&row, "mask_value") != 0));
        let (_, repeat) = owner.filter(1, 0).unwrap().unwrap();
        if n(&row, "mask_value") != 0 {
            assert_ne!(
                mask.storage_usage().unwrap().0,
                repeat.storage_usage().unwrap().0
            );
            assert_eq!(mask.digest(), repeat.digest());
        } else {
            assert!(mask.storage_usage().is_none());
            assert!(repeat.storage_usage().is_none());
        }
        drop(repeat);
        if row.get("own_present").unwrap().as_bool().unwrap() {
            let (_, own) = owner.layer(1).unwrap().unwrap();
            assert_ne!(
                mask.storage_usage().unwrap().0,
                own.storage_usage().unwrap().0
            );
            assert_eq!(mask.digest(), own.digest());
            assert_eq!(mask.stats(), own.stats());
            drop(own);
        } else {
            assert!(owner.layer(1).unwrap().is_none());
        }
        assert_eq!(owner.filter(1, 1).unwrap_err().code, ErrorCode::CodecFailed);
        assert_eq!(plan.work_used(), used.get());
        assert_eq!(plan.check_calls(), calls.get());
        drop(mask);
        drop(owner);
        assert_eq!(coverage_scope.footprint().live, 0);
        drop(plan);
        assert_eq!(plan_scope.footprint().live, 0);
        assert_eq!(bytes, original);
    }
}
#[test]
fn sole_filter_alias_survives_original_factories_while_full_guard_is_still_held() {
    let row = rows().remove(0);
    let bytes = bytes(&row);
    let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(16 << 20);
    let plan_scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let coverage_scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let mut guard = budget.account(AllocationClass::PaintMetadata);
    let h = canonical_a8_storage_arc_quote().unwrap();
    guard.resize(h).unwrap();
    let check = || Ok(());
    let spend = |_| Ok(());
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &plan_scope, &check, &spend)
        .unwrap();
    let owner = plan.coverage_materializer(&coverage_scope, &guard).unwrap();
    let (_, mask) = owner.filter(1, 0).unwrap().unwrap();
    let alias = mask.clone();
    let a = retained_quote(1, 1).unwrap();
    drop(mask);
    drop(owner);
    assert_eq!(coverage_scope.footprint().live, a);
    drop(plan);
    drop(plan_scope);
    drop(coverage_scope);
    assert_eq!(budget.usage().reserved, a + h);
    assert_eq!(alias.coverage(0, 0), 128);
    assert_eq!(guard.bytes(), h);
    drop(alias);
    assert_eq!(budget.usage().reserved, h);
    drop(guard);
    assert_eq!(budget.usage().reserved, 0);
}

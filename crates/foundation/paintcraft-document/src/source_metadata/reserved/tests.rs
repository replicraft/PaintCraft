use super::*;
use paintcraft_core::types::{
    json,
    resources::{AllocationClass, SharedBudget},
};
use std::cell::Cell;
mod controls;
mod tagged;
fn rows() -> Vec<json::Value> {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/reserved-source-metadata-fixtures/oracle.json"
    )))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn bytes(row: &json::Value, key: &str) -> Vec<u8> {
    row.get(key)
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
fn expected_charge(profile: usize) -> u64 {
    if cfg!(target_pointer_width = "64") {
        488 + profile as u64
    } else {
        472 + profile as u64
    }
}
fn sentinel() -> Error {
    Error::new(ErrorCode::OperationCancelled, "original ICC sentinel").at(17)
}
#[test]
fn independent_target_defining_layout_and_control_policy_are_frozen() {
    let q = SourceMetadata::reserved_layout_quote().unwrap();
    let actual = (
        std::mem::size_of::<SourceDescriptor>(),
        std::mem::size_of::<Inner>(),
        q.inner_allocation_upper,
        q.lease_control_allocation_upper,
        std::mem::size_of::<ReservedSourceMetadataReader<'static>>(),
        std::mem::size_of::<SourceMetadataRecipe<'static, 'static, 'static>>(),
    );
    assert_eq!(
        actual,
        if cfg!(target_pointer_width = "64") {
            (120, 264, 280, 40, 104, 256)
        } else {
            (120, 248, 256, 24, 64, 240)
        }
    );
    assert_eq!(q.constructor_control_bytes, 4096);
    assert_eq!(
        SourceMetadata::reserved_retained_quote(0).unwrap(),
        expected_charge(0)
    );
    assert_eq!(
        SourceMetadata::reserved_retained_quote(132).unwrap(),
        expected_charge(132)
    );
}
#[test]
fn independent_wire_validation_precedes_retention_and_matches_old_semantics() {
    for row in rows() {
        let header = bytes(&row, "header_hex");
        let profile = bytes(&row, "profile_hex");
        let original = (header.clone(), profile.clone());
        let icc = SharedBudget::new(1 << 20);
        let reservation = icc
            .reserve_capacity(if cfg!(target_pointer_width = "64") {
                620
            } else {
                604
            })
            .unwrap();
        let source = SharedBudget::new(8192);
        let scope = source
            .reserve_capacity(8192)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let mut guard = scope.account();
        guard.resize(512).unwrap();
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || Ok(());
        let reader = ReservedSourceMetadataReader::new(
            &reservation,
            &scope,
            &guard,
            8 << 20,
            &check,
            &spend,
        )
        .unwrap();
        let available = reservation.available();
        {
            let result = reader.inspect(&header, &profile);
            if row.get("accept").unwrap().as_bool() == Some(true) {
                let recipe = result.unwrap();
                assert_eq!(recipe.profile_len(), profile.len());
                assert_eq!(
                    reservation.available(),
                    available,
                    "borrowed validation allocates no retained ICC"
                );
                let metadata = recipe.materialize().unwrap();
                assert_eq!(metadata.wire_header().as_slice(), header);
                assert_eq!(metadata.storage_usage().1, expected_charge(profile.len()));
                assert_eq!(
                    reservation.available(),
                    available - expected_charge(profile.len())
                );
                let old = SourceMetadata::from_wire(
                    &header,
                    &profile,
                    8 << 20,
                    &SharedBudget::new(1 << 20),
                    &|| Ok(()),
                )
                .unwrap();
                assert_eq!(metadata, old);
                assert_eq!(
                    metadata.profile().is_some(),
                    row.get("profile_present").unwrap().as_bool().unwrap()
                );
                drop(old);
                drop(metadata);
            } else {
                assert_eq!(result.unwrap_err().code, ErrorCode::InputSchema);
            }
        }
        assert_eq!(reservation.available(), available);
        assert_eq!((header, profile), original);
        assert!(used.get() > 0);
        drop(reader);
        assert_eq!(scope.footprint().live, 512);
        drop(guard);
        drop(scope);
        drop(reservation);
        assert_eq!(icc.usage().reserved, 0);
        assert_eq!(source.usage().reserved, 0);
    }
}
#[test]
fn resolution_only_inner_and_lease_survive_factory_and_reader_as_one_shared_owner() {
    let row = rows().remove(0);
    let header = bytes(&row, "header_hex");
    let icc = SharedBudget::new(1 << 20);
    let reservation = icc.reserve_capacity(expected_charge(0)).unwrap();
    let source = SharedBudget::new(8192);
    let scope = source
        .reserve_capacity(8192)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let mut guard = scope.account();
    guard.resize(512).unwrap();
    let check = || Ok(());
    let spend = |_| Ok(());
    let reader =
        ReservedSourceMetadataReader::new(&reservation, &scope, &guard, 8 << 20, &check, &spend)
            .unwrap();
    let metadata = {
        let recipe = reader.inspect(&header, &[]).unwrap();
        recipe.materialize().unwrap()
    };
    let alias = metadata.clone();
    assert!(metadata.profile().is_none());
    assert_eq!(metadata.storage_usage().1, expected_charge(0));
    drop(reader);
    drop(reservation);
    assert_eq!(icc.usage().reserved, expected_charge(0));
    drop(metadata);
    assert_eq!(icc.usage().reserved, expected_charge(0));
    drop(alias);
    assert_eq!(icc.usage().reserved, 0);
    assert_eq!(
        scope.footprint().live,
        512,
        "full M guard follows final actual alias"
    );
    drop(guard);
    drop(scope);
    assert_eq!(source.usage().reserved, 0);
}
#[test]
fn bounded_debug_invokes_no_callback_and_does_not_dump_header_or_profile() {
    let row = rows().remove(2);
    let header = bytes(&row, "header_hex");
    let profile = bytes(&row, "profile_hex");
    let icc = SharedBudget::new(1 << 20);
    let reservation = icc.reserve_capacity(1024).unwrap();
    let source = SharedBudget::new(8192);
    let scope = source.scope(AllocationClass::PaintMetadata);
    let mut guard = scope.account();
    guard.resize(512).unwrap();
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let spend = |_| Ok(());
    let reader =
        ReservedSourceMetadataReader::new(&reservation, &scope, &guard, 8 << 20, &check, &spend)
            .unwrap();
    let recipe = reader.inspect(&header, &profile).unwrap();
    let before = calls.get();
    let text = format!("{reader:?} {recipe:?}");
    assert!(text.len() < 256);
    assert!(!text.contains("PCSMETA"));
    assert_eq!(calls.get(), before);
}

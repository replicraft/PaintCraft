use super::*;
use crate::types::resources::AllocationClass;

#[test]
fn literal_target_factory_quotes_include_full_logical_arc_storage() {
    let actual = [
        SharedBudget::control_allocation_quote().unwrap(),
        BudgetReservation::control_allocation_quote().unwrap(),
        BudgetScope::control_allocation_quote().unwrap(),
    ];
    // Independent frozen literals. Other targets retain the defining-type API
    // but need their own audited target expectations before qualification.
    let expected = if cfg!(all(target_os = "macos", target_pointer_width = "64")) {
        Some([88, 56, 48])
    } else if cfg!(all(
        any(target_os = "linux", target_os = "windows"),
        target_pointer_width = "64"
    )) {
        Some([80, 48, 40])
    } else if cfg!(all(target_arch = "wasm32", target_os = "unknown")) {
        Some([64, 40, 32])
    } else {
        None
    };
    if let Some(expected) = expected {
        assert_eq!(actual, expected);
    }
    for bytes in actual {
        assert!(bytes >= (2 * size_of::<usize>()) as u64);
        assert!(bytes.is_multiple_of(align_of::<usize>() as u64));
    }
}

#[test]
fn checked_arc_layout_preserves_header_and_tail_alignment() {
    #[repr(align(64))]
    struct Empty;
    #[repr(C, align(64))]
    struct Byte {
        _value: u8,
    }
    assert_eq!(arc_bytes::<Empty>().unwrap(), 64);
    assert_eq!(arc_bytes::<Byte>().unwrap(), 128);
    assert_eq!(
        arc_bytes::<u8>().unwrap(),
        if usize::BITS == 64 { 24 } else { 12 }
    );
    assert_eq!(
        arc_bytes::<u64>().unwrap(),
        if usize::BITS == 64 { 24 } else { 16 }
    );
}

#[test]
fn pure_quotes_do_not_admit_release_clone_or_grow_original_storage() {
    let budget = SharedBudget::new(1000);
    let reserved = budget.reserve_capacity(37).unwrap();
    let scope = reserved.clone().scope(AllocationClass::PaintMetadata);
    let mut account = scope.account();
    account.resize(11).unwrap();
    let usage = budget.usage();
    let activity = budget.activity();
    let footprint = scope.footprint();
    let strong = std::sync::Arc::strong_count(&budget.0);
    assert_eq!(usage.reserved, 37);
    assert_eq!(reserved.available(), 26);
    for _ in 0..2 {
        SharedBudget::control_allocation_quote().unwrap();
        BudgetReservation::control_allocation_quote().unwrap();
        BudgetScope::control_allocation_quote().unwrap();
        assert_eq!(budget.usage(), usage);
        assert_eq!(budget.activity(), activity);
        assert_eq!(scope.footprint(), footprint);
        assert_eq!(std::sync::Arc::strong_count(&budget.0), strong);
        assert_eq!(reserved.available(), 26);
    }
    // A numeric quote did not grant more capacity to the original reservation.
    assert!(account.resize(38).is_err());
    assert_eq!(account.bytes(), 11);
    assert_eq!(reserved.available(), 26);
    drop(account);
    drop(scope);
    assert_eq!(budget.usage().reserved, 37);
    drop(reserved);
    assert_eq!(budget.usage().reserved, 0);
}

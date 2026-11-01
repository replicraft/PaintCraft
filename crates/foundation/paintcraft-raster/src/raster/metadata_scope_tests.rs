//! The caller scope is retained verbatim rather than replaced by a fresh realm.
use super::*;
#[test]
fn injected_metadata_scope_shares_reservation_footprint_and_denial() {
    let budget = SharedBudget::new(128);
    let reservation = budget.reserve_capacity(32).unwrap();
    let scope = reservation.scope(AllocationClass::PaintMetadata);
    let mut caller = scope.account();
    caller.resize(12).unwrap();
    let admission = Admission::new(0)
        .with_paint_metadata_limit(19)
        .with_paint_metadata_scope(scope.clone());
    assert_eq!(admission.paint_metadata_limit(), 19);
    let mut stage = admission.paint_metadata_account();
    stage.resize(20).unwrap();
    assert_eq!(scope.footprint().live, 32);
    assert_eq!(budget.usage().reserved, 32);
    assert_eq!(stage.resize(21).unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(stage.bytes(), 20);
    assert_eq!(scope.footprint().live, 32);
    assert_eq!(scope.footprint().peak, 33);
    caller.resize(0).unwrap();
    stage.resize(32).unwrap();
    assert_eq!(scope.footprint().live, 32);
    drop(stage);
    drop(admission);
    drop(scope);
    drop(caller);
    assert_eq!(budget.usage().reserved, 0);
}

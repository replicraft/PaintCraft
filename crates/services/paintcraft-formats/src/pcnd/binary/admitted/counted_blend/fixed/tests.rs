use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
#[test]
fn fixed_output_capacity_exact_and_minus_one_never_detach_or_borrow_another_realm() {
    let exact = if cfg!(target_pointer_width = "64") {
        168
    } else {
        104
    };
    for cap in [exact - 1, exact] {
        let budget = SharedBudget::new(65536);
        let reservation = budget.reserve_capacity(cap).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let result = allocate(&scope, 8, &|_| Ok(()));
        if cap == exact {
            let (owner, sink) = result.unwrap();
            assert_eq!(owner.as_bytes(), &[0; 8]);
            assert_eq!(scope.footprint().live, exact);
            assert_eq!(reservation.available(), 0);
            drop(owner);
            drop(sink);
        } else {
            assert_eq!(result.err().unwrap().code, ErrorCode::ResourceLimit);
        }
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(reservation.available(), cap);
    }
}

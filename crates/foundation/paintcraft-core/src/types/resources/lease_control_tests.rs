use super::*;
#[test]
fn complete_defining_lease_arc_quote_is_pure_and_target_frozen() {
    let budget = SharedBudget::new(0);
    let expected = if cfg!(target_pointer_width = "64") {
        40
    } else {
        24
    };
    assert_eq!(BudgetLease::control_allocation_quote().unwrap(), expected);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(
        std::mem::size_of::<Charge>(),
        if cfg!(target_pointer_width = "64") {
            24
        } else {
            16
        }
    );
}

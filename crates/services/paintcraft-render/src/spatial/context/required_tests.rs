use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};

#[test]
fn absent_required_capture_never_installs_or_admits_a_fallback() {
    assert!(capture_current_workspace().is_none());
    let unrelated = SharedBudget::new(4096);
    let before = unrelated.usage();
    assert_eq!(
        capture_required_workspace(128, 5).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(unrelated.usage(), before);
    assert!(capture_current_workspace().is_none());
}
#[test]
fn capture_minima_retain_prior_debt_original_scope_and_actual_tls() {
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let spare = budget.scope(AllocationClass::SharedBudget);
    let parent = WorkspaceAdmission::new(scope.clone(), 128, 11);
    parent.reserve_work(3).unwrap();
    let before = budget.usage();
    let child = with_workspace(parent.clone(), || {
        let child = capture_required_workspace(256, 5)?;
        assert!(child.same_job(&parent));
        assert_eq!(
            (child.bytes_limit(), child.work_limit(), child.work_used()),
            (128, 5, 3)
        );
        let installed = capture_current_workspace().unwrap();
        assert!(installed.same_job(&parent));
        assert_eq!((installed.bytes_limit(), installed.work_limit()), (128, 11));
        assert_eq!(budget.usage(), before);
        Ok(child)
    })
    .unwrap();
    assert!(capture_current_workspace().is_none());
    child.reserve_work(2).unwrap();
    assert_eq!(parent.work_used(), 5);
    assert_eq!(
        child.reserve_work(1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(parent.work_used(), 5);
    let mut account = child.scope().account();
    account.resize(32).unwrap();
    assert_eq!(scope.footprint().live, 32);
    assert_eq!(spare.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 32);
    drop(account);
    assert_eq!(budget.usage().reserved, 0);
    let foreign = WorkspaceAdmission::new(scope, 128, 11);
    assert!(!foreign.same_job(&child));
}
#[test]
fn tightening_exhausted_and_zero_requests_preserves_caller_counter_and_context() {
    let budget = SharedBudget::new(4096);
    let parent = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 128, 11);
    parent.reserve_work(6).unwrap();
    with_workspace(parent.clone(), || {
        let before = budget.usage();
        assert_eq!(
            capture_required_workspace(64, 5).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(parent.work_used(), 6);
        let current = capture_current_workspace().unwrap();
        assert!(current.same_job(&parent));
        assert_eq!((current.bytes_limit(), current.work_limit()), (128, 11));
        assert_eq!(budget.usage(), before);
        Ok(())
    })
    .unwrap();
    let zero_parent = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 128, 11);
    with_workspace(zero_parent.clone(), || {
        let before = budget.usage();
        let zero = capture_required_workspace(0, 0)?;
        assert!(zero.same_job(&zero_parent));
        assert_eq!(
            (zero.bytes_limit(), zero.work_limit(), zero.work_used()),
            (0, 0, 0)
        );
        assert_eq!(
            zero.reserve_work(1).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(zero_parent.work_used(), 0);
        assert_eq!(capture_current_workspace().unwrap().work_limit(), 11);
        assert_eq!(budget.usage(), before);
        Ok(())
    })
    .unwrap();
    assert!(capture_current_workspace().is_none());
}
#[test]
fn required_base_capture_preserves_general_matched_noise_context() {
    use crate::noise_work::{
        NoiseAdmission, NoiseJobPolicy, NoiseLimits, capture_current_noise_workspace,
        with_noise_workspace,
    };
    let budget = SharedBudget::new(4096);
    let pair = NoiseAdmission::new_root(
        budget.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::BrowserV1,
        NoiseLimits {
            bytes: 4096,
            base_work: 11,
            noise_work: 17,
        },
    )
    .unwrap();
    with_noise_workspace(pair.clone(), || {
        let before = budget.usage();
        let child = capture_required_workspace(128, 7)?;
        assert!(child.same_job(pair.workspace()));
        assert_eq!((child.bytes_limit(), child.work_limit()), (128, 7));
        let noise = capture_current_noise_workspace().unwrap();
        assert!(noise.same_job(&pair));
        assert_eq!(noise.limits(), pair.limits());
        assert_eq!(budget.usage(), before);
        Ok(())
    })
    .unwrap();
    assert!(capture_current_workspace().is_none());
    assert!(capture_current_noise_workspace().is_none());
}

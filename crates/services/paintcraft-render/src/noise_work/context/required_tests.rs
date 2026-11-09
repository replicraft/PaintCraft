use super::*;
use crate::spatial::{WorkspaceAdmission, capture_current_workspace, with_workspace};
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};

fn limits() -> NoiseLimits {
    NoiseLimits {
        bytes: 4096,
        base_work: 11,
        noise_work: 17,
    }
}
fn owner() -> (SharedBudget, NoiseAdmission) {
    let budget = SharedBudget::new(4096);
    let pair = NoiseAdmission::new_root(
        budget.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::BrowserV1,
        limits(),
    )
    .unwrap();
    (budget, pair)
}
#[test]
fn required_noise_rejects_absence_and_both_half_contexts_without_installation() {
    assert!(capture_current_workspace().is_none());
    assert!(capture_current_noise_workspace().is_none());
    assert_eq!(
        capture_required_noise_workspace(limits()).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let budget = SharedBudget::new(4096);
    let base = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 4096, 11);
    with_workspace(base.clone(), || {
        let before = budget.usage();
        assert_eq!(
            capture_required_noise_workspace(limits()).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert!(capture_current_workspace().unwrap().same_job(&base));
        assert!(capture_current_noise_workspace().is_none());
        assert_eq!(budget.usage(), before);
        Ok(())
    })
    .unwrap();
    let (budget, pair) = owner();
    with_noise_workspace(pair.clone(), || {
        let before = budget.usage();
        // Existing internal capture-clear seam constructs the otherwise
        // unavailable half-context; no new hook or unsafe state manipulation.
        crate::spatial::captured(&None, || {
            assert!(capture_current_workspace().is_none());
            assert_eq!(
                capture_required_noise_workspace(limits()).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            assert!(capture_current_workspace().is_none());
            assert!(capture_current_noise_workspace().unwrap().same_job(&pair));
            Ok(())
        })?;
        assert!(
            capture_current_workspace()
                .unwrap()
                .same_job(pair.workspace())
        );
        assert_eq!(budget.usage(), before);
        Ok(())
    })
    .unwrap();
    assert!(capture_current_workspace().is_none());
    assert!(capture_current_noise_workspace().is_none());
}
#[test]
fn matched_capture_retains_both_original_counters_prior_debt_and_tls_limits() {
    let (budget, pair) = owner();
    pair.reserve_transferred_work(WorkComponents { base: 3, noise: 5 })
        .unwrap();
    let before = budget.usage();
    let child = with_noise_workspace(pair.clone(), || {
        let child = capture_required_noise_workspace(NoiseLimits {
            bytes: 128,
            base_work: 7,
            noise_work: 9,
        })?;
        assert!(child.same_job(&pair));
        assert!(child.workspace().same_job(pair.workspace()));
        assert_eq!(
            child.limits(),
            NoiseLimits {
                bytes: 128,
                base_work: 7,
                noise_work: 9
            }
        );
        assert_eq!(child.used()?, WorkComponents { base: 3, noise: 5 });
        let installed = capture_current_noise_workspace().unwrap();
        assert!(installed.same_job(&pair));
        assert_eq!(installed.limits(), limits());
        assert_eq!(capture_current_workspace().unwrap().bytes_limit(), 4096);
        assert_eq!(capture_current_workspace().unwrap().work_limit(), 11);
        assert_eq!(budget.usage(), before);
        Ok(child)
    })
    .unwrap();
    assert!(capture_current_workspace().is_none());
    assert!(capture_current_noise_workspace().is_none());
    child.reserve_base_work(4).unwrap();
    assert_eq!(pair.used().unwrap(), WorkComponents { base: 7, noise: 5 });
    assert_eq!(
        child.reserve_base_work(1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(pair.used().unwrap(), WorkComponents { base: 7, noise: 5 });
    child
        .reserve_transferred_work(WorkComponents { base: 0, noise: 4 })
        .unwrap();
    assert_eq!(pair.used().unwrap(), WorkComponents { base: 7, noise: 9 });
    assert_eq!(
        child
            .reserve_transferred_work(WorkComponents { base: 0, noise: 1 })
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(pair.used().unwrap(), WorkComponents { base: 7, noise: 9 });
    assert_eq!(budget.usage(), before);
}
#[test]
fn actual_tighter_base_context_is_retained_and_foreign_same_realm_is_refused() {
    let (budget, pair) = owner();
    let tighter = pair.workspace().tightened(96, 6).unwrap();
    let foreign = WorkspaceAdmission::new(pair.workspace().scope().clone(), 4096, 11);
    assert!(!foreign.same_job(pair.workspace()));
    with_noise_workspace(pair.clone(), || {
        let before = budget.usage();
        with_workspace(tighter.clone(), || {
            let child = capture_required_noise_workspace(NoiseLimits {
                bytes: 128,
                base_work: 7,
                noise_work: 9,
            })?;
            assert_eq!(
                child.limits(),
                NoiseLimits {
                    bytes: 96,
                    base_work: 6,
                    noise_work: 9
                }
            );
            assert!(child.same_job(&pair));
            assert!(capture_current_workspace().unwrap().same_job(&tighter));
            assert_eq!(capture_current_workspace().unwrap().bytes_limit(), 96);
            Ok(())
        })?;
        with_workspace(foreign.clone(), || {
            assert_eq!(
                capture_required_noise_workspace(limits()).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            assert!(capture_current_workspace().unwrap().same_job(&foreign));
            assert!(capture_current_noise_workspace().unwrap().same_job(&pair));
            Ok(())
        })?;
        assert!(
            capture_current_workspace()
                .unwrap()
                .same_job(pair.workspace())
        );
        assert_eq!(pair.used()?, WorkComponents::default());
        assert_eq!(foreign.work_used(), 0);
        assert_eq!(budget.usage(), before);
        Ok(())
    })
    .unwrap();
}
#[test]
fn exhausted_component_zero_bytes_and_overflow_requests_never_change_the_pair() {
    let (budget, pair) = owner();
    pair.reserve_transferred_work(WorkComponents { base: 3, noise: 5 })
        .unwrap();
    with_noise_workspace(pair.clone(), || {
        let before = budget.usage();
        for requested in [
            NoiseLimits {
                base_work: 2,
                ..limits()
            },
            NoiseLimits {
                noise_work: 4,
                ..limits()
            },
            NoiseLimits {
                bytes: 0,
                ..limits()
            },
            NoiseLimits {
                base_work: u64::MAX,
                noise_work: 1,
                ..limits()
            },
        ] {
            assert_eq!(
                capture_required_noise_workspace(requested)
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(pair.used()?, WorkComponents { base: 3, noise: 5 });
            let current = capture_current_noise_workspace().unwrap();
            assert!(current.same_job(&pair));
            assert_eq!(current.limits(), limits());
            assert!(
                capture_current_workspace()
                    .unwrap()
                    .same_job(pair.workspace())
            );
            assert_eq!(budget.usage(), before);
        }
        Ok(())
    })
    .unwrap();
}
#[test]
fn zero_work_request_retains_an_existing_pair_without_an_execution_grant() {
    let (budget, pair) = owner();
    with_noise_workspace(pair.clone(), || {
        let before = budget.usage();
        let child = capture_required_noise_workspace(NoiseLimits {
            base_work: 0,
            noise_work: 0,
            ..limits()
        })?;
        assert!(child.same_job(&pair));
        assert_eq!(
            child.limits(),
            NoiseLimits {
                base_work: 0,
                noise_work: 0,
                ..limits()
            }
        );
        assert_eq!(
            child.reserve_base_work(1).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(
            child
                .reserve_transferred_work(WorkComponents { base: 0, noise: 1 })
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(pair.used()?, WorkComponents::default());
        assert_eq!(
            capture_current_noise_workspace().unwrap().limits(),
            limits()
        );
        assert_eq!(budget.usage(), before);
        Ok(())
    })
    .unwrap();
}

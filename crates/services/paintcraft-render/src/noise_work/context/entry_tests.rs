use super::*;
use crate::spatial::{capture_current_workspace, with_workspace};
use paintcraft_core::types::{
    Error,
    resources::{AllocationClass, SharedBudget},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn limits() -> NoiseLimits {
    NoiseLimits {
        bytes: 4096,
        base_work: 11,
        noise_work: 17,
    }
}
fn owner() -> (SharedBudget, NoiseAdmission) {
    let budget = SharedBudget::new(4096);
    let scope = budget
        .reserve_capacity(4096)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let admission = NoiseAdmission::new_root(scope, NoiseJobPolicy::BrowserV1, limits()).unwrap();
    (budget, admission)
}
fn absent() {
    assert!(current().is_none());
    assert!(capture_current_workspace().is_none());
}
#[test]
fn entry_consumes_only_absent_base_input_and_retains_nested_original_owner() {
    absent();
    let (_budget, pair) = owner();
    let mut slot = Some(pair.clone());
    let (guard, base) = enter(&mut slot).unwrap();
    assert!(slot.is_none());
    assert!(base.same_job(pair.workspace()));
    assert!(current().unwrap().same_job(&pair));
    assert!(capture_current_workspace().is_none());
    with_workspace(base, || {
        let tighter = pair.workspace().tightened(128, 7)?;
        with_workspace(tighter, || {
            let mut nested = Some(pair.clone());
            let (restore, child_base) = enter(&mut nested)?;
            assert_eq!(
                nested.as_ref().unwrap().limits(),
                limits(),
                "original input retained"
            );
            assert!(nested.as_ref().unwrap().same_job(&pair));
            assert_eq!(child_base.bytes_limit(), 128);
            assert_eq!(child_base.work_limit(), 7);
            assert_eq!(current().unwrap().limits().bytes, 128);
            drop(child_base);
            drop(restore);
            assert_eq!(current().unwrap().limits(), limits());
            assert_eq!(nested.unwrap().limits(), limits());
            Ok(())
        })
    })
    .unwrap();
    drop(guard);
    absent();
}
struct RejectedWork {
    budget: SharedBudget,
    seen: Arc<AtomicBool>,
}
impl Drop for RejectedWork {
    fn drop(&mut self) {
        assert_eq!(
            self.budget.usage().reserved,
            4096,
            "input retained through work Drop"
        );
        assert!(current().is_none());
        assert!(capture_current_workspace().is_some());
        self.seen.store(true, Ordering::Release);
    }
}
#[test]
fn half_context_refusal_drops_work_before_input_and_never_installs_noise() {
    absent();
    let (foreign_budget, foreign) = owner();
    let base_budget = SharedBudget::new(4096);
    let base = WorkspaceAdmission::new(base_budget.scope(AllocationClass::SharedBudget), 128, 11);
    with_workspace(base.clone(), || {
        let seen = Arc::new(AtomicBool::new(false));
        let probe = RejectedWork {
            budget: foreign_budget.clone(),
            seen: seen.clone(),
        };
        let error = with_noise_workspace(foreign, move || -> Result<()> {
            let _probe = probe;
            panic!("half-context callback must not execute");
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(seen.load(Ordering::Acquire));
        assert_eq!(foreign_budget.usage().reserved, 0);
        assert!(capture_current_workspace().unwrap().same_job(&base));
        assert!(current().is_none());
        Ok(())
    })
    .unwrap();
    absent();
}
fn nested(pair: &NoiseAdmission, levels: u8, work: &dyn Fn() -> Result<()>) -> Result<()> {
    if levels == 0 {
        return work();
    }
    with_noise_workspace(pair.clone(), || nested(pair, levels - 1, work))
}
#[test]
fn nesting_bound_cancellation_and_unwind_restore_pair_with_original_debt() {
    absent();
    let (_budget, pair) = owner();
    pair.reserve_transferred_work(WorkComponents { base: 3, noise: 5 })
        .unwrap();
    nested(&pair, 16, &|| {
        assert_eq!(ACTIVE.with(|a| a.borrow().as_ref().unwrap().1), 16);
        let error = with_noise_workspace(pair.clone(), || -> Result<()> {
            panic!("seventeenth callback must not execute");
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(current().unwrap().same_job(&pair));
        Ok(())
    })
    .unwrap();
    absent();
    with_noise_workspace(pair.clone(), || {
        let original = Error::new(
            ErrorCode::OperationCancelled,
            "paired context error".repeat(9),
        )
        .at(23);
        let pointer = original.message.as_ptr();
        let error = with_noise_workspace(pair.clone(), move || Err::<(), _>(original)).unwrap_err();
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!(error.operation_index, Some(23));
        assert_eq!(current().unwrap().limits(), limits());
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_noise_workspace(pair.clone(), || -> Result<()> {
                panic!("pair unwind");
            });
        }));
        assert!(panic.is_err());
        assert!(current().unwrap().same_job(&pair));
        assert!(
            capture_current_workspace()
                .unwrap()
                .same_job(pair.workspace())
        );
        assert_eq!(pair.used()?, WorkComponents { base: 3, noise: 5 });
        Ok(())
    })
    .unwrap();
    absent();
}
#[test]
fn pair_retirement_wake_reenters_both_contexts_before_final_budget_release() {
    absent();
    let (budget, pair) = owner();
    let seen = Arc::new(AtomicBool::new(false));
    let flag = seen.clone();
    let observed = budget.clone();
    let watch = budget
        .watch(1, move || {
            if observed.usage().reserved < 4096 {
                absent();
                flag.store(true, Ordering::Release);
            }
        })
        .unwrap();
    assert_eq!(watch.observe().reserved, 4096);
    watch.wait_for_release(true);
    with_noise_workspace(pair, || {
        assert!(current().is_some());
        Ok(())
    })
    .unwrap();
    absent();
    assert!(seen.load(Ordering::Acquire));
    assert_eq!(budget.usage().reserved, 0);
}

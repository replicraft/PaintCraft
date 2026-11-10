use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn nested(a: &WorkspaceAdmission, levels: u8, work: &dyn Fn() -> Result<()>) -> Result<()> {
    if levels == 0 {
        return work();
    }
    with_workspace(a.clone(), || nested(a, levels - 1, work))
}
struct WorkDrop {
    budget: SharedBudget,
    dropped: Arc<AtomicBool>,
}
impl Drop for WorkDrop {
    fn drop(&mut self) {
        assert_eq!(
            self.budget.usage().reserved,
            512,
            "input outlives rejected work"
        );
        assert_eq!(ACTIVE.with(|a| a.borrow().as_ref().unwrap().1), 16);
        self.dropped.store(true, Ordering::Release);
    }
}
#[test]
fn seventeenth_entry_refuses_work_before_original_input_retires() {
    assert!(current().is_none());
    let budget = SharedBudget::new(1024);
    let parent = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 512, 11);
    nested(&parent, 16, &|| {
        assert_eq!(ACTIVE.with(|a| a.borrow().as_ref().unwrap().1), 16);
        let foreign_budget = SharedBudget::new(512);
        let foreign = WorkspaceAdmission::new(
            foreign_budget
                .reserve_capacity(512)?
                .scope(AllocationClass::SharedBudget),
            512,
            11,
        );
        let dropped = Arc::new(AtomicBool::new(false));
        let probe = WorkDrop {
            budget: foreign_budget.clone(),
            dropped: dropped.clone(),
        };
        let error = with_workspace(foreign, move || -> Result<()> {
            let _probe = probe;
            panic!("seventeenth callback must not execute");
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(dropped.load(Ordering::Acquire));
        assert_eq!(foreign_budget.usage().reserved, 0);
        assert!(current().unwrap().same_job(&parent));
        Ok(())
    })
    .unwrap();
    assert!(current().is_none());
    assert_eq!(parent.work_used(), 0);
}
#[test]
fn cancellation_identity_and_panic_restore_parent_without_resetting_debt() {
    let budget = SharedBudget::new(4096);
    let parent = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 512, 11);
    parent.reserve_work(3).unwrap();
    with_workspace(parent.clone(), || {
        let original = Error::new(
            ErrorCode::OperationCancelled,
            "original context error".repeat(9),
        )
        .at(23);
        let pointer = original.message.as_ptr();
        let child = parent.tightened(128, 7)?;
        let error = with_workspace(child, move || Err::<(), _>(original)).unwrap_err();
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!(error.operation_index, Some(23));
        assert_eq!(current().unwrap().bytes_limit(), 512);
        assert_eq!(parent.work_used(), 3);
        let child = parent.tightened(64, 5)?;
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_workspace(child, || -> Result<()> {
                assert_eq!(current().unwrap().bytes_limit(), 64);
                panic!("original context unwind");
            });
        }));
        assert!(panic.is_err());
        assert!(current().unwrap().same_job(&parent));
        assert_eq!(current().unwrap().bytes_limit(), 512);
        assert_eq!(parent.work_used(), 3);
        Ok(())
    })
    .unwrap();
    assert!(current().is_none());
}
#[test]
fn final_factory_release_observer_reenters_after_tls_is_restored() {
    assert!(current().is_none());
    let budget = SharedBudget::new(512);
    let owner = WorkspaceAdmission::new(
        budget
            .reserve_capacity(512)
            .unwrap()
            .scope(AllocationClass::SharedBudget),
        512,
        11,
    );
    let seen = Arc::new(AtomicBool::new(false));
    let flag = seen.clone();
    let observed = budget.clone();
    let watch = budget
        .watch(1, move || {
            if observed.usage().reserved == 0 {
                assert!(
                    current().is_none(),
                    "released RefCell borrow before observer"
                );
                flag.store(true, Ordering::Release);
            }
        })
        .unwrap();
    assert_eq!(watch.observe().reserved, 512);
    watch.wait_for_release(true);
    with_workspace(owner, || {
        assert_eq!(current().unwrap().work_limit(), 11);
        Ok(())
    })
    .unwrap();
    assert!(seen.load(Ordering::Acquire));
    assert_eq!(budget.usage().reserved, 0);
}

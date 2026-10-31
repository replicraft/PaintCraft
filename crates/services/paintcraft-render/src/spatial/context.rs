use super::*;
use std::cell::RefCell;

thread_local! {
    static ACTIVE: RefCell<Option<(WorkspaceAdmission, usize)>> = const { RefCell::new(None) };
}
struct Restore(Option<(WorkspaceAdmission, usize)>);
impl Drop for Restore {
    fn drop(&mut self) {
        // Releasing a budget can notify reentrant observers. Release RefCell
        // borrows before dropping the replaced scope.
        let old = ACTIVE.with(|active| active.replace(self.0.take()));
        drop(old);
    }
}

/// Supply an explicit shared intermediate-storage scope to synchronous rendering
/// on this thread. It restores on errors/unwind and does not create a budget.
/// The bounded parallel renderer explicitly propagates this factory to its jobs.
/// Spawned async work must install its captured scope on the executing thread.
pub fn with_workspace<T>(
    admission: WorkspaceAdmission,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let previous = ACTIVE.with(|active| active.borrow().clone());
    let depth = previous.as_ref().map_or(1, |(_, depth)| depth + 1);
    if depth > 16 {
        return Err(limit("Spatial workspace nesting exceeds its bound"));
    }
    ACTIVE.with(|active| *active.borrow_mut() = Some((admission, depth)));
    let _restore = Restore(previous);
    work()
}
pub(crate) fn current() -> Option<WorkspaceAdmission> {
    ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .map(|(admission, _)| admission.clone())
    })
}
/// Capture an installed caller for work that must survive synchronous scope
/// exit. The fallback is constructed only when no caller exists. This retains
/// the selected realm, cumulative counter and frozen program owner, tightens
/// limits, and does not install a thread-local scope.
pub fn capture_workspace_or_current(
    fallback: impl FnOnce() -> WorkspaceAdmission,
    byte_limit: u64,
    work_limit: u64,
) -> Result<WorkspaceAdmission> {
    let mut admission = match current() {
        Some(admission) => {
            drop(fallback);
            admission
        }
        None => fallback(),
    };
    admission.bytes_limit = admission.bytes_limit.min(byte_limit);
    admission.work_limit = admission.work_limit.min(work_limit);
    if admission.work_used() > admission.work_limit {
        return Err(limit(
            "Caller already exceeds the operation spatial-work ceiling",
        ));
    }
    Ok(admission)
}
/// Reuse an installed caller's byte realm and cumulative spatial-work counter.
/// Only an absent caller scope selects the supplied operation-reserved fallback.
/// Limits can tighten either factory, never enlarge it. The tighter work ceiling
/// conservatively includes work already consumed earlier in the caller's job.
/// Byte denial never authorizes switching to a different fallback realm.
pub fn with_workspace_or_current<T>(
    fallback: WorkspaceAdmission,
    byte_limit: u64,
    work_limit: u64,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let mut admission = current().unwrap_or(fallback);
    admission.bytes_limit = admission.bytes_limit.min(byte_limit);
    admission.work_limit = admission.work_limit.min(work_limit);
    if admission.work_used() > admission.work_limit {
        return Err(limit(
            "Caller already exceeds the operation spatial-work ceiling",
        ));
    }
    with_workspace(admission, work)
}
pub(crate) fn captured<T>(
    admission: &Option<WorkspaceAdmission>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    match admission {
        Some(admission) => with_workspace(admission.clone(), work),
        None => {
            // A Rayon worker may be running a nested task under another
            // request's scope. Absence must not inherit that unrelated owner.
            let previous = ACTIVE.with(|active| active.replace(None));
            let _restore = Restore(previous);
            work()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    #[test]
    fn captured_caller_survives_scope_exit_and_spends_without_constructing_fallback() {
        let parent_budget = SharedBudget::new(1024);
        let spare_budget = SharedBudget::new(1024);
        let parent =
            WorkspaceAdmission::new(parent_budget.scope(AllocationClass::SharedBudget), 512, 10);
        parent.reserve_work(3).unwrap();
        let captured = with_workspace(parent.clone(), || {
            capture_workspace_or_current(|| panic!("unused fallback constructed"), 128, 5)
        })
        .unwrap();
        assert!(current().is_none());
        assert_eq!((captured.bytes_limit(), captured.work_limit()), (128, 5));
        captured.reserve_work(2).unwrap();
        assert_eq!(parent.work_used(), 5);
        assert!(captured.reserve_work(1).is_err());
        assert_eq!(parent.work_used(), 5);
        let mut charge = captured.scope().account();
        charge.resize(64).unwrap();
        assert_eq!(parent_budget.usage().reserved, 64);
        assert_eq!(spare_budget.usage().reserved, 0);
        drop(charge);
        parent.reserve_work(1).unwrap();
        assert_eq!(parent.work_used(), 6);
        assert!(
            with_workspace(parent, || {
                capture_workspace_or_current(|| panic!("denial chose fallback"), 128, 5)
            })
            .is_err()
        );
        let fallback = capture_workspace_or_current(
            || WorkspaceAdmission::new(spare_budget.scope(AllocationClass::SharedBudget), 256, 3),
            64,
            2,
        )
        .unwrap();
        fallback.reserve_work(2).unwrap();
        assert!(fallback.reserve_work(1).is_err());
        let mut charge = fallback.scope().account();
        charge.resize(16).unwrap();
        assert_eq!(spare_budget.usage().reserved, 16);
        assert_eq!(parent_budget.usage().reserved, 0);
        assert!(current().is_none());
    }
    #[test]
    fn absent_captured_owner_cannot_borrow_an_unrelated_worker_scope() {
        let budget = SharedBudget::new(4096);
        let admission =
            WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 1024, 1000);
        assert!(current().is_none());
        with_workspace(admission, || {
            assert_eq!(current().unwrap().bytes_limit(), 1024);
            captured(&None, || {
                assert!(current().is_none());
                Ok(())
            })?;
            assert_eq!(current().unwrap().bytes_limit(), 1024);
            let error = captured(&None, || {
                Err::<(), _>(Error::new(
                    ErrorCode::OperationCancelled,
                    "test cancellation",
                ))
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(current().unwrap().bytes_limit(), 1024);
            Ok(())
        })
        .unwrap();
        assert!(current().is_none());
        assert_eq!(budget.usage().reserved, 0);
    }

    fn scene() -> paintcraft_document::Document {
        use paintcraft_core::types::{
            filters::{BoxBlurV1, Descriptor},
            ids::deterministic_id,
        };
        let mut doc = paintcraft_document::new_document(
            1,
            1,
            Some(Rgba([17, 93, 201, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
                deterministic_id(&["context-box"]),
                true,
                Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
            )])
            .unwrap();
        doc
    }

    #[test]
    fn installed_caller_denial_never_mints_fallback_capacity() {
        let doc = scene();
        let parent_budget = SharedBudget::new(1);
        let fallback_budget = SharedBudget::new(65536);
        let parent = WorkspaceAdmission::new(
            parent_budget.scope(AllocationClass::SharedBudget),
            65536,
            u64::MAX,
        );
        let fallback = WorkspaceAdmission::new(
            fallback_budget.scope(AllocationClass::SharedBudget),
            65536,
            u64::MAX,
        );
        let error = with_workspace(parent, || {
            with_workspace_or_current(fallback, 65536, u64::MAX, || crate::render_flat(&doc))
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(error.allocation_denial().is_some());
        assert_eq!(fallback_budget.usage().peak, 0);
        assert_eq!(parent_budget.usage().reserved, 0);
        assert!(current().is_none());
    }

    #[test]
    fn operation_restrictions_retain_prior_work_and_fallback_is_only_for_no_owner() {
        let doc = scene();
        let plan =
            LayerAppearancePlan::new(&doc.layers[&doc.roots[0]], doc.canvas_rect(), &|| Ok(()))
                .unwrap();
        let work = plan.quote().work;
        let budget = SharedBudget::new(65536);
        let spare_budget = SharedBudget::new(65536);
        let parent =
            WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 65536, work * 2);
        let spare = WorkspaceAdmission::new(
            spare_budget.scope(AllocationClass::SharedBudget),
            65536,
            u64::MAX,
        );
        with_workspace(parent.clone(), || {
            assert_eq!(
                with_workspace_or_current(spare.clone(), 65536, work * 2, || crate::render_flat(
                    &doc
                ))?,
                [17, 93, 201, 255]
            );
            assert_eq!(parent.work_used(), work);
            assert_eq!(
                with_workspace_or_current(spare.clone(), 65536, work, || crate::render_flat(&doc))
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(parent.work_used(), work);
            with_workspace_or_current(spare.clone(), 65536, work * 2, || crate::render_flat(&doc))?;
            assert_eq!(parent.work_used(), work * 2);
            Ok(())
        })
        .unwrap();
        assert_eq!(spare_budget.usage().peak, 0);
        assert_eq!(
            with_workspace_or_current(spare, 65536, u64::MAX, || crate::render_flat(&doc)).unwrap(),
            [17, 93, 201, 255]
        );
        assert!(spare_budget.usage().peak > 0);
        assert_eq!(spare_budget.usage().reserved, 0);
        assert!(current().is_none());
    }
}

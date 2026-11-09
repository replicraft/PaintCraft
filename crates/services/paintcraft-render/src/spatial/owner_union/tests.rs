use super::*;
use crate::noise_work::{
    NoiseAdmission, NoiseJobPolicy, NoiseLimits, capture_current_noise_workspace,
    capture_required_noise_workspace, with_noise_workspace,
};
use crate::spatial::{capture_current_workspace, with_workspace};
use paintcraft_core::types::{
    Rgba,
    filters::{Descriptor, LevelsRgbV1},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};

#[test]
fn unbound_union_is_zero_and_leaves_context_budget_and_work_unchanged() {
    let budget = SharedBudget::new(4096);
    let first = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 1024, 100);
    let second = WorkspaceAdmission::new(first.scope().clone(), 1024, 100);
    first.reserve_work(3).unwrap();
    let before = budget.usage();
    assert_eq!(first.prepared_owner_union_bytes(None).unwrap(), 0);
    assert_eq!(first.prepared_owner_union_bytes(Some(&second)).unwrap(), 0);
    assert_eq!(first.work_used(), 3);
    assert_eq!(second.work_used(), 0);
    assert_eq!(budget.usage(), before);
    assert!(capture_current_workspace().is_none());
    assert!(capture_current_noise_workspace().is_none());
}

#[test]
fn required_noise_capture_keeps_distinct_ambient_owner_in_the_byte_union() {
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["owner-union-filter"]),
            true,
            Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        )])
        .unwrap();
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let limits = NoiseLimits {
        bytes: 1 << 20,
        base_work: 268435456,
        noise_work: 1024,
    };
    let pair = NoiseAdmission::new_root(scope, NoiseJobPolicy::BrowserV1, limits).unwrap();
    let ambient = pair.prepare_document(&doc, &|| Ok(())).unwrap();
    let alternate = pair.prepare_document(&doc, &|| Ok(())).unwrap();
    assert!(ambient.same_job(&alternate));
    assert!(!std::ptr::eq(
        ambient.workspace().prepared_levels().unwrap(),
        alternate.workspace().prepared_levels().unwrap(),
    ));
    let first_bytes = ambient.workspace().prepared_retained_bytes();
    let second_bytes = alternate.workspace().prepared_retained_bytes();
    assert!(first_bytes > 0);
    assert_eq!(first_bytes, second_bytes);
    let before_usage = budget.usage();
    let before_work = pair.used().unwrap();
    with_noise_workspace(ambient.clone(), || {
        with_workspace(alternate.workspace().clone(), || {
            let original = capture_current_noise_workspace().unwrap();
            let selected = capture_required_noise_workspace(limits)?;
            assert!(std::ptr::eq(
                selected.workspace().prepared_levels().unwrap(),
                alternate.workspace().prepared_levels().unwrap(),
            ));
            assert!(std::ptr::eq(
                original.workspace().prepared_levels().unwrap(),
                ambient.workspace().prepared_levels().unwrap(),
            ));
            assert_eq!(
                selected
                    .workspace()
                    .prepared_owner_union_bytes(Some(original.workspace()))?,
                first_bytes + second_bytes,
            );
            assert_eq!(
                selected.workspace().prepared_owner_union_bytes(None)?,
                second_bytes
            );
            let same = selected.workspace().clone();
            assert_eq!(
                selected
                    .workspace()
                    .prepared_owner_union_bytes(Some(&same))?,
                second_bytes
            );
            assert!(
                capture_current_workspace()
                    .unwrap()
                    .same_job(alternate.workspace())
            );
            assert_eq!(pair.used()?, before_work);
            assert_eq!(budget.usage(), before_usage);
            Ok(())
        })
    })
    .unwrap();
    assert!(capture_current_workspace().is_none());
    assert!(capture_current_noise_workspace().is_none());
    assert_eq!(pair.used().unwrap(), before_work);
    assert_eq!(budget.usage(), before_usage);
}

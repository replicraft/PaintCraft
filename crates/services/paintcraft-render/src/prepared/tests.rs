use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use std::{cell::Cell, sync::Arc};

fn factory(realm: &SharedBudget, bytes: u64, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(realm.scope(AllocationClass::SharedBudget), bytes, work)
}
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "test cancellation")
}

#[test]
fn retained_program_reuses_exact_hidden_rgb_and_one_mask_blend() {
    let p = LevelsRgbV1::new(0, 255, 200, 0, 255).unwrap();
    let q = quote_levels(p);
    let realm = SharedBudget::new(q.peak_bytes);
    let admission = factory(&realm, q.peak_bytes, q.compile_work + 1024);
    let program = Arc::new(PreparedLevels::new(p, &admission, &|| Ok(())).unwrap());
    assert_eq!(program.policy(), p);
    assert!(program.actual_compile_work() <= q.compile_work);
    assert_eq!(realm.usage().reserved, 256);
    assert_eq!(realm.usage().peak, q.peak_bytes);
    let source = [
        Rgba([64, 16, 128, 0]),
        Rgba([64, 16, 128, 0]),
        Rgba([64, 16, 128, 0]),
        Rgba([1, 4, 9, 255]),
        Rgba([1, 4, 9, 255]),
        Rgba([1, 4, 9, 255]),
    ];
    let before = source;
    let coverage = [0, 128, 255, 0, 128, 255];
    let expected = [
        Rgba([64, 16, 128, 0]),
        Rgba([96, 40, 155, 0]),
        Rgba([128, 64, 181, 0]),
        Rgba([1, 4, 9, 255]),
        Rgba([9, 18, 29, 255]),
        Rgba([16, 32, 48, 255]),
    ];
    for iteration in 1..=4 {
        let mut out = [Rgba::TRANSPARENT; 6];
        program
            .evaluate_row(&admission, &source, Some(&coverage), &mut out, &|| Ok(()))
            .unwrap();
        assert_eq!(out, expected);
        assert_eq!(source, before);
        assert_eq!(admission.work_used(), q.compile_work + iteration * 12);
        assert_eq!(realm.usage().reserved, 256);
    }
    let peer = program.clone();
    drop(program);
    assert_eq!(realm.usage().reserved, 256);
    let shared_admission = admission.clone();
    let out = std::thread::spawn(move || {
        let mut out = [Rgba::TRANSPARENT];
        peer.evaluate_row(
            &shared_admission,
            &[Rgba([1, 4, 9, 109])],
            None,
            &mut out,
            &|| Ok(()),
        )
        .unwrap();
        out
    })
    .join()
    .unwrap();
    assert_eq!(out, [Rgba([16, 32, 48, 109])]);
    assert_eq!(realm.usage().reserved, 0);
    assert_eq!(admission.work_used(), q.compile_work + 50);
}

#[test]
fn preparation_preflight_and_parent_denial_never_create_another_realm() {
    let p = LevelsRgbV1::default();
    let q = quote_levels(p);
    for (bytes, work) in [
        (q.peak_bytes - 1, q.compile_work),
        (q.peak_bytes, q.compile_work - 1),
    ] {
        let realm = SharedBudget::new(q.peak_bytes);
        let admission = factory(&realm, bytes, work);
        let calls = Cell::new(0);
        let error = PreparedLevels::new(p, &admission, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(calls.get(), 0);
        assert_eq!(admission.work_used(), 0);
        assert_eq!(realm.usage().reserved, 0);
    }
    let realm = SharedBudget::new(q.peak_bytes - 1);
    let admission = factory(&realm, q.peak_bytes, q.compile_work);
    assert_eq!(
        PreparedLevels::new(p, &admission, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(admission.work_used(), q.compile_work);
    assert_eq!(realm.usage().reserved, 0);
    assert_eq!(admission.scope().footprint().peak, q.peak_bytes);
}

#[test]
fn cancellation_at_every_preparation_check_releases_scratch_and_table() {
    let p = LevelsRgbV1::default();
    let q = quote_levels(p);
    let realm = SharedBudget::new(q.peak_bytes);
    let admission = factory(&realm, q.peak_bytes, q.compile_work);
    let count = Cell::new(0);
    drop(
        PreparedLevels::new(p, &admission, &|| {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    assert!(count.get() > 256);
    assert_eq!(realm.usage().reserved, 0);
    for fail_at in 1..=count.get() {
        let realm = SharedBudget::new(q.peak_bytes);
        let admission = factory(&realm, q.peak_bytes, q.compile_work);
        let calls = Cell::new(0);
        let result = PreparedLevels::new(p, &admission, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == fail_at {
                Err(cancelled())
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(realm.usage().reserved, 0, "failure at check {fail_at}");
        assert!(admission.work_used() <= q.compile_work);
    }
}

#[test]
fn panic_during_private_preparation_releases_accounts_without_returning_an_owner() {
    let p = LevelsRgbV1::default();
    let q = quote_levels(p);
    let realm = SharedBudget::new(q.peak_bytes);
    let admission = factory(&realm, q.peak_bytes, q.compile_work);
    let calls = Cell::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        PreparedLevels::new(p, &admission, &|| {
            calls.set(calls.get() + 1);
            assert_ne!(calls.get(), 3, "test panic after byte admission");
            Ok(())
        })
    }));
    assert!(result.is_err());
    assert_eq!(realm.usage().reserved, 0);
    assert_eq!(admission.scope().footprint().live, 0);
    assert_eq!(admission.work_used(), q.compile_work);
}

#[test]
fn cloned_factories_share_compile_work_and_live_table_bytes() {
    let p = LevelsRgbV1::default();
    let q = quote_levels(p);
    let realm = SharedBudget::new(q.peak_bytes * 2);
    let admission = factory(&realm, q.peak_bytes, q.compile_work * 2 - 1);
    let first = PreparedLevels::new(p, &admission, &|| Ok(())).unwrap();
    assert_eq!(
        PreparedLevels::new(p, &admission.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(realm.usage().reserved, 256);
    assert_eq!(admission.work_used(), q.compile_work);
    drop(first);
    assert_eq!(realm.usage().reserved, 0);

    let realm = SharedBudget::new(q.peak_bytes);
    let admission = factory(&realm, q.peak_bytes, q.compile_work * 4);
    let first = PreparedLevels::new(p, &admission, &|| Ok(())).unwrap();
    assert_eq!(
        PreparedLevels::new(p, &admission.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(realm.usage().reserved, 256);
    assert_eq!(admission.work_used(), q.compile_work * 2);
    assert_eq!(admission.scope().footprint().peak, q.peak_bytes + 256);
    drop(first);
    drop(PreparedLevels::new(p, &admission, &|| Ok(())).unwrap());
    assert_eq!(realm.usage().reserved, 0);
}

#[test]
fn spatial_regions_and_preparation_spend_the_same_counter_in_either_order() {
    use crate::spatial::LayerAppearancePlan;
    use paintcraft_core::types::{
        Rect,
        filters::{BoxBlurV1, Descriptor},
        ids::deterministic_id,
    };
    use paintcraft_document::{FilterInstance, FilterStack};
    let doc = paintcraft_document::import_image(
        2,
        1,
        &[255, 0, 0, 255, 0, 255, 0, 255],
        "shared-program-region-work",
    )
    .unwrap();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["shared-program-region-work"]),
        true,
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
    )])
    .unwrap();
    let plan =
        LayerAppearancePlan::new(&layer, Rect::new(0, 0, 2, 1).unwrap(), &|| Ok(())).unwrap();
    let p = LevelsRgbV1::default();
    let q = quote_levels(p);
    let bytes = q.peak_bytes.max(plan.quote().bytes) + 256;
    for compile_first in [false, true] {
        let realm = SharedBudget::new(bytes);
        let admission = factory(&realm, bytes, q.compile_work + plan.quote().work - 1);
        let mut out = [Rgba([7, 8, 9, 10]); 2];
        if compile_first {
            let program = PreparedLevels::new(p, &admission, &|| Ok(())).unwrap();
            let error = plan
                .evaluate_into(&admission.clone(), &mut out, &|| Ok(()))
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::ResourceLimit);
            assert_eq!(out, [Rgba([7, 8, 9, 10]); 2]);
            assert_eq!(admission.work_used(), q.compile_work);
            assert_eq!(realm.usage().reserved, 256);
            drop(program);
        } else {
            plan.evaluate_into(&admission, &mut out, &|| Ok(()))
                .unwrap();
            assert_eq!(out, [Rgba([170, 85, 0, 255]), Rgba([85, 170, 0, 255])]);
            assert_eq!(
                PreparedLevels::new(p, &admission.clone(), &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(admission.work_used(), plan.quote().work);
        }
        assert_eq!(realm.usage().reserved, 0);
    }
}

#[test]
fn retained_table_survives_reserved_job_retirement_without_unused_capacity() {
    let p = LevelsRgbV1::default();
    let q = quote_levels(p);
    let realm = SharedBudget::new(q.peak_bytes);
    let scope = realm
        .reserve_capacity(q.peak_bytes)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let admission = WorkspaceAdmission::new(scope.clone(), q.peak_bytes, q.compile_work);
    let program = PreparedLevels::new(p, &admission, &|| Ok(())).unwrap();
    assert_eq!(realm.usage().reserved, q.peak_bytes);
    drop(admission);
    drop(scope);
    assert_eq!(realm.usage().reserved, 256);
    let next_job = factory(&realm, q.peak_bytes, 2);
    let mut out = [Rgba::TRANSPARENT];
    program
        .evaluate_row(&next_job, &[Rgba([1, 4, 9, 0])], None, &mut out, &|| Ok(()))
        .unwrap();
    assert_eq!(out, [Rgba([1, 4, 9, 0])]);
    assert_eq!(next_job.work_used(), 2);
    drop(program);
    assert_eq!(realm.usage().reserved, 0);
}

#[test]
fn row_geometry_work_and_cancellation_preserve_source_and_retained_owner() {
    let p = LevelsRgbV1::new(0, 255, 200, 0, 255).unwrap();
    let q = quote_levels(p);
    let realm = SharedBudget::new(q.peak_bytes);
    let admission = factory(&realm, q.peak_bytes, q.compile_work + 4096);
    let program = PreparedLevels::new(p, &admission, &|| Ok(())).unwrap();
    let source = [Rgba([1, 4, 9, 0]); 130];
    let sentinel = Rgba([7, 8, 9, 10]);
    let before = admission.work_used();
    for mask in [None, Some(&[0u8; 1][..])] {
        let mut out = vec![sentinel; if mask.is_some() { 3 } else { 2 }];
        let original = out.clone();
        let error = program
            .evaluate_row(&admission, &source[..3], mask, &mut out, &|| {
                panic!("geometry must fail before callbacks")
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InputSchema);
        assert_eq!(out, original);
    }
    assert_eq!(admission.work_used(), before);
    let short = factory(&realm, q.peak_bytes, 5);
    let mut out = [sentinel; 3];
    assert_eq!(
        program
            .evaluate_row(&short, &source[..3], None, &mut out, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(out, [sentinel; 3]);
    let mut out = [sentinel; 130];
    let calls = Cell::new(0);
    let error = program
        .evaluate_row(&admission, &source, None, &mut out, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err(cancelled())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(&out[..64], &[Rgba([16, 32, 48, 0]); 64]);
    assert_eq!(&out[64..], &[sentinel; 66]);
    assert_eq!(source, [Rgba([1, 4, 9, 0]); 130]);
    assert_eq!(admission.work_used(), before + 260);
    assert_eq!(realm.usage().reserved, 256);
}

use super::*;
use crate::prepared::{CompileWork, LevelsBackend};
use paintcraft_core::types::{
    Error, ErrorCode, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use std::cell::Cell;
fn factory(realm: &SharedBudget, bytes: u64, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(realm.scope(AllocationClass::SharedBudget), bytes, work)
}
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "job test cancellation")
}
fn constant(ib: i16, iw: i16, g: i16, ob: i16, ow: i16) -> LevelsRgbV1 {
    LevelsRgbV1::new(ib, iw, g, ob, ow).unwrap()
}
fn live(job: &PreparedLevelsJob) -> u64 {
    pointer_bytes(job.programs.capacity()).unwrap() + job.len() as u64 * 256
}
#[test]
fn eager_complete_keys_dedup_once_without_collapsing_equal_output_policies() {
    let p = constant(0, 255, 100, 73, 73);
    let distinct = [
        p,
        constant(1, 255, 100, 73, 73),
        constant(0, 254, 100, 73, 73),
        constant(0, 255, 101, 73, 73),
        constant(0, 255, 100, 72, 73),
        constant(0, 255, 100, 73, 74),
    ];
    let mut submitted = distinct.to_vec();
    submitted.extend(distinct);
    submitted.push(p);
    let q = quote_levels_job(&submitted).unwrap();
    assert_eq!(q.unique_programs, 6);
    let realm = SharedBudget::new(q.peak_bytes + 1024);
    let admission = factory(&realm, q.peak_bytes + 1024, q.compile_work + 12);
    let job = PreparedLevelsJob::new(&submitted, admission.clone(), &|| Ok(())).unwrap();
    assert_eq!(job.len(), 6);
    assert!(!job.is_empty());
    assert_eq!(realm.usage().reserved, live(&job));
    assert_eq!(admission.work_used(), q.compile_work);
    let original = job.get(p).unwrap();
    for policy in distinct {
        assert_eq!(job.get(policy).unwrap().policy(), policy);
    }
    for _ in 0..1000 {
        assert!(std::ptr::eq(original, job.get(p).unwrap()));
    }
    assert!(job.get(LevelsRgbV1::default()).is_none());
    assert_eq!(admission.work_used(), q.compile_work);
    let source = [Rgba([17, 64, 239, 0]), Rgba([0, 255, 31, 255])];
    let before = source;
    let mut out = [Rgba::TRANSPARENT; 2];
    original
        .evaluate_row(&admission, &source, None, &mut out, &|| Ok(()))
        .unwrap();
    assert_eq!(out, [Rgba([73, 73, 73, 0]), Rgba([73, 73, 73, 255])]);
    assert_eq!(source, before);
    assert_eq!(admission.work_used(), q.compile_work + 4);
    drop(job);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn aggregate_limits_refuse_before_compile_and_parent_denial_releases_index() {
    let p = LevelsRgbV1::default();
    let policies = [p, constant(0, 255, 100, 73, 73)];
    let q = quote_levels_job(&policies).unwrap();
    for (bytes, work) in [
        (q.peak_bytes - 1, q.compile_work),
        (q.peak_bytes, q.compile_work - 1),
    ] {
        let realm = SharedBudget::new(q.peak_bytes);
        let admission = factory(&realm, bytes, work);
        let calls = Cell::new(0);
        let result = PreparedLevelsJob::new(&policies, admission.clone(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(calls.get(), 0);
        assert_eq!(admission.work_used(), 0);
        assert_eq!(realm.usage().reserved, 0);
    }
    let one = quote_levels_job(&[p]).unwrap();
    let realm = SharedBudget::new(one.peak_bytes - 1);
    let admission = factory(&realm, one.peak_bytes, one.compile_work);
    assert_eq!(
        PreparedLevelsJob::new(&[p], admission.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(admission.work_used(), one.compile_work);
    assert_eq!(realm.usage().reserved, 0);
    let realm = SharedBudget::new(q.peak_bytes);
    let admission = factory(&realm, q.peak_bytes, q.compile_work);
    admission.clone().reserve_work(1).unwrap();
    assert_eq!(
        PreparedLevelsJob::new(&policies, admission.clone(), &|| panic!(
            "aggregate shared-work refusal called back"
        ))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(admission.work_used(), 1);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn cancellation_at_every_check_never_publishes_or_leaks_and_retry_keeps_spent_work() {
    let p = LevelsRgbV1::default();
    let policies = [p, p, constant(0, 255, 100, 73, 73)];
    let q = quote_levels_job(&policies).unwrap();
    let realm = SharedBudget::new(q.peak_bytes + 1024);
    let calls = Cell::new(0);
    let job = PreparedLevelsJob::new(
        &policies,
        factory(&realm, q.peak_bytes + 1024, q.compile_work),
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    let total = calls.get();
    drop(job);
    assert_eq!(realm.usage().reserved, 0);
    assert!(total > 500);
    for stop in 1..=total {
        let seen = Cell::new(0);
        let admission = factory(&realm, q.peak_bytes + 1024, q.compile_work);
        let result = PreparedLevelsJob::new(&policies, admission.clone(), &|| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(cancelled())
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "check {stop}"
        );
        assert_eq!(realm.usage().reserved, 0, "check {stop}");
        assert!(admission.work_used() <= q.compile_work);
    }
    let admission = factory(&realm, q.peak_bytes + 1024, q.compile_work * 2);
    let first = Cell::new(true);
    assert_eq!(
        PreparedLevelsJob::new(&policies, admission.clone(), &|| {
            if admission.work_used() > 0 && first.replace(false) {
                Err(cancelled())
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    let spent = admission.work_used();
    assert!(spent > 0);
    assert_eq!(realm.usage().reserved, 0);
    let job = PreparedLevelsJob::new(&policies, admission.clone(), &|| Ok(())).unwrap();
    assert_eq!(admission.work_used(), spent + q.compile_work);
    drop(job);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn frozen_job_owns_shared_tables_until_last_arc_owner_and_reserved_scope_retirement() {
    let p = LevelsRgbV1::default();
    let q = quote_levels_job(&[p, p]).unwrap();
    let realm = SharedBudget::new(q.peak_bytes + 1024);
    let scope = realm
        .reserve_capacity(q.peak_bytes + 1024)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let admission = WorkspaceAdmission::new(scope.clone(), q.peak_bytes + 1024, q.compile_work);
    let job = Arc::new(PreparedLevelsJob::new(&[p, p], admission, &|| Ok(())).unwrap());
    let retained = live(&job);
    assert_eq!(realm.usage().reserved, q.peak_bytes + 1024);
    drop(scope);
    assert_eq!(realm.usage().reserved, retained);
    let peer = job.clone();
    drop(job);
    assert_eq!(realm.usage().reserved, retained);
    let row = factory(&realm, 1024, 2);
    let out = std::thread::spawn(move || {
        let mut out = [Rgba::TRANSPARENT];
        peer.get(p)
            .unwrap()
            .evaluate_row(&row, &[Rgba([7, 19, 239, 0])], None, &mut out, &|| Ok(()))
            .unwrap();
        out
    })
    .join()
    .unwrap();
    assert_eq!(out, [Rgba([7, 19, 239, 0])]);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn input_and_unique_host_caps_are_explicit_while_empty_job_is_zero_payload() {
    let p = LevelsRgbV1::default();
    let duplicates = vec![p; 256];
    let q = quote_levels_job(&duplicates).unwrap();
    assert_eq!(q.unique_programs, 1);
    let realm = SharedBudget::new(1 << 20);
    let a = factory(&realm, 1 << 20, 1 << 20);
    let empty = PreparedLevelsJob::new(&[], a.clone(), &|| Ok(())).unwrap();
    assert!(empty.is_empty());
    assert_eq!(live(&empty), 0);
    assert_eq!(realm.usage().reserved, 0);
    let job = PreparedLevelsJob::new(&duplicates, a.clone(), &|| Ok(())).unwrap();
    assert_eq!(job.len(), 1);
    assert_eq!(realm.usage().reserved, live(&job));
    drop(job);
    let before = a.work_used();
    assert_eq!(
        PreparedLevelsJob::new(&vec![p; 257], a.clone(), &|| panic!(
            "input cap called back"
        ))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), before);
    assert_eq!(realm.usage().reserved, 0);
    let unique: Vec<_> = (0..=255).map(|v| constant(0, 255, 100, v, v)).collect();
    let job = PreparedLevelsJob::new(&unique, a.clone(), &|| Ok(())).unwrap();
    assert_eq!(job.len(), 256);
    assert_eq!(realm.usage().reserved, live(&job));
    drop(job);
    drop(empty);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn separate_jobs_do_not_get_global_dedup_and_wide_reports_remain_tagged() {
    let p = constant(0, 255, 999, 0, 255);
    let q = quote_levels_job(&[p, p]).unwrap();
    let realm = SharedBudget::new(q.peak_bytes * 2);
    let admission = factory(&realm, q.peak_bytes * 2, q.compile_work * 2);
    let first = PreparedLevelsJob::new(&[p, p], admission.clone(), &|| Ok(())).unwrap();
    let retained = live(&first);
    let second = PreparedLevelsJob::new(&[p], admission.clone(), &|| Ok(())).unwrap();
    assert_eq!(realm.usage().reserved, retained + live(&second));
    assert_eq!(admission.work_used(), q.compile_work * 2);
    assert!(!std::ptr::eq(first.get(p).unwrap(), second.get(p).unwrap()));
    assert_eq!(first.get(p).unwrap().backend(), LevelsBackend::U64);
    assert!(matches!(
        first.get(p).unwrap().actual_compile_work(),
        CompileWork::U64 { .. }
    ));
    drop(first);
    assert_eq!(realm.usage().reserved, live(&second));
    drop(second);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn panic_after_partial_preparation_releases_every_private_payload() {
    let policies = [LevelsRgbV1::default(), constant(0, 255, 100, 73, 73)];
    let q = quote_levels_job(&policies).unwrap();
    let realm = SharedBudget::new(q.peak_bytes + 1024);
    let admission = factory(&realm, q.peak_bytes + 1024, q.compile_work);
    let calls = Cell::new(0);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = PreparedLevelsJob::new(&policies, admission.clone(), &|| {
            calls.set(calls.get() + 1);
            assert!(calls.get() != 300, "private job panic");
            Ok(())
        });
    }));
    assert!(panic.is_err());
    assert_eq!(realm.usage().reserved, 0);
    assert!(admission.work_used() > 0);
}

#[test]
fn eager_job_and_spatial_regions_share_the_same_counter_in_both_orders() {
    use crate::spatial::LayerAppearancePlan;
    use paintcraft_core::types::{
        Rect,
        filters::{BoxBlurV1, Descriptor},
        ids::deterministic_id,
    };
    use paintcraft_document::{FilterInstance, FilterStack};
    let doc =
        paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255], "job-region")
            .unwrap();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["job-region"]),
        true,
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
    )])
    .unwrap();
    let region =
        LayerAppearancePlan::new(&layer, Rect::new(0, 0, 2, 1).unwrap(), &|| Ok(())).unwrap();
    let p = LevelsRgbV1::default();
    let q = quote_levels_job(&[p, p]).unwrap();
    let bytes = q.peak_bytes.max(region.quote().bytes) + 512;
    for compile_first in [false, true] {
        let realm = SharedBudget::new(bytes);
        let admission = factory(&realm, bytes, q.compile_work + region.quote().work - 1);
        let mut out = [Rgba([7, 8, 9, 10]); 2];
        if compile_first {
            let job = PreparedLevelsJob::new(&[p, p], admission.clone(), &|| Ok(())).unwrap();
            assert!(job.get(p).is_some());
            assert_eq!(admission.work_used(), q.compile_work);
            assert_eq!(
                region
                    .evaluate_into(&admission.clone(), &mut out, &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(out, [Rgba([7, 8, 9, 10]); 2]);
            assert_eq!(admission.work_used(), q.compile_work);
            assert_eq!(realm.usage().reserved, live(&job));
            drop(job);
        } else {
            region
                .evaluate_into(&admission, &mut out, &|| Ok(()))
                .unwrap();
            assert_eq!(out, [Rgba([170, 85, 0, 255]), Rgba([85, 170, 0, 255])]);
            assert_eq!(
                PreparedLevelsJob::new(&[p, p], admission.clone(), &|| panic!(
                    "aggregate job preflight called back"
                ))
                .unwrap_err()
                .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(admission.work_used(), region.quote().work);
        }
        assert_eq!(realm.usage().reserved, 0);
    }
}

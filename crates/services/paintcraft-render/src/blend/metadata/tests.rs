//! Frozen ledger literals predate candidate source; Root executes these controls.
use super::*;
use crate::{noise_work, spatial};
use paintcraft_core::types::{
    LayerBlendMode, Rgba,
    filters::{AddNoiseRgbV1, CurvesRgbV1, Descriptor, LevelsRgbV1},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
use std::cell::Cell;
fn scene() -> Document {
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut front = doc.layers[&doc.roots[0]].clone();
    front.id = deterministic_id(&["unbound-forecast-front"]);
    front.blend_mode = LayerBlendMode::Multiply;
    front.raster = paintcraft_raster::RasterLayer::new(1, 1, Rgba([204, 77, 13, 128]));
    doc.roots.push(front.id);
    doc.layers.insert(front.id, front);
    doc
}
fn filters(doc: &mut Document, descriptors: Vec<(bool, Descriptor)>) {
    doc.layers.get_mut(&doc.roots[1]).unwrap().filters = FilterStack::new(
        descriptors
            .into_iter()
            .enumerate()
            .map(|(i, (enabled, policy))| {
                FilterInstance::new(
                    deterministic_id(&["unbound-filter", &i.to_string()]),
                    enabled,
                    policy,
                )
            })
            .collect(),
    )
    .unwrap();
}
fn debt(used: &Cell<u64>, limit: u64, n: u64) -> Result<()> {
    let next = used
        .get()
        .checked_add(n)
        .filter(|next| *next <= limit)
        .ok_or_else(super::super::limit)?;
    used.set(next);
    Ok(())
}
#[test]
fn no_filter_forecasts_pay_only_metadata_and_do_not_create_authority() {
    let doc = scene();
    let before = doc.clone();
    let used = Cell::new(0);
    let p = quote_unbound_programs_checked(&doc, &|| Ok(()), &|n| debt(&used, 4160, n)).unwrap();
    assert_eq!(used.get(), 4160);
    assert_eq!(p.prepare_metadata_work, 4160);
    assert_eq!(p.numerical.unique_programs, 0);
    assert_eq!(p.numerical.compile_work, 0);
    assert_eq!(p.numerical.retained_bytes, 0);
    assert!(!p.requires_noise);
    assert!(spatial::current().is_none());
    assert!(noise_work::capture_current_noise_workspace().is_none());
    let c = quote_unbound_call_checked(&doc, doc.canvas_rect(), &|| Ok(()), &|n| {
        debt(&used, 8832, n)
    })
    .unwrap();
    assert_eq!(used.get(), 8832); //4160 program +4672 call metadata
    assert_eq!(
        (
            c.prepared_validation_work,
            c.setup_work,
            c.opaque_work,
            c.general_work
        ),
        (4160, 512, 32, 0)
    );
    assert!(!c.requires_noise);
    assert_eq!(c.context_bytes, super::super::control_quote());
    assert!(doc.same_semantics(&before));
    assert!(spatial::current().is_none());
}
#[test]
fn failed_attempts_keep_issued_prefix_and_retry_cannot_reset_debt() {
    let doc = scene();
    let used = Cell::new(0);
    let p = quote_unbound_programs_checked(&doc, &|| Ok(()), &|n| debt(&used, 4160, n));
    assert!(p.is_ok());
    let err = quote_unbound_call_checked(&doc, doc.canvas_rect(), &|| Ok(()), &|n| {
        debt(&used, 8320, n)
    })
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert_eq!(used.get(), 8320); // next512 proof helper was denied
    let err =
        quote_unbound_programs_checked(&doc, &|| Ok(()), &|n| debt(&used, 8320, n)).unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert_eq!(used.get(), 8320);
    assert!(spatial::current().is_none());
}
#[test]
fn levels_and_max_curves_share_frozen_existing_ledger_without_compilation() {
    for (policies, paid, prepare, validation) in [
        (
            vec![(true, Descriptor::LevelsRgbV1(LevelsRgbV1::default()))],
            5400,
            8792,
            4232,
        ),
        (
            (0..=255)
                .map(|i| {
                    (
                        true,
                        Descriptor::CurvesRgbV1(CurvesRgbV1::new([i, 0, 0, 0, 0])),
                    )
                })
                .collect(),
            663168,
            1202240,
            479296,
        ),
    ] {
        let mut doc = scene();
        filters(&mut doc, policies);
        let used = Cell::new(0);
        let q =
            quote_unbound_programs_checked(&doc, &|| Ok(()), &|n| debt(&used, paid, n)).unwrap();
        assert_eq!(used.get(), paid);
        assert_eq!(q.prepare_metadata_work, prepare);
        assert!(q.numerical.compile_work > 0);
        assert!(q.numerical.retained_bytes > 0);
        let call_debt = Cell::new(0);
        let c = quote_unbound_call_checked(&doc, doc.canvas_rect(), &|| Ok(()), &|n| {
            debt(&call_debt, paid + 512, n)
        })
        .unwrap();
        assert_eq!(call_debt.get(), paid + 512);
        assert_eq!(c.prepared_validation_work, validation);
        assert_eq!(c.opaque_work, 32);
        assert!(spatial::current().is_none());
        assert!(noise_work::capture_current_noise_workspace().is_none());
    }
}
#[test]
fn stored_default_noise_requirement_survives_enabled_and_disabled_descriptors() {
    for enabled in [false, true] {
        let mut doc = scene();
        filters(
            &mut doc,
            vec![(enabled, Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()))],
        );
        let used = Cell::new(0);
        let p =
            quote_unbound_programs_checked(&doc, &|| Ok(()), &|n| debt(&used, 4176, n)).unwrap();
        assert!(p.requires_noise);
        assert_eq!(used.get(), 4176);
        assert_eq!(p.prepare_metadata_work, 4176);
        assert_eq!(p.numerical.unique_programs, 0);
        let used = Cell::new(0);
        let c = quote_unbound_call_checked(&doc, doc.canvas_rect(), &|| Ok(()), &|n| {
            debt(&used, 4688, n)
        })
        .unwrap();
        assert!(c.requires_noise);
        assert_eq!(used.get(), 4688);
        assert_eq!(c.prepared_validation_work, 4176);
        assert!(noise_work::capture_current_noise_workspace().is_none());
    }
}
#[test]
fn actual_base_and_noise_ambient_refuse_before_any_callback_or_debt() {
    let doc = scene();
    let b = SharedBudget::new(4 << 20);
    let scope = b.scope(AllocationClass::SharedBudget);
    let base = spatial::WorkspaceAdmission::new(scope.clone(), 1 << 20, 0);
    let check_refusal = || {
        let checks = Cell::new(0);
        let spent = Cell::new(0);
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        let spend = |n| debt(&spent, u64::MAX, n);
        assert_eq!(
            quote_unbound_programs_checked(&doc, &check, &spend)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            quote_unbound_call_checked(&doc, doc.canvas_rect(), &check, &spend)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!((checks.get(), spent.get()), (0, 0));
        Ok(())
    };
    spatial::with_workspace(base.clone(), check_refusal).unwrap();
    assert_eq!(base.work_used(), 0);
    let pair = noise_work::NoiseAdmission::new_root(
        scope,
        noise_work::NoiseJobPolicy::BrowserV1,
        noise_work::NoiseLimits {
            bytes: 1 << 20,
            base_work: 0,
            noise_work: 0,
        },
    )
    .unwrap();
    noise_work::with_noise_workspace(pair.clone(), || {
        let captured = noise_work::capture_current_noise_workspace().unwrap();
        assert!(captured.same_job(&pair));
        assert_eq!(captured.limits(), pair.limits());
        check_refusal()
    })
    .unwrap();
    assert_eq!(pair.used().unwrap().base, 0);
    assert!(noise_work::capture_current_noise_workspace().is_none());
}
#[test]
fn actual_retention_observation_distinguishes_new_borrowed_and_zero_lut() {
    use crate::blend::service::{Caller, prepare_programs_checked};
    let b = SharedBudget::new(4 << 20);
    let a =
        spatial::WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, u64::MAX);
    let empty = prepare_programs_checked(&scene(), Caller::Base(&a), &|| Ok(()), &|n| {
        a.reserve_work(n)
    })
    .unwrap();
    assert_eq!(empty.newly_retained_bytes(), 0);
    let mut doc = scene();
    filters(
        &mut doc,
        vec![(true, Descriptor::LevelsRgbV1(LevelsRgbV1::default()))],
    );
    let new = prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
        .unwrap();
    assert_eq!(
        new.newly_retained_bytes(),
        256 + std::mem::size_of::<usize>() as u64
    );
    let borrowed =
        prepare_programs_checked(&doc, Caller::Base(new.workspace()), &|| Ok(()), &|n| {
            a.reserve_work(n)
        })
        .unwrap();
    assert_eq!(borrowed.newly_retained_bytes(), 0);
    assert_eq!(borrowed.retained_bytes(), new.retained_bytes());
    assert!(borrowed.same_job(Caller::Base(&a)));
}
#[test]
fn max_keys_helper_denial_and_cancellation_keep_prefix_without_new_owner() {
    let mut doc = scene();
    filters(
        &mut doc,
        (0..=255)
            .map(|i| {
                (
                    true,
                    Descriptor::CurvesRgbV1(CurvesRgbV1::new([i, 0, 0, 0, 0])),
                )
            })
            .collect(),
    );
    let used = Cell::new(0u64);
    let denied = Cell::new(0u64);
    let err = quote_unbound_programs_checked(&doc, &|| Ok(()), &|n| {
        if used.get().checked_add(n).is_none_or(|next| next > 663167) {
            denied.set(n);
        }
        debt(&used, 663167, n)
    })
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert_eq!((used.get(), denied.get()), (401728, 261440));
    let checks = Cell::new(0);
    let used = Cell::new(0);
    let err = quote_unbound_programs_checked(
        &scene(),
        &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 2 {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel metadata"))
            } else {
                Ok(())
            }
        },
        &|n| debt(&used, u64::MAX, n),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::OperationCancelled);
    assert_eq!(used.get(), 4160);
    assert!(spatial::current().is_none());
    assert!(noise_work::capture_current_noise_workspace().is_none());
}
#[test]
fn fresh_unbound_forecast_matches_existing_detached_caller_metadata_path() {
    use crate::blend::service::{self, Caller};
    let mut doc = scene();
    filters(
        &mut doc,
        vec![(true, Descriptor::LevelsRgbV1(LevelsRgbV1::default()))],
    );
    let budget = SharedBudget::new(4 << 20);
    let a = spatial::WorkspaceAdmission::new(
        budget.scope(AllocationClass::SharedBudget),
        1 << 20,
        u64::MAX,
    );
    let used = Cell::new(0);
    let forecast =
        quote_unbound_programs_checked(&doc, &|| Ok(()), &|n| debt(&used, u64::MAX, n)).unwrap();
    let bound =
        service::program_quote_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
            .unwrap();
    assert_eq!((used.get(), a.work_used()), (5400, 5400));
    assert_eq!(forecast.numerical, bound.numerical);
    assert_eq!(forecast.prepare_metadata_work, bound.prepare_metadata_work);
    let before = a.work_used();
    let forecast = quote_unbound_call_checked(&doc, doc.canvas_rect(), &|| Ok(()), &|n| {
        debt(&used, u64::MAX, n)
    })
    .unwrap();
    let bound = service::call_quote_checked(
        &doc,
        doc.canvas_rect(),
        Caller::Base(&a),
        &|| Ok(()),
        &|n| a.reserve_work(n),
    )
    .unwrap();
    assert_eq!((used.get() - 5400, a.work_used() - before), (5912, 5912));
    assert_eq!(forecast.prepared_validation_work, bound.call_metadata_work);
    assert_eq!(
        (
            forecast.setup_work,
            forecast.context_bytes,
            forecast.opaque_work,
            forecast.general_work
        ),
        (
            bound.setup_work,
            bound.context_bytes,
            bound.opaque_work,
            bound.general_work
        )
    );
    assert!(!a.has_prepared_programs());
    assert_eq!(budget.usage().reserved, 0);
}

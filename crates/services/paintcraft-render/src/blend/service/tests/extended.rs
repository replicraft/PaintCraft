//! Extended phase-C acceptance controls; independent expectations precede source.
use super::*;
use paintcraft_core::types::filters::{CurvesRgbV1, ExposureSrgbV1};
use paintcraft_document::{FilterMask, LayerMask};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
fn mask(value: u8) -> A8Mask {
    A8Mask::uniform(
        Rect::new(0, 0, 1, 1).unwrap(),
        value,
        &SharedBudget::new(1 << 20),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn set_filters(doc: &mut Document, descriptors: Vec<Descriptor>) {
    doc.layers.get_mut(&doc.roots[1]).unwrap().filters = FilterStack::new(
        descriptors
            .into_iter()
            .enumerate()
            .map(|(i, descriptor)| {
                FilterInstance::new(
                    deterministic_id(&["service-extended", &i.to_string()]),
                    true,
                    descriptor,
                )
            })
            .collect(),
    )
    .unwrap();
}
#[test]
fn actual_mixed_tables_masks_opacity_and_modes_match_eight_independent_literals() {
    use paintcraft_core::types::json;
    let oracle = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/blend-service-fixtures/oracle.json"
    )))
    .unwrap();
    for row in oracle.get("records").unwrap().as_array().unwrap() {
        let byte = |key| u8::try_from(row.get(key).unwrap().as_i64().unwrap()).unwrap();
        let mut doc = scene();
        doc.layers.get_mut(&doc.roots[0]).unwrap().raster =
            paintcraft_raster::RasterLayer::new(1, 1, Rgba([31, 127, 241, 255]));
        let mode = if row.get("mode").unwrap().as_str().unwrap() == "multiply" {
            LayerBlendMode::Multiply
        } else {
            LayerBlendMode::Screen
        };
        let target = doc.layers.get_mut(&doc.roots[1]).unwrap();
        target.blend_mode = mode;
        target.mask = Some(LayerMask::new(true, mask(byte("own_mask"))));
        target.filters = FilterStack::new(vec![
            FilterInstance::new(
                deterministic_id(&["mixed-levels"]),
                true,
                Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 100, 128, 128).unwrap()),
            )
            .with_mask(Some(FilterMask::new(true, mask(byte("filter_mask"))))),
            FilterInstance::new(
                deterministic_id(&["mixed-exposure"]),
                true,
                Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(100).unwrap()),
            ),
            FilterInstance::new(
                deterministic_id(&["mixed-curves"]),
                true,
                Descriptor::CurvesRgbV1(CurvesRgbV1::new([255, 191, 127, 63, 0])),
            ),
        ])
        .unwrap();
        let before = doc.clone();
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::SharedBudget);
        let a = owner(scope.clone(), u64::MAX);
        let quote =
            program_quote_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
                .unwrap();
        assert_eq!(
            (
                quote.numerical.levels_programs,
                quote.numerical.exposure_programs,
                quote.numerical.curves_programs()
            ),
            (1, 1, 1)
        );
        let programs =
            prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
                .unwrap();
        assert!(programs.workspace().prepared_curves().is_some());
        let expected: Vec<_> = row
            .get("composed")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| u8::try_from(v.as_i64().unwrap()).unwrap())
            .collect();
        let actual = render_call_checked(&doc, doc.canvas_rect(), &programs, &|| Ok(())).unwrap();
        assert_eq!(actual, expected, "mixed pipeline {row:?}");
        assert!(doc.same_semantics(&before));
        assert_eq!(doc.state_id, before.state_id);
        assert_eq!(scope.footprint().live, programs.retained_bytes());
        // A real spatial prefix follows the same independently frozen literals:
        // all true-edge-clamped taps of a one-pixel RGBA source are identical.
        let mut spatial = doc.clone();
        let layer = spatial.layers.get_mut(&spatial.roots[1]).unwrap();
        let mut stack = vec![FilterInstance::new(
            deterministic_id(&["mixed-spatial-prefix"]),
            true,
            Descriptor::BoxBlurV1(paintcraft_core::types::filters::BoxBlurV1::new(1).unwrap()),
        )];
        stack.extend(layer.filters.iter().cloned());
        layer.filters = FilterStack::new(stack).unwrap();
        let spatial_source = spatial.clone();
        assert_eq!(
            render_call_checked(&spatial, spatial.canvas_rect(), &programs, &|| Ok(())).unwrap(),
            expected
        );
        assert!(spatial.same_semantics(&spatial_source));
        assert_eq!(spatial.state_id, spatial_source.state_id);
        assert_eq!(scope.footprint().live, programs.retained_bytes());
        let work = a.work_used();
        drop(programs);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(a.work_used(), work);
    }
}
#[test]
fn actual_256_key_owner_validates_and_tight_metadata_refuses_before_lookup_helper() {
    let mut all = scene();
    set_filters(
        &mut all,
        (0..=255)
            .map(|i| Descriptor::CurvesRgbV1(CurvesRgbV1::new([i; 5])))
            .collect(),
    );
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let parent = owner(scope.clone(), u64::MAX);
    let programs = prepare_programs_checked(&all, Caller::Base(&parent), &|| Ok(()), &|n| {
        parent.reserve_work(n)
    })
    .unwrap();
    assert_eq!(programs.workspace().prepared_curves().unwrap().len(), 256);
    let mut candidate = scene();
    set_filters(
        &mut candidate,
        vec![Descriptor::CurvesRgbV1(CurvesRgbV1::new([177; 5]))],
    );
    let frozen = candidate.clone();
    let prior = parent.work_used();
    for (preparing, suffix, prefix) in [(false, 6022, 4183), (true, 6015, 4176)] {
        let used = parent.work_used();
        let child = programs
            .workspace()
            .tightened(1 << 20, used + suffix)
            .unwrap();
        let helper_charge_called = Cell::new(false);
        let spend = |n| {
            if n == 1840 {
                helper_charge_called.set(true);
            }
            parent.reserve_work(n)
        };
        let error = crate::spatial::with_workspace(child, || {
            if preparing {
                prepare_programs_checked(
                    &candidate,
                    Caller::Base(programs.workspace()),
                    &|| Ok(()),
                    &spend,
                )
                .map(|_| ())
            } else {
                program_quote_checked(
                    &candidate,
                    Caller::Base(programs.workspace()),
                    &|| Ok(()),
                    &spend,
                )
                .map(|_| ())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(parent.work_used(), used + prefix);
        assert!(!helper_charge_called.get());
    }
    assert!(parent.work_used() > prior);
    assert_eq!(
        render_call_checked(&candidate, candidate.canvas_rect(), &programs, &|| Ok(())).unwrap(),
        [245, 245, 245, 255]
    );
    assert!(candidate.same_semantics(&frozen));
    assert_eq!(candidate.state_id, frozen.state_id);
}
#[test]
fn original_mixed_reservation_with_independent_trackers_is_not_a_workspace_subtotal() {
    use paintcraft_core::types::resources::BudgetAccount;
    struct OuterBuffer {
        _bytes: Vec<u8>,
        _charge: BudgetAccount,
    }
    let budget = SharedBudget::new(128 << 20);
    let reservation = budget.reserve_capacity(96 << 20).unwrap();
    let outer_scope = reservation.clone().scope(AllocationClass::PaintMetadata);
    let workspace_scope = reservation.clone().scope(AllocationClass::SharedBudget);
    let requested = 80 << 20;
    let mut charge = outer_scope.account();
    charge.resize(requested).unwrap();
    let mut bytes = Vec::<u8>::new();
    bytes.try_reserve_exact(requested as usize).unwrap();
    charge.resize(bytes.capacity() as u64).unwrap();
    let outer = OuterBuffer {
        _bytes: bytes,
        _charge: charge,
    };
    assert!(outer_scope.footprint().live > 64 << 20);
    assert_eq!(workspace_scope.footprint().live, 0);
    assert_eq!(
        workspace_scope.capacity_snapshot().available,
        reservation.available()
    );
    let a = WorkspaceAdmission::new(workspace_scope.clone(), 64 << 20, u64::MAX);
    let doc = scene();
    let programs =
        prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
            .unwrap();
    assert_eq!(
        render_call_checked(&doc, doc.canvas_rect(), &programs, &|| Ok(())).unwrap(),
        [249, 233, 225, 255]
    );
    assert_eq!(workspace_scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 96 << 20);
    drop(programs);
    drop(outer);
    assert_eq!(reservation.available(), 96 << 20);
}
#[test]
fn new_preparation_exact_and_one_short_bytes_use_the_original_reserved_scope() {
    let policy = LevelsRgbV1::new(0, 255, 100, 17, 17).unwrap();
    // Pure source-defined formula, fixed BEFORE any preparation/counter attempt.
    let peak = blend_programs::CAPTURE_BYTES
        + std::mem::size_of::<std::sync::Arc<crate::prepared::PreparedLevels>>() as u64
        + crate::prepared::quote_levels(policy).peak_bytes;
    for (bytes, success) in [(peak, true), (peak - 1, false)] {
        let mut doc = scene();
        level(&mut doc, policy);
        let before = doc.clone();
        let budget = SharedBudget::new(peak + (1 << 20));
        let reservation = budget.reserve_capacity(bytes).unwrap();
        let scope = reservation.clone().scope(AllocationClass::SharedBudget);
        let a = WorkspaceAdmission::new(scope.clone(), bytes, u64::MAX);
        let result =
            prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n));
        if success {
            let programs = result.unwrap();
            assert!(programs.retained_bytes() > 0);
            assert_eq!(reservation.available(), bytes - programs.retained_bytes());
            let used = a.work_used();
            drop(programs);
            assert_eq!(reservation.available(), bytes);
            assert_eq!(a.work_used(), used);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(reservation.available(), bytes);
            assert!(a.work_used() > 0);
        }
        assert!(doc.same_semantics(&before));
        assert_eq!(doc.state_id, before.state_id);
        assert_eq!(budget.usage().reserved, bytes);
    }
}
#[test]
fn enabled_default_and_disabled_noise_keep_pair_and_tighter_noise_ceiling() {
    use crate::noise_work::{NoiseJobPolicy, NoiseLimits};
    for enabled in [true, false] {
        let mut doc = scene();
        let layer = doc.layers.get_mut(&doc.roots[1]).unwrap();
        layer.opacity = 255;
        layer.raster = paintcraft_raster::RasterLayer::new(1, 1, Rgba([128, 128, 128, 127]));
        layer.filters = FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["default-noise-real"]),
            enabled,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
        let before = doc.clone();
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::SharedBudget);
        let pair = NoiseAdmission::new_root(
            scope.clone(),
            NoiseJobPolicy::BrowserV1,
            NoiseLimits {
                bytes: 1 << 20,
                base_work: 1 << 20,
                noise_work: 1 << 20,
            },
        )
        .unwrap();
        let programs = prepare_programs_checked(&doc, Caller::Noise(&pair), &|| Ok(()), &|n| {
            pair.reserve_base_work(n)
        })
        .unwrap();
        let tight = pair
            .child(NoiseLimits {
                bytes: 1 << 20,
                base_work: 1 << 20,
                noise_work: 0,
            })
            .unwrap();
        let result = crate::noise_work::with_noise_workspace(tight, || {
            render_call_checked(&doc, doc.canvas_rect(), &programs, &|| Ok(()))
        });
        if enabled {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        } else {
            assert_eq!(result.unwrap(), [192, 192, 192, 255]);
        }
        assert_eq!(pair.used().unwrap().noise, 0);
        let expected = if enabled {
            [203, 203, 203, 255]
        } else {
            [192, 192, 192, 255]
        };
        assert_eq!(
            render_call_checked(&doc, doc.canvas_rect(), &programs, &|| Ok(())).unwrap(),
            expected
        );
        assert_eq!(pair.used().unwrap().noise > 0, enabled);
        assert!(doc.same_semantics(&before));
        assert_eq!(doc.state_id, before.state_id);
        assert_eq!(scope.footprint().live, pair.retained_control_bytes());
    }
}

use super::*;
use crate::{
    cache::{CacheLimits, CompositeCache, Domain},
    display::{DisplayCache, DisplayRequest},
    spatial::{LayerAppearancePlan, with_workspace},
};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    filters::BoxBlurV1,
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use std::cell::Cell;
fn inverse() -> CurvesRgbV1 {
    CurvesRgbV1::new([255, 191, 127, 63, 0])
}
fn descriptor(family: usize) -> Descriptor {
    match family {
        0 => Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        1 => Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(100).unwrap()),
        _ => Descriptor::CurvesRgbV1(inverse()),
    }
}
fn scene(filters: Vec<Descriptor>) -> Document {
    let mut d = paintcraft_document::import_image(
        2,
        1,
        &[64, 128, 255, 255, 16, 10, 11, 255],
        "curves-owner",
    )
    .unwrap();
    set(&mut d, filters);
    d
}
fn set(d: &mut Document, filters: Vec<Descriptor>) {
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(
        filters
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                FilterInstance::new(deterministic_id(&["curves-test", &i.to_string()]), true, p)
            })
            .collect(),
    )
    .unwrap();
}
fn factory(b: &SharedBudget, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, work)
}
fn prepared(d: &Document, b: &SharedBudget) -> WorkspaceAdmission {
    let q = quote_document_programs(d, &|| Ok(())).unwrap();
    prepare_document_programs(d, factory(b, q.compile_work), &|| Ok(())).unwrap()
}
#[test]
fn all_six_frozen_family_directions_refuse_before_warm_cpu_display_cache_and_legacy_extension() {
    for from in 0..3 {
        for to in 0..3 {
            if from == to {
                continue;
            }
            let target = scene(vec![descriptor(to)]);
            let donor = scene(vec![descriptor(from)]);
            let b = SharedBudget::new(1 << 20);
            let good = prepared(&target, &b);
            let mut cache = CompositeCache::new(CacheLimits {
                tile_edge: 1,
                committed_bytes: 4096,
                preview_bytes: 4096,
                entries_per_domain: 16,
            })
            .unwrap();
            let mut display = DisplayCache::new(CacheLimits {
                tile_edge: 1,
                committed_bytes: 4096,
                preview_bytes: 4096,
                entries_per_domain: 16,
            })
            .unwrap();
            let request = DisplayRequest::new(&target, target.canvas_rect(), 0).unwrap();
            with_workspace(good, || {
                cache.render(&target, target.canvas_rect(), Domain::Committed, || Ok(()))?;
                display.render(&target, request, Domain::Committed, || Ok(()))?;
                Ok(())
            })
            .unwrap();
            let frozen = prepared(&donor, &b);
            let work = frozen.work_used();
            let live = b.usage().reserved;
            assert_eq!(
                prepare_document_programs(&target, frozen.clone(), &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(
                with_workspace(frozen.clone(), || cache.render(
                    &target,
                    target.canvas_rect(),
                    Domain::Committed,
                    || Ok(())
                ))
                .unwrap_err()
                .code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(
                with_workspace(frozen.clone(), || display.render(
                    &target,
                    request,
                    Domain::Committed,
                    || Ok(())
                ))
                .unwrap_err()
                .code,
                ErrorCode::CapabilityUnsupported
            );
            if from == 2 {
                assert_eq!(
                    super::super::prepare_document_levels(&target, frozen.clone(), &|| Ok(()))
                        .unwrap_err()
                        .code,
                    ErrorCode::CapabilityUnsupported
                );
            }
            assert_eq!(frozen.work_used(), work);
            assert_eq!(b.usage().reserved, live);
        }
    }
}
#[test]
fn complete_five_byte_key_same_uuid_is_frozen_and_lookup_is_private_source_preserving() {
    let d = scene(vec![descriptor(2)]);
    let before = d.clone();
    let b = SharedBudget::new(1 << 20);
    let a = prepared(&d, &b);
    let q = quote_document_programs(&d, &|| Ok(())).unwrap();
    let program = a.prepared_curves().unwrap().get(inverse()).unwrap();
    assert_eq!(
        program.evaluate_pixel(Rgba([64, 128, 255, 0])),
        Rgba([191, 127, 0, 0])
    );
    assert_eq!(q.curves_programs(), 1);
    assert_eq!(a.prepared_curves().unwrap().len(), 1);
    assert!(a.has_prepared_programs());
    assert!(a.prepared_programs().is_none());
    assert!(a.prepared_levels().is_none());
    with_workspace(a.clone(), || {
        for _ in 0..3 {
            assert_eq!(
                crate::render_flat(&d)?,
                [191, 127, 0, 255, 239, 245, 244, 255]
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(a.work_used(), q.compile_work);
    assert!(d.same_semantics(&before));
    let mut changed = d.clone();
    let f = d.layers[&d.roots[0]].filters.as_slice()[0].with_descriptor(Descriptor::CurvesRgbV1(
        CurvesRgbV1::new([255, 191, 127, 63, 1]),
    ));
    changed.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(vec![f]).unwrap();
    assert_eq!(
        with_workspace(a.clone(), || crate::render_flat(&changed))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(a.work_used(), q.compile_work);
    let clone = a.clone();
    drop(a);
    assert!(b.usage().reserved > 0);
    drop(clone);
    assert_eq!(b.usage().reserved, 0);
}
#[test]
fn mixed_families_masks_box_and_explicit_regions_share_one_eager_owner() {
    let mut d = scene(vec![descriptor(0), descriptor(1), descriptor(2)]);
    let b = SharedBudget::new(1 << 20);
    let a = prepare_document_programs(&d, factory(&b, 268435456), &|| Ok(())).unwrap();
    assert_eq!(a.prepared_curves().unwrap().len(), 3);
    assert!(
        a.prepared_programs()
            .unwrap()
            .exposure(ExposureSrgbV1::new(100).unwrap())
            .is_some()
    );
    assert!(
        a.prepared_levels()
            .unwrap()
            .get(LevelsRgbV1::default())
            .is_some()
    );
    with_workspace(a.clone(), || {
        assert_eq!(
            crate::render_flat(&d)?,
            [165, 79, 0, 255, 229, 237, 236, 255]
        );
        Ok(())
    })
    .unwrap();
    set(
        &mut d,
        vec![
            descriptor(2),
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        ],
    );
    let mask = paintcraft_raster::coverage_mask::A8Mask::from_bands(
        Rect::new(0, 0, 2, 1).unwrap(),
        &[paintcraft_raster::coverage_mask::Band {
            y0: 0,
            y1: 1,
            runs: &[paintcraft_raster::coverage_mask::Run {
                x0: 0,
                x1: 1,
                coverage: 128,
            }],
        }],
        &SharedBudget::new(1 << 20),
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    let id = d.roots[0];
    let f = d.layers[&id].filters.as_slice()[0].with_mask(Some(FilterMask::new(true, mask)));
    let second = d.layers[&id].filters.as_slice()[1].clone();
    d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![f, second]).unwrap();
    // The same superset owner retains its original complete keys and counter.
    let compiled_work = a.work_used();
    let bound = prepare_document_programs(&d, a.clone(), &|| Ok(())).unwrap();
    assert_eq!(bound.work_used(), compiled_work);
    let plan = LayerAppearancePlan::new(&d.layers[&id], Rect::new(0, 0, 2, 1).unwrap(), &|| Ok(()))
        .unwrap();
    let mut out = [Rgba([0; 4]); 2];
    plan.evaluate_into(&bound, &mut out, &|| Ok(())).unwrap();
    assert_eq!(out, [Rgba([91, 88, 88, 255]), Rgba([53, 49, 50, 255])]);
}
#[test]
fn hidden_enabled_identity_and_mask_zero_keys_are_kept_disabled_curves_delegate_old_path() {
    let mut d = scene(vec![
        Descriptor::CurvesRgbV1(CurvesRgbV1::default()),
        descriptor(1),
    ]);
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().visible = false;
    let zero = paintcraft_raster::coverage_mask::A8Mask::uniform(
        d.canvas_rect(),
        0,
        &SharedBudget::new(1 << 20),
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    let mut hidden_filters = d.layers[&id].filters.as_slice().to_vec();
    hidden_filters[0] = hidden_filters[0].with_mask(Some(FilterMask::new(true, zero)));
    d.layers.get_mut(&id).unwrap().filters = FilterStack::new(hidden_filters).unwrap();
    let q = quote_document_programs(&d, &|| Ok(())).unwrap();
    assert_eq!(q.unique_programs, 2);
    let b = SharedBudget::new(1 << 20);
    let a = prepared(&d, &b);
    assert!(
        a.prepared_curves()
            .unwrap()
            .get(CurvesRgbV1::default())
            .is_some()
    );
    drop(a);
    let old = scene(vec![descriptor(1)]);
    let mut disabled = old.clone();
    let mut f = old.layers[&old.roots[0]].filters.as_slice().to_vec();
    f.push(FilterInstance::new(
        deterministic_id(&["disabled-curves"]),
        false,
        Descriptor::CurvesRgbV1(CurvesRgbV1::default()),
    ));
    disabled.layers.get_mut(&old.roots[0]).unwrap().filters = FilterStack::new(f).unwrap();
    assert_eq!(
        quote_document_programs(&old, &|| Ok(())).unwrap(),
        quote_document_programs(&disabled, &|| Ok(())).unwrap()
    );
    let a = prepared(&disabled, &b);
    assert!(a.prepared_curves().is_none());
    assert!(a.prepared_programs().is_some());
}
#[test]
fn cancellation_releases_private_payload_preserves_spent_work_and_same_scope_retry() {
    let d = scene(vec![descriptor(2)]);
    let q = quote_document_programs(&d, &|| Ok(())).unwrap();
    let b = SharedBudget::new(1 << 20);
    let a = factory(&b, 2 * q.compile_work);
    let calls = Cell::new(0);
    assert_eq!(
        prepare_document_programs(&d, a.clone(), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 20 {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(b.usage().reserved, 0);
    assert_eq!(a.work_used(), q.compile_work);
    let ready = prepare_document_programs(&d, a.clone(), &|| Ok(())).unwrap();
    assert_eq!(a.work_used(), 2 * q.compile_work);
    assert!(ready.prepared_curves().is_some());
    drop(ready);
    assert_eq!(b.usage().reserved, 0);
}
#[test]
fn capture_deduplicates_before_the_shared_256_key_limit() {
    let mut d = scene(vec![descriptor(2)]);
    let original = d.layers[&d.roots[0]].clone();
    for part in 0..2 {
        let mut l = original.clone();
        l.id = deterministic_id(&["many-curves", &part.to_string()]);
        l.filters = FilterStack::new(
            (0..200)
                .map(|i| {
                    FilterInstance::new(
                        deterministic_id(&["duplicate-curves", &i.to_string()]),
                        true,
                        descriptor(2),
                    )
                })
                .collect(),
        )
        .unwrap();
        d.roots.push(l.id);
        d.layers.insert(l.id, l);
    }
    let q = quote_document_programs(&d, &|| Ok(())).unwrap();
    assert_eq!(q.unique_programs, 1);
    let b = SharedBudget::new(1 << 20);
    let a = prepared(&d, &b);
    assert_eq!(a.prepared_curves().unwrap().len(), 1);
    drop(a);
    d = scene(vec![]);
    let original = d.layers[&d.roots[0]].clone();
    d.layers.clear();
    d.roots.clear();
    for part in 0..2 {
        let mut l = original.clone();
        l.id = deterministic_id(&["unique-curves", &part.to_string()]);
        l.filters = FilterStack::new(
            (part * 128..(part + 1) * 128 + usize::from(part == 1))
                .map(|i| {
                    FilterInstance::new(
                        deterministic_id(&["key", &i.to_string()]),
                        true,
                        Descriptor::CurvesRgbV1(CurvesRgbV1::new([
                            i as u8,
                            (i >> 8) as u8,
                            0,
                            0,
                            0,
                        ])),
                    )
                })
                .collect(),
        )
        .unwrap();
        d.roots.push(l.id);
        d.layers.insert(l.id, l);
    }
    assert_eq!(
        quote_document_programs(&d, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let a = factory(&b, 268435456);
    assert_eq!(
        prepare_document_programs(&d, a.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), 0);
    assert_eq!(b.usage().reserved, 0);
}

#[test]
fn combined_256_all_three_keys_have_exact_coexistence_peak_and_257_refuses_before_compile() {
    let mut d = scene(vec![]);
    let original = d.layers[&d.roots[0]].clone();
    d.layers.clear();
    d.roots.clear();
    let mut all = vec![
        Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        Descriptor::ExposureSrgbV1(ExposureSrgbV1::default()),
    ];
    all.extend((0..254).map(|i| Descriptor::CurvesRgbV1(CurvesRgbV1::new([i as u8, 0, 0, 0, 0]))));
    for (part, chunk) in all.chunks(128).enumerate() {
        let mut layer = original.clone();
        layer.id = deterministic_id(&["combined-layer", &part.to_string()]);
        layer.filters = FilterStack::new(
            chunk
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    FilterInstance::new(
                        deterministic_id(&["combined-filter", &part.to_string(), &i.to_string()]),
                        true,
                        *p,
                    )
                })
                .collect(),
        )
        .unwrap();
        d.roots.push(layer.id);
        d.layers.insert(layer.id, layer);
    }
    let q = quote_document_programs(&d, &|| Ok(())).unwrap();
    assert_eq!(
        (
            q.levels_programs,
            q.exposure_programs,
            q.curves_programs(),
            q.unique_programs
        ),
        (1, 1, 254, 256)
    );
    let b = SharedBudget::new(q.preparation_peak_bytes);
    let scope = b.scope(AllocationClass::SharedBudget);
    let a = prepare_document_programs(
        &d,
        WorkspaceAdmission::new(scope.clone(), q.preparation_peak_bytes, q.compile_work),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(scope.footprint().peak, q.preparation_peak_bytes);
    assert_eq!(a.prepared_retained_bytes(), q.retained_bytes);
    assert_eq!(a.work_used(), q.compile_work);
    let id = d.roots[1];
    let mut stack = d.layers[&id].filters.as_slice().to_vec();
    stack.push(FilterInstance::new(
        deterministic_id(&["key-257"]),
        true,
        Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(100).unwrap()),
    ));
    d.layers.get_mut(&id).unwrap().filters = FilterStack::new(stack).unwrap();
    assert_eq!(
        quote_document_programs(&d, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let fresh = WorkspaceAdmission::new(scope.clone(), q.preparation_peak_bytes, 268435456);
    assert_eq!(
        prepare_document_programs(&d, fresh.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(fresh.work_used(), 0);
    assert_eq!(b.usage().reserved, q.retained_bytes);
    drop(a);
    assert_eq!(b.usage().reserved, 0);
}

#[test]
fn cancellation_inside_each_family_keeps_shared_spending_and_releases_all_private_owners() {
    let d = scene(vec![
        Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        Descriptor::ExposureSrgbV1(ExposureSrgbV1::default()),
        descriptor(2),
    ]);
    let q = quote_document_programs(&d, &|| Ok(())).unwrap();
    let l = super::super::quote_levels(LevelsRgbV1::default()).compile_work;
    let e = super::super::quote_exposure(ExposureSrgbV1::default()).compile_work;
    for milestone in [l, l + e, q.compile_work] {
        let b = SharedBudget::new(1 << 20);
        let a = factory(&b, 2 * q.compile_work);
        let phase_calls = Cell::new(0);
        let err = prepare_document_programs(&d, a.clone(), &|| {
            if a.work_used() == milestone {
                phase_calls.set(phase_calls.get() + 1);
                if phase_calls.get() == 5 {
                    return Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "family arithmetic cancelled",
                    ));
                }
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
        assert_eq!(phase_calls.get(), 5);
        assert_eq!(a.work_used(), milestone);
        assert_eq!(b.usage().reserved, 0);
        let ready = prepare_document_programs(&d, a.clone(), &|| Ok(())).unwrap();
        assert_eq!(a.work_used(), milestone + q.compile_work);
        assert_eq!(ready.prepared_curves().unwrap().len(), 3);
        drop(ready);
        assert_eq!(b.usage().reserved, 0);
    }
}

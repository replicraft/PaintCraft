use super::*;
use crate::{
    cache::{CacheLimits, CompositeCache, Domain},
    display::{DisplayCache, DisplayRequest},
    spatial::{LayerAppearancePlan, with_workspace},
};
use paintcraft_core::types::{
    Rect, Rgba,
    filters::BoxBlurV1,
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use std::cell::Cell;
fn e(stops: i16) -> ExposureSrgbV1 {
    ExposureSrgbV1::new(stops).unwrap()
}
fn l() -> LevelsRgbV1 {
    LevelsRgbV1::default()
}
fn instance(d: Descriptor, n: u64) -> FilterInstance {
    FilterInstance::new(
        deterministic_id(&["prepared-program", &n.to_string()]),
        true,
        d,
    )
}
fn stack(doc: &mut Document, descriptors: Vec<FilterInstance>) {
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(descriptors).unwrap();
}
fn scene() -> Document {
    let mut doc =
        paintcraft_document::import_image(2, 1, &[64, 128, 255, 255, 16, 10, 11, 255], "exposure")
            .unwrap();
    stack(
        &mut doc,
        vec![
            instance(Descriptor::LevelsRgbV1(l()), 1),
            instance(Descriptor::ExposureSrgbV1(e(100)), 2),
        ],
    );
    doc
}
fn literal() -> Vec<u8> {
    vec![90, 176, 255, 255, 26, 18, 19, 255]
}
#[test]
fn exposure_only_frozen_owner_rejects_levels_only_before_cache_hit_or_legacy_extension() {
    let mut levels_doc = scene();
    stack(
        &mut levels_doc,
        vec![instance(Descriptor::LevelsRgbV1(l()), 1)],
    );
    let mut exposure_doc = levels_doc.clone();
    stack(
        &mut exposure_doc,
        vec![instance(Descriptor::ExposureSrgbV1(e(100)), 2)],
    );
    let budget = SharedBudget::new(1 << 20);
    let levels = super::super::prepare_document_levels(
        &levels_doc,
        factory(&budget, 1 << 20, 268_435_456),
        &|| Ok(()),
    )
    .unwrap();
    let mut cache = CompositeCache::new(CacheLimits {
        tile_edge: 1,
        committed_bytes: 4096,
        preview_bytes: 4096,
        entries_per_domain: 16,
    })
    .unwrap();
    with_workspace(levels, || {
        cache.render(
            &levels_doc,
            levels_doc.canvas_rect(),
            Domain::Committed,
            || Ok(()),
        )
    })
    .unwrap();
    let exposure = prepare_document_programs(
        &exposure_doc,
        factory(&budget, 1 << 20, 268_435_456),
        &|| Ok(()),
    )
    .unwrap();
    let used = exposure.work_used();
    let retained = budget.usage().reserved;
    for generic in [false, true] {
        let error = if generic {
            prepare_document_programs(&levels_doc, exposure.clone(), &|| Ok(()))
        } else {
            super::super::prepare_document_levels(&levels_doc, exposure.clone(), &|| Ok(()))
        }
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(exposure.work_used(), used);
        assert_eq!(budget.usage().reserved, retained);
    }
    let error = with_workspace(exposure.clone(), || {
        cache.render(
            &levels_doc,
            levels_doc.canvas_rect(),
            Domain::Committed,
            || Ok(()),
        )
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert_eq!(exposure.work_used(), used);
    assert_eq!(budget.usage().reserved, retained);
}
fn factory(budget: &SharedBudget, bytes: u64, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), bytes, work)
}
#[test]
fn legacy_eight_only_quote_callbacks_and_owner_path_remain_exact() {
    let mut doc = scene();
    stack(&mut doc, vec![instance(Descriptor::LevelsRgbV1(l()), 1)]);
    let old_calls = Cell::new(0);
    let new_calls = Cell::new(0);
    let old = super::super::quote_document_levels(&doc, &|| {
        old_calls.set(old_calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let new = quote_document_programs(&doc, &|| {
        new_calls.set(new_calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(old_calls.get(), new_calls.get());
    assert_eq!(old.compile_work, new.compile_work);
    assert_eq!(old.retained_bytes, new.retained_bytes);
    assert_eq!(old.preparation_peak_bytes, new.preparation_peak_bytes);
    assert_eq!(new.exposure_programs, 0);
    let budget = SharedBudget::new(1 << 20);
    let a = factory(&budget, 1 << 20, old.compile_work);
    let old_calls = Cell::new(0);
    let new_calls = Cell::new(0);
    let old = super::super::prepare_document_levels(&doc, a, &|| {
        old_calls.set(old_calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let new = prepare_document_programs(&doc, factory(&budget, 1 << 20, new.compile_work), &|| {
        new_calls.set(new_calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(old_calls.get(), new_calls.get());
    assert!(new.prepared_programs().is_none());
    assert!(new.prepared_levels().is_some());
    assert_eq!(old.prepared_retained_bytes(), new.prepared_retained_bytes());
    drop((old, new));
    assert_eq!(budget.usage().reserved, 0);
    stack(&mut doc, vec![]);
    let calls = Cell::new(0);
    assert_eq!(
        with_document_programs(
            &doc,
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
            || Ok(73)
        )
        .unwrap(),
        73
    );
    assert_eq!(calls.get(), 0);
}
#[test]
fn mixed_complete_keys_dedup_all_layers_and_release_at_last_owner() {
    let doc = scene();
    let mut doc = doc.clone();
    let id = doc.roots[0];
    let mut hidden = doc.layers[&id].clone();
    hidden.id = deterministic_id(&["hidden-prepared-program"]);
    hidden.visible = false;
    hidden.filters = FilterStack::new(vec![
        instance(Descriptor::ExposureSrgbV1(e(-100)), 3),
        instance(Descriptor::ExposureSrgbV1(e(0)), 4),
        instance(Descriptor::LevelsRgbV1(l()), 5),
    ])
    .unwrap();
    doc.roots.push(hidden.id);
    doc.layers.insert(hidden.id, hidden);
    let q = quote_document_programs(&doc, &|| Ok(())).unwrap();
    assert_eq!(
        (q.levels_programs, q.exposure_programs, q.unique_programs),
        (1, 3, 4)
    );
    let budget = SharedBudget::new(1 << 20);
    let a = prepare_document_programs(
        &doc,
        factory(&budget, q.preparation_peak_bytes, q.compile_work),
        &|| Ok(()),
    )
    .unwrap();
    let job = a.prepared_programs().unwrap();
    assert_eq!(job.len(), 4);
    assert!(!job.is_empty());
    assert!(a.prepared_levels().unwrap().get(l()).is_some());
    assert!(job.exposure(e(0)).is_some());
    assert!(job.exposure(e(1)).is_none());
    let pointer = job.exposure(e(100)).unwrap() as *const _;
    for _ in 0..1000 {
        assert_eq!(job.exposure(e(100)).unwrap() as *const _, pointer);
    }
    assert_eq!(a.work_used(), q.compile_work);
    assert_eq!(budget.usage().reserved, a.prepared_retained_bytes());
    let last = a.clone();
    drop(a);
    assert_eq!(budget.usage().reserved, last.prepared_retained_bytes());
    drop(last);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn aggregate_preflight_and_mid_family_cancel_release_all_private_payloads_without_resetting_work() {
    let doc = scene();
    let q = quote_document_programs(&doc, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(1 << 20);
    for (bytes, work) in [
        (q.preparation_peak_bytes - 1, q.compile_work),
        (q.preparation_peak_bytes, q.compile_work - 1),
    ] {
        let a = factory(&budget, bytes, work);
        assert_eq!(
            prepare_document_programs(&doc, a.clone(), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(a.work_used(), 0);
        assert_eq!(budget.usage().reserved, 0);
    }
    let a = factory(&budget, q.preparation_peak_bytes, q.compile_work);
    let lw = super::super::quote_levels(l()).compile_work;
    let err = prepare_document_programs(&doc, a.clone(), &|| {
        if a.work_used() > lw {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel Exposure after Levels",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::OperationCancelled);
    assert_eq!(a.work_used(), q.compile_work);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(
        prepare_document_programs(&doc, a.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), q.compile_work);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn canonical_family_order_is_independent_of_hashmap_insertion_and_stack_order() {
    let mut doc = scene();
    let id = doc.roots[0];
    let heavy = LevelsRgbV1::new(0, 255, 999, 0, 255).unwrap();
    let mut other = doc.layers[&id].clone();
    other.id = deterministic_id(&["other-program-layer"]);
    other.visible = false;
    other.filters = FilterStack::new(vec![
        instance(Descriptor::LevelsRgbV1(heavy), 3),
        instance(Descriptor::ExposureSrgbV1(e(-1)), 4),
    ])
    .unwrap();
    doc.roots.push(other.id);
    doc.layers.insert(other.id, other);
    let map_order: Vec<_> = doc.layers.keys().copied().collect();
    for (i, id) in map_order.iter().enumerate() {
        doc.layers.get_mut(id).unwrap().filters = FilterStack::new(vec![
            instance(
                Descriptor::LevelsRgbV1(if i == 0 { heavy } else { l() }),
                10,
            ),
            instance(
                Descriptor::ExposureSrgbV1(if i == 0 { e(-1) } else { e(100) }),
                11,
            ),
        ])
        .unwrap();
    }
    let mut ls = [LevelsRgbV1::default(); PreparedPointJob::MAX_PROGRAMS];
    let mut es = [ExposureSrgbV1::default(); PreparedPointJob::MAX_PROGRAMS];
    let (ln, en) = capture(&doc, &mut ls, &mut es, &|| Ok(())).unwrap();
    assert_eq!(&ls[..ln], &[l(), heavy]);
    assert_eq!(&es[..en], &[e(100), e(-1)]);
    let mut reversed = doc.clone();
    let mut entries: Vec<_> = doc.layers.iter().map(|(id, l)| (*id, l.clone())).collect();
    entries.reverse();
    reversed.layers = entries.into_iter().collect();
    reversed.roots.reverse();
    let q = quote_document_programs(&doc, &|| Ok(())).unwrap();
    assert_eq!(q, quote_document_programs(&reversed, &|| Ok(())).unwrap());
    let budget = SharedBudget::new(1 << 20);
    for source in [&doc, &reversed] {
        let a = prepare_document_programs(
            source,
            factory(&budget, 1 << 20, q.compile_work),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(a.work_used(), q.compile_work);
        assert_eq!(a.prepared_programs().unwrap().len(), 4);
        drop(a);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn combined_unique_bound_applies_after_dedup_not_per_instance_or_per_family() {
    let mut doc = scene();
    let mut filters = Vec::new();
    for n in 0..128 {
        filters.push(instance(
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 100, n, n).unwrap()),
            n as u64,
        ));
        filters.push(instance(
            Descriptor::ExposureSrgbV1(e(n - 64)),
            1000 + n as u64,
        ));
    }
    stack(&mut doc, filters);
    assert_eq!(
        quote_document_programs(&doc, &|| Ok(()))
            .unwrap()
            .unique_programs,
        256
    );
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = deterministic_id(&["overflow-program-layer"]);
    layer.filters =
        FilterStack::new(vec![instance(Descriptor::ExposureSrgbV1(e(64)), 9999)]).unwrap();
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
    assert_eq!(
        quote_document_programs(&doc, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut duplicate = scene();
    stack(
        &mut duplicate,
        (0..256)
            .map(|n| instance(Descriptor::ExposureSrgbV1(e(100)), n))
            .collect(),
    );
    let mut second = duplicate.layers[&duplicate.roots[0]].clone();
    second.id = deterministic_id(&["duplicates-second-layer"]);
    duplicate.roots.push(second.id);
    duplicate.layers.insert(second.id, second);
    assert_eq!(
        quote_document_programs(&duplicate, &|| Ok(()))
            .unwrap()
            .unique_programs,
        1
    );
}
#[test]
fn cache_pool_display_and_same_id_wrong_parameters_require_the_frozen_inventory() {
    let doc = scene();
    let before = doc.clone();
    let q = quote_document_programs(&doc, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(1 << 20);
    let a = prepare_document_programs(&doc, factory(&budget, 1 << 20, q.compile_work), &|| Ok(()))
        .unwrap();
    let mut cache = CompositeCache::new(CacheLimits {
        tile_edge: 1,
        committed_bytes: 4096,
        preview_bytes: 4096,
        entries_per_domain: 16,
    })
    .unwrap();
    for _ in 0..2 {
        let frame = with_workspace(a.clone(), || {
            cache.render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(frame.pixels, literal());
    }
    assert_eq!(
        cache
            .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let actual = with_workspace(a.clone(), || {
        crate::parallel::render_in_pool(
            &pool,
            &doc,
            doc.canvas_rect(),
            crate::parallel::Partition::Tiles { edge: 1 },
            4096,
            || Ok(()),
        )
    })
    .unwrap();
    assert_eq!(actual, literal());
    let mut display = DisplayCache::new(CacheLimits {
        tile_edge: 1,
        committed_bytes: 4096,
        preview_bytes: 4096,
        entries_per_domain: 16,
    })
    .unwrap();
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1).unwrap();
    let frame = with_workspace(a.clone(), || {
        display.render(&doc, request, Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(frame.pixels, vec![58, 97, 137, 255]);
    let mut wrong = doc.clone();
    let id = wrong.roots[0];
    let mut f = wrong.layers[&id].filters.as_slice().to_vec();
    f[1] = f[1].with_descriptor(Descriptor::ExposureSrgbV1(e(101)));
    stack(&mut wrong, f);
    let entered = Cell::new(false);
    assert_eq!(
        with_workspace(a.clone(), || with_document_programs(
            &wrong,
            &|| Ok(()),
            || {
                entered.set(true);
                Ok(())
            }
        ))
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(!entered.get());
    let wrong_q = quote_document_programs(&wrong, &|| Ok(())).unwrap();
    let wrong_a = prepare_document_programs(
        &wrong,
        factory(&budget, 1 << 20, wrong_q.compile_work),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        with_workspace(wrong_a, || cache.render(
            &doc,
            doc.canvas_rect(),
            Domain::Committed,
            || Ok(())
        ))
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(a.work_used(), q.compile_work);
    assert!(doc.same_semantics(&before));
    drop(a);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn legacy_bound_owner_cannot_be_extended_and_explicit_plan_ignores_wrong_tls() {
    use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};
    let mut doc =
        paintcraft_document::import_image(2, 1, &[10, 11, 128, 0, 64, 16, 255, 255], "masked")
            .unwrap();
    let masks = SharedBudget::new(1 << 20);
    let mask = A8Mask::from_bands(
        Rect::new(0, 0, 2, 1).unwrap(),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &[Run {
                x0: 0,
                x1: 1,
                coverage: 128,
            }],
        }],
        &masks,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    stack(
        &mut doc,
        vec![
            instance(Descriptor::LevelsRgbV1(l()), 1),
            instance(Descriptor::ExposureSrgbV1(e(100)), 2)
                .with_mask(Some(FilterMask::new(true, mask))),
        ],
    );
    let budget = SharedBudget::new(1 << 20);
    let legacy = super::super::prepare_document_levels(
        &doc,
        factory(&budget, 1 << 20, u64::MAX),
        &|| Ok(()),
    )
    .unwrap();
    let before = legacy.work_used();
    assert_eq!(
        prepare_document_programs(&doc, legacy.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(legacy.work_used(), before);
    drop(legacy);
    let explicit =
        prepare_document_programs(&doc, factory(&budget, 1 << 20, u64::MAX), &|| Ok(())).unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    let raster = layer.raster.clone();
    let plan = LayerAppearancePlan::new(layer, Rect::new(0, 0, 2, 1).unwrap(), &|| Ok(())).unwrap();
    let absent = factory(&budget, 1 << 20, u64::MAX);
    let mut out = [Rgba([7, 8, 9, 10]); 2];
    with_workspace(explicit.clone(), || {
        assert_eq!(
            plan.evaluate_into(&absent, &mut out, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(out, [Rgba([7, 8, 9, 10]); 2]);
        plan.evaluate_into(&explicit, &mut out, &|| Ok(()))
    })
    .unwrap();
    assert_eq!(out, [Rgba([14, 15, 152, 0]), Rgba([64, 16, 255, 255])]);
    assert_eq!(layer.raster, raster);
    drop(explicit);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn family_preparation_order_never_reorders_appearance_and_box_halos_use_exposure_once() {
    let mut doc =
        paintcraft_document::import_image(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255], "order")
            .unwrap();
    let budget = SharedBudget::new(1 << 20);
    let levels = instance(
        Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 100, 64, 64).unwrap()),
        1,
    );
    let exposure = instance(Descriptor::ExposureSrgbV1(e(100)), 2);
    for (filters, rgb) in [
        (vec![levels.clone(), exposure.clone()], 90),
        (vec![exposure, levels], 64),
    ] {
        stack(&mut doc, filters);
        assert_eq!(
            with_workspace(factory(&budget, 1 << 20, u64::MAX), || crate::render_flat(
                &doc
            ))
            .unwrap(),
            vec![rgb, rgb, rgb, 255, rgb, rgb, rgb, 255]
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let exposure = instance(Descriptor::ExposureSrgbV1(e(-100)), 3);
    let blur = instance(Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()), 4);
    // The independent Fraction oracle pins E(-1EV,85)=60,E(-1EV,170)=124.
    for (filters, expected) in [
        (
            vec![exposure.clone(), blur.clone()],
            vec![63, 63, 63, 255, 125, 125, 125, 255],
        ),
        (
            vec![blur, exposure],
            vec![60, 60, 60, 255, 124, 124, 124, 255],
        ),
    ] {
        stack(&mut doc, filters);
        let a = factory(&budget, 1 << 20, u64::MAX);
        assert_eq!(
            with_workspace(a, || crate::render_flat(&doc)).unwrap(),
            expected
        );
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn implicit_uniform_defaults_and_disabled_exposure_keep_their_correct_paths() {
    let mut doc = paintcraft_document::new_document(
        1024,
        1024,
        Some(Rgba([64, 128, 255, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    stack(
        &mut doc,
        vec![instance(Descriptor::ExposureSrgbV1(e(100)), 1)],
    );
    let budget = SharedBudget::new(1 << 20);
    let q = quote_document_programs(&doc, &|| Ok(())).unwrap();
    let a = factory(&budget, 1 << 20, q.compile_work);
    assert_eq!(
        with_workspace(a.clone(), || crate::render_region_cancellable(
            &doc,
            Rect::new(10, 10, 1, 1).unwrap(),
            || Ok(())
        ))
        .unwrap(),
        vec![90, 176, 255, 255]
    );
    assert_eq!(a.work_used(), q.compile_work);
    assert_eq!(budget.usage().reserved, 0);
    let f = doc.layers[&doc.roots[0]].filters.as_slice()[0].with_enabled(false);
    stack(&mut doc, vec![f]);
    assert_eq!(
        quote_document_programs(&doc, &|| Ok(())).unwrap(),
        DocumentProgramsQuote::default()
    );
    assert_eq!(
        crate::render_region_cancellable(&doc, Rect::new(10, 10, 1, 1).unwrap(), || Ok(()))
            .unwrap(),
        vec![64, 128, 255, 255]
    );
}

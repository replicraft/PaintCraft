use super::*;
use crate::{
    cache::{CacheLimits, CompositeCache, Domain},
    display::{DisplayCache, DisplayRequest},
    spatial::{LayerAppearancePlan, with_workspace},
};
use paintcraft_core::types::{
    Rect, Rgba,
    filters::{BoxBlurV1, Descriptor},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
fn policy() -> LevelsRgbV1 {
    LevelsRgbV1::new(0, 4, 200, 0, 255).unwrap()
}
fn filter(p: LevelsRgbV1, id: u128) -> FilterInstance {
    FilterInstance::new(
        deterministic_id(&["levels-document", &id.to_string()]),
        true,
        Descriptor::LevelsRgbV1(p),
    )
}
fn fixture() -> Document {
    let mut doc =
        paintcraft_document::import_image(2, 1, &[1, 0, 4, 255, 4, 1, 0, 255], "levels").unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![filter(policy(), 1)]).unwrap();
    doc
}
fn factory(realm: &SharedBudget, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(realm.scope(AllocationClass::SharedBudget), 1 << 20, work)
}
fn literal() -> Vec<u8> {
    vec![128, 0, 255, 255, 255, 128, 0, 255]
}
#[test]
fn outer_cache_and_pool_tiles_compile_once_and_cache_hits_require_caller_admission() {
    let doc = fixture();
    let before = doc.clone();
    let q = quote_document_levels(&doc, &|| Ok(())).unwrap();
    let realm = SharedBudget::new(1 << 20);
    let admission = factory(&realm, q.compile_work);
    let mut cache = CompositeCache::new(CacheLimits {
        tile_edge: 1,
        committed_bytes: 4096,
        preview_bytes: 4096,
        entries_per_domain: 16,
    })
    .unwrap();
    let frame = with_workspace(admission.clone(), || {
        cache.render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(frame.pixels, literal());
    assert_eq!(admission.work_used(), q.compile_work);
    assert_eq!(realm.usage().reserved, 0);
    assert_eq!(
        cache
            .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let denied = SharedBudget::new(0);
    assert_eq!(
        with_workspace(factory(&denied, q.compile_work), || cache.render(
            &doc,
            doc.canvas_rect(),
            Domain::Committed,
            || Ok(())
        ))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(denied.usage().reserved, 0);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    for partition in [
        crate::parallel::Partition::Tiles { edge: 1 },
        crate::parallel::Partition::Bands { rows: 1 },
    ] {
        let a = factory(&realm, q.compile_work);
        let pixels = with_workspace(a.clone(), || {
            crate::parallel::render_in_pool(&pool, &doc, doc.canvas_rect(), partition, 4096, || {
                Ok(())
            })
        })
        .unwrap();
        assert_eq!(pixels, literal());
        assert_eq!(a.work_used(), q.compile_work);
        assert_eq!(realm.usage().reserved, 0);
    }
    assert!(doc.same_semantics(&before));
}
#[test]
fn explicit_bound_generation_reuses_across_regions_and_rejects_new_keys_with_same_ids() {
    let doc = fixture();
    let q = quote_document_levels(&doc, &|| Ok(())).unwrap();
    let realm = SharedBudget::new(1 << 20);
    let admission =
        prepare_document_levels(&doc, factory(&realm, q.compile_work), &|| Ok(())).unwrap();
    let retained = admission.prepared_levels().unwrap().retained_bytes();
    assert_eq!(realm.usage().reserved, retained);
    for _ in 0..3 {
        with_workspace(admission.clone(), || {
            assert_eq!(
                crate::render_region_cancellable(&doc, Rect::new(0, 0, 1, 1).unwrap(), || Ok(()))?,
                literal()[..4]
            );
            assert_eq!(
                crate::render_region_cancellable(&doc, Rect::new(1, 0, 1, 1).unwrap(), || Ok(()))?,
                literal()[4..]
            );
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(admission.work_used(), q.compile_work);
    let mut changed = doc.clone();
    let id = changed.roots[0];
    changed.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![filter(
        LevelsRgbV1::new(0, 4, 100, 73, 73).unwrap(),
        1,
    )])
    .unwrap();
    assert_eq!(
        (changed.document_id, changed.state_id),
        (doc.document_id, doc.state_id)
    );
    assert_eq!(
        with_workspace(admission.clone(), || crate::render_flat(&changed))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(admission.work_used(), q.compile_work);
    assert_eq!(realm.usage().reserved, retained);
    drop(admission);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn explicit_plan_uses_its_admission_not_an_unrelated_tls_and_preserves_hidden_rgb_mask() {
    use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};
    let mut doc =
        paintcraft_document::import_image(2, 1, &[1, 0, 4, 0, 4, 1, 0, 255], "hidden").unwrap();
    let masks = SharedBudget::new(1 << 20);
    let coverage = A8Mask::from_bands(
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
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(vec![
        filter(policy(), 1).with_mask(Some(FilterMask::new(true, coverage))),
    ])
    .unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    let before = layer.raster.clone();
    let plan = LayerAppearancePlan::new(layer, Rect::new(0, 0, 2, 1).unwrap(), &|| Ok(())).unwrap();
    let realm = SharedBudget::new(1 << 20);
    let explicit = prepare_document_levels(&doc, factory(&realm, u64::MAX), &|| Ok(())).unwrap();
    let mut wrong = doc.clone();
    wrong.layers.get_mut(&wrong.roots[0]).unwrap().filters =
        FilterStack::new(vec![filter(LevelsRgbV1::default(), 1)]).unwrap();
    let unrelated = prepare_document_levels(&wrong, factory(&realm, u64::MAX), &|| Ok(())).unwrap();
    let absent = factory(&realm, u64::MAX);
    let mut output = [Rgba([7, 8, 9, 10]); 2];
    with_workspace(unrelated, || {
        assert_eq!(
            plan.evaluate_into(&absent, &mut output, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(output, [Rgba([7, 8, 9, 10]); 2]);
        plan.evaluate_into(&explicit, &mut output, &|| Ok(()))
    })
    .unwrap();
    assert_eq!(output, [Rgba([65, 0, 130, 0]), Rgba([4, 1, 0, 255])]);
    assert_eq!(layer.raster, before);
    drop(explicit);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn nonlinear_order_and_box_halos_use_the_prepared_stage_before_reduction() {
    let mut doc =
        paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255], "ordered")
            .unwrap();
    let levels = filter(LevelsRgbV1::new(0, 255, 50, 0, 255).unwrap(), 1);
    let blur = FilterInstance::new(
        deterministic_id(&["levels-document-blur"]),
        true,
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
    );
    let realm = SharedBudget::new(1 << 20);
    for (stack, expected) in [
        (
            vec![levels.clone(), blur.clone()],
            vec![170, 85, 0, 255, 85, 170, 0, 255],
        ),
        (vec![blur, levels], vec![113, 28, 0, 255, 28, 113, 0, 255]),
    ] {
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(stack).unwrap();
        let before = doc.clone();
        assert_eq!(
            with_workspace(factory(&realm, u64::MAX), || crate::render_flat(&doc)).unwrap(),
            expected
        );
        assert!(doc.same_semantics(&before));
        assert_eq!(realm.usage().reserved, 0);
    }
}
#[test]
fn implicit_uniform_display_and_dense_rows_share_prepared_values() {
    let mut doc = paintcraft_document::new_document(
        1024,
        1024,
        Some(Rgba([1, 0, 4, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![filter(policy(), 1)]).unwrap();
    let realm = SharedBudget::new(1 << 20);
    let q = quote_document_levels(&doc, &|| Ok(())).unwrap();
    let a = factory(&realm, q.compile_work);
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 8).unwrap();
    let mut cache = DisplayCache::new(CacheLimits {
        tile_edge: 1,
        committed_bytes: 4096,
        preview_bytes: 4096,
        entries_per_domain: 16,
    })
    .unwrap();
    let frame = with_workspace(a.clone(), || {
        cache.render(&doc, request, Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(frame.pixels, [128, 0, 255, 255].repeat(16));
    assert_eq!(a.work_used(), q.compile_work);
    assert_eq!(realm.usage().reserved, 0);
    assert_eq!(
        crate::display::render_display_cancellable(&doc, request, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn collector_dedups_instances_before_unique_cap_and_quotes_without_compilation() {
    let mut doc = fixture();
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().filters =
        FilterStack::new((0..256).map(|i| filter(policy(), i + 1)).collect()).unwrap();
    let mut duplicate = doc.layers[&id].clone();
    duplicate.id = deterministic_id(&["levels-document-duplicate"]);
    duplicate.visible = false;
    duplicate.opacity = 0;
    doc.roots.push(duplicate.id);
    doc.layers.insert(duplicate.id, duplicate);
    let q = quote_document_levels(&doc, &|| Ok(())).unwrap();
    assert_eq!(q.unique_programs, 1);
    assert_eq!(
        q.compile_work,
        super::super::quote_levels(policy()).compile_work
    );
    assert_eq!(
        q.retained_bytes,
        std::mem::size_of::<std::sync::Arc<crate::prepared::PreparedLevels>>() as u64 + 256
    );
    let realm = SharedBudget::new(1 << 20);
    let a = factory(&realm, q.compile_work);
    let bound = prepare_document_levels(&doc, a.clone(), &|| Ok(())).unwrap();
    assert_eq!(bound.prepared_levels().unwrap().len(), 1);
    assert_eq!(a.work_used(), q.compile_work);
    drop(bound);
    assert_eq!(realm.usage().reserved, 0);
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(
        (0..256)
            .map(|v| {
                filter(
                    LevelsRgbV1::new(0, 255, 100, v as i16, v as i16).unwrap(),
                    v + 1,
                )
            })
            .collect(),
    )
    .unwrap();
    let other = doc.roots[1];
    doc.layers.get_mut(&other).unwrap().filters = FilterStack::new(vec![filter(
        LevelsRgbV1::new(1, 255, 100, 73, 73).unwrap(),
        999,
    )])
    .unwrap();
    let a = factory(&realm, u64::MAX);
    assert_eq!(
        prepare_document_levels(&doc, a.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), 0);
    assert_eq!(realm.usage().reserved, 0);
}
#[test]
fn inventory_peak_and_cancellation_release_private_job_without_fallback() {
    let doc = fixture();
    let before = doc.clone();
    let q = quote_document_levels(&doc, &|| Ok(())).unwrap();
    let realm = SharedBudget::new(1 << 20);
    let admission = WorkspaceAdmission::new(
        realm.scope(AllocationClass::SharedBudget),
        q.preparation_peak_bytes - 1,
        q.compile_work,
    );
    assert_eq!(
        prepare_document_levels(&doc, admission.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(admission.work_used(), 0);
    assert_eq!(realm.usage().reserved, 0);
    let calls = std::cell::Cell::new(0);
    let error = with_workspace(factory(&realm, u64::MAX), || {
        crate::render_region_cancellable(&doc, doc.canvas_rect(), || {
            calls.set(calls.get() + 1);
            if calls.get() == 50 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel Levels job",
                ))
            } else {
                Ok(())
            }
        })
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(realm.usage().reserved, 0);
    assert!(doc.same_semantics(&before));
}

#[test]
fn complete_policy_order_is_independent_of_hash_iteration_insertion_and_roots() {
    let narrow = LevelsRgbV1::default();
    let wide = LevelsRgbV1::new(0, 255, 999, 0, 255).unwrap();
    assert!(narrow.semantic_bytes() < wide.semantic_bytes());
    let mut first = fixture();
    let initial = first.roots[0];
    let mut second_layer = first.layers[&initial].clone();
    second_layer.id = deterministic_id(&["levels-order-other"]);
    first.roots.push(second_layer.id);
    first.layers.insert(second_layer.id, second_layer);
    // Assign the descending policy order to this map's ACTUAL iteration order.
    // Thus an unsorted collector deterministically fails, regardless of its seed.
    let map_order: Vec<_> = first.layers.keys().copied().collect();
    for (id, policy) in map_order.iter().zip([wide, narrow]) {
        first.layers.get_mut(id).unwrap().filters =
            FilterStack::new(vec![filter(policy, 1)]).unwrap();
    }
    let mut other = first.clone();
    other.roots.reverse();
    other.layers = map_order
        .iter()
        .rev()
        .map(|id| (*id, first.layers[id].clone()))
        .collect();
    assert_eq!(
        (first.document_id, first.state_id),
        (other.document_id, other.state_id)
    );
    let q = quote_document_levels(&first, &|| Ok(())).unwrap();
    assert_eq!(q, quote_document_levels(&other, &|| Ok(())).unwrap());
    assert_eq!(q.unique_programs, 2);
    for doc in [&first, &other] {
        let mut policies = [LevelsRgbV1::default(); PreparedLevelsJob::MAX_PROGRAMS];
        let len = capture(doc, &mut policies, &|| Ok(())).unwrap();
        assert_eq!(&policies[..len], &[narrow, wide]);
        let realm = SharedBudget::new(1 << 20);
        let a = prepare_document_levels(doc, factory(&realm, q.compile_work), &|| Ok(())).unwrap();
        let job = a.prepared_levels().unwrap();
        assert_eq!(job.len(), 2);
        for policy in [narrow, wide] {
            assert_eq!(
                job.get(policy).unwrap().policy().semantic_bytes(),
                policy.semantic_bytes()
            );
        }
        assert_eq!(a.work_used(), q.compile_work);
        drop(a);
        assert_eq!(realm.usage().reserved, 0);
    }
}

use crate::spatial::{LayerAppearancePlan, WorkspaceAdmission, with_workspace};
use crate::*;
use paintcraft_core::types::{
    filters::{BoxBlurV1, CurvesRgbV1, Descriptor, VibranceRgbV1},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use std::cell::Cell;
fn v(a: i16) -> Descriptor {
    Descriptor::VibranceRgbV1(VibranceRgbV1::new(a).unwrap())
}
fn scene(filters: Vec<Descriptor>) -> Document {
    let mut d =
        paintcraft_document::import_image(2, 1, &[20, 115, 200, 255, 64, 16, 128, 255], "direct-v")
            .unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(
        filters
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                FilterInstance::new(deterministic_id(&["direct", &i.to_string()]), true, p)
            })
            .collect(),
    )
    .unwrap();
    d
}
fn factory(b: &SharedBudget, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, work)
}
#[test]
fn qualified_model_price_and_cumulative_parent_are_enforced_without_a_lut() {
    let d = scene(vec![v(100)]);
    let layer = &d.layers[&d.roots[0]];
    let area = Rect::new(0, 0, 2, 1).unwrap();
    let p = LayerAppearancePlan::new(layer, area, &|| Ok(())).unwrap();
    assert_eq!(
        p.direct_work(&|| Ok(())).unwrap(),
        paintcraft_effects::vibrance::quote(2).unwrap().work
    );
    assert_eq!(p.quote().work, 6 + 128); // existing extraction +visit+mask, plus direct64
    let q = prepared::quote_document_programs(&d, &|| Ok(())).unwrap();
    assert_eq!(
        (
            q.unique_programs,
            q.compile_work,
            q.retained_bytes,
            q.preparation_peak_bytes
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(
        render_flat(&d).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let b = SharedBudget::new(1 << 20);
    let a = factory(&b, 256);
    assert_eq!(
        with_workspace(a.clone(), || render_flat(&d)).unwrap(),
        [4, 116, 216, 255, 62, 4, 140, 255]
    );
    assert_eq!(a.work_used(), 128);
    assert_eq!(b.usage().reserved, 0);
    assert_eq!(
        with_workspace(a.clone(), || render_flat(&d))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), 128);
    let low = factory(&b, 255);
    assert_eq!(
        with_workspace(low.clone(), || render_flat(&d))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(low.work_used(), 0);
    let identity = scene(vec![v(0)]);
    let a = factory(&b, 256);
    assert_eq!(
        with_workspace(a.clone(), || render_flat(&identity)).unwrap(),
        [20, 115, 200, 255, 64, 16, 128, 255]
    );
    assert_eq!(a.work_used(), 128);
}
#[test]
fn uniform_attempt_dense_fallback_and_zero_mask_have_one_bounded_direct_charge() {
    let mut d = scene(vec![v(100)]);
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().raster =
        paintcraft_raster::RasterLayer::new(2, 1, Rgba([20, 115, 200, 255]));
    let b = SharedBudget::new(1 << 20);
    let coverage = paintcraft_raster::coverage_mask::A8Mask::uniform(
        Rect::new(0, 0, 1, 1).unwrap(),
        127,
        &b,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    let f = d.layers[&id].filters.as_slice()[0].with_mask(Some(FilterMask::new(true, coverage)));
    d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![f]).unwrap();
    let a = factory(&b, 256);
    let out = with_workspace(a.clone(), || render_flat(&d)).unwrap();
    assert_eq!(out, [12, 115, 208, 255, 20, 115, 200, 255]);
    assert_eq!(a.work_used(), 192); // one proof64 + dense two slots128 (outside mask skipped)
    assert!(a.work_used() <= 2 * paintcraft_effects::vibrance::quote(2).unwrap().work);
}
#[test]
fn spatial_prefix_halos_are_priced_and_bulk_debited_once() {
    let b = SharedBudget::new(1 << 20);
    for (descriptors, expected) in [
        (
            vec![v(100), Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap())],
            128,
        ),
        (
            vec![Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()), v(100)],
            64,
        ),
    ] {
        let d = scene(descriptors);
        let layer = &d.layers[&d.roots[0]];
        let p =
            LayerAppearancePlan::new(layer, Rect::new(0, 0, 1, 1).unwrap(), &|| Ok(())).unwrap();
        assert_eq!(p.direct_work(&|| Ok(())).unwrap(), expected);
        let a = factory(&b, p.quote().work);
        let mut actual = Vec::new();
        p.with_pixels(&a, &|| Ok(()), |pixels| {
            actual.extend_from_slice(pixels);
            Ok(())
        })
        .unwrap();
        assert_eq!(a.work_used(), p.quote().work);
        assert_eq!(actual.len(), 1);
        assert_eq!(b.usage().reserved, 0);
    }
}
#[test]
fn warm_cpu_display_caches_visible_sampling_and_parallel_require_the_captured_realm() {
    use crate::cache::{CacheLimits, CompositeCache, Domain};
    use crate::display::{DisplayCache, DisplayRequest};
    let d = scene(vec![v(100)]);
    let b = SharedBudget::new(1 << 20);
    let a = factory(&b, 100000);
    let limits = CacheLimits {
        tile_edge: 64,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 32,
    };
    let mut cpu = CompositeCache::new(limits).unwrap();
    let mut display = DisplayCache::new(limits).unwrap();
    let view = DisplayRequest::new(&d, d.canvas_rect(), 0).unwrap();
    with_workspace(a.clone(), || {
        cpu.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))
    })
    .unwrap();
    with_workspace(a.clone(), || {
        display.render(&d, view, Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(
        cpu.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        display
            .render(&d, view, Domain::Committed, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        sampling::sample_color(
            &d,
            None,
            (0, 0),
            paintcraft_core::types::picker::SampleSize::One,
            || Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        with_workspace(a.clone(), || render_flat(&d)).unwrap(),
        [4, 116, 216, 255, 62, 4, 140, 255]
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    for partition in [
        parallel::Partition::Bands { rows: 1 },
        parallel::Partition::Tiles { edge: 1 },
    ] {
        let bytes = with_workspace(a.clone(), || {
            parallel::render_in_pool(&pool, &d, d.canvas_rect(), partition, 1 << 20, || Ok(()))
        })
        .unwrap();
        assert_eq!(bytes, [4, 116, 216, 255, 62, 4, 140, 255]);
    }
    // A frozen Curves owner can coexist with direct V; no new LUT key/family.
    let curves = scene(vec![Descriptor::CurvesRgbV1(CurvesRgbV1::default())]);
    let bound = prepared::prepare_document_programs(&curves, a.clone(), &|| Ok(())).unwrap();
    let used = bound.work_used();
    with_workspace(bound.clone(), || render_flat(&d)).unwrap();
    assert_eq!(bound.work_used() - used, 128);
    assert_eq!(bound.prepared_curves().unwrap().len(), 1);
}
#[test]
fn cancellation_spends_accepted_direct_batches_and_retry_never_resets_the_parent() {
    let d = scene(vec![v(100)]);
    let frozen = d.clone();
    let b = SharedBudget::new(1 << 20);
    let a = factory(&b, 100000);
    let calls = Cell::new(0);
    let result = with_workspace(a.clone(), || {
        render_cancellable(&d, || {
            calls.set(calls.get() + 1);
            if a.work_used() > 0 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "after direct batch",
                ))
            } else {
                Ok(())
            }
        })
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    let spent = a.work_used();
    assert_eq!(spent, 128);
    assert!(d.same_semantics(&frozen));
    with_workspace(a.clone(), || render_flat(&d)).unwrap();
    assert_eq!(a.work_used(), spent + 128);
}

#[test]
fn independent_ordered_curves_vibrance_box_mask_planes_and_prefix_rois_match() {
    use paintcraft_core::types::json::{Value, parse};
    fn bytes(v: &Value) -> Vec<u8> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| u8::try_from(x.as_i64().unwrap()).unwrap())
            .collect()
    }
    let o = parse(include_bytes!(
        "../tests/vibrance-order-fixtures/oracle.json"
    ))
    .unwrap();
    let cases = o.get("cases").unwrap().as_array().unwrap();
    assert_eq!(cases.len(), 12);
    for row in cases {
        let size = row.get("size").unwrap().as_array().unwrap();
        let w = size[0].as_i64().unwrap() as u32;
        let h = size[1].as_i64().unwrap() as u32;
        let raw = bytes(row.get("source_rgba").unwrap());
        let expected = bytes(row.get("expected_rgba").unwrap());
        let mut d = paintcraft_document::import_image(w, h, &raw, "independent-order").unwrap();
        let id = d.roots[0];
        let origin = row.get("origin").unwrap().as_array().unwrap();
        d.layers.get_mut(&id).unwrap().origin =
            (origin[0].as_i64().unwrap(), origin[1].as_i64().unwrap());
        let b = SharedBudget::new(16 << 20);
        let domain = Rect::new(0, 0, i64::from(w), i64::from(h)).unwrap();
        let mut filters = Vec::new();
        for (i, s) in row
            .get("stages")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let descriptor = match s.get("kind").unwrap().as_str().unwrap() {
                "curves" => Descriptor::CurvesRgbV1(CurvesRgbV1::new([23, 9, 240, 73, 201])),
                "vibrance" => v(100),
                "box" => Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
                x => panic!("unknown independent stage {x}"),
            };
            let coverage = u8::try_from(s.get("coverage").unwrap().as_i64().unwrap()).unwrap();
            let mask = paintcraft_raster::coverage_mask::A8Mask::uniform(
                domain,
                coverage,
                &b,
                Default::default(),
                &|| Ok(()),
            )
            .unwrap();
            filters.push(
                FilterInstance::new(
                    deterministic_id(&["ordered-v", &i.to_string()]),
                    true,
                    descriptor,
                )
                .with_mask(Some(FilterMask::new(true, mask))),
            );
        }
        d.layers.get_mut(&id).unwrap().filters = FilterStack::new(filters).unwrap();
        let frozen = d.clone();
        let a =
            prepared::prepare_document_programs(&d, factory(&b, 10_000_000), &|| Ok(())).unwrap();
        let layer = &d.layers[&id];
        let mut actual = Vec::new();
        let plan = LayerAppearancePlan::new(layer, domain, &|| Ok(())).unwrap();
        plan.with_pixels(&a, &|| Ok(()), |p| {
            actual.extend(p.iter().flat_map(|p| p.0));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            actual,
            expected,
            "{}",
            row.get("name").unwrap().as_str().unwrap()
        );
        for part in row
            .get("partitions")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .chain(row.get("roi"))
        {
            let p = part.get("region").unwrap().as_array().unwrap();
            let local = Rect::new(
                p[0].as_i64().unwrap() - layer.origin.0,
                p[1].as_i64().unwrap() - layer.origin.1,
                p[2].as_i64().unwrap(),
                p[3].as_i64().unwrap(),
            )
            .unwrap();
            let plan = LayerAppearancePlan::new(layer, local, &|| Ok(())).unwrap();
            let mut actual = Vec::new();
            plan.with_pixels(&a, &|| Ok(()), |p| {
                actual.extend(p.iter().flat_map(|p| p.0));
                Ok(())
            })
            .unwrap();
            assert_eq!(actual, bytes(part.get("expected_rgba").unwrap()));
        }
        // All intermediate stage planes are independent expectations too.
        for (index, prefix) in row
            .get("prefixes_rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let mut partial = layer.clone();
            partial.filters =
                FilterStack::new(layer.filters.as_slice()[..=index].to_vec()).unwrap();
            let plan = LayerAppearancePlan::new(&partial, domain, &|| Ok(())).unwrap();
            let mut plane = Vec::new();
            plan.with_pixels(&a, &|| Ok(()), |p| {
                plane.extend(p.iter().flat_map(|p| p.0));
                Ok(())
            })
            .unwrap();
            assert_eq!(plane, bytes(prefix));
        }
        let mut composed = d.clone();
        composed.layers.get_mut(&id).unwrap().origin = (0, 0);
        let mut visible = expected.clone();
        for p in visible.as_chunks_mut::<4>().0 {
            if p[3] == 0 {
                p[..3].fill(0);
            }
        }
        assert_eq!(
            with_workspace(a.clone(), || render_flat(&composed)).unwrap(),
            visible
        );
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        for edge in [32, 64, 128] {
            let actual = with_workspace(a.clone(), || {
                parallel::render_in_pool(
                    &pool,
                    &composed,
                    composed.canvas_rect(),
                    parallel::Partition::Tiles { edge },
                    1 << 20,
                    || Ok(()),
                )
            })
            .unwrap();
            assert_eq!(actual, visible);
        }
        assert!(d.same_semantics(&frozen));
    }
}

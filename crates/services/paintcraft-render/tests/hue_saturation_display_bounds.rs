//! Priced HSL evaluation through public display and cache APIs.
use paintcraft_core::types::{
    Rect, Rgba,
    filters::{Descriptor, HueSaturationRgbV1},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterMask, FilterStack};
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    display::{DisplayCache, DisplayRequest},
    spatial::{WorkspaceAdmission, quote_direct_work, with_workspace},
};
fn scene(width: u32) -> (Document, SharedBudget) {
    let b = SharedBudget::new(1 << 20);
    let mut d = paintcraft_document::new_document(
        width,
        1,
        Some(Rgba([20, 115, 200, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mask = paintcraft_raster::coverage_mask::A8Mask::uniform(
        Rect::new(0, 0, 1, 1).unwrap(),
        127,
        &b,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    let f = FilterInstance::new(
        deterministic_id(&["tight-public-display"]),
        true,
        Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::new(60, 100, 10).unwrap()),
    )
    .with_mask(Some(FilterMask::new(true, mask)));
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(vec![f]).unwrap();
    (d, b)
}
fn limits() -> CacheLimits {
    CacheLimits {
        tile_edge: 64,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 32,
    }
}
fn factory(b: &SharedBudget, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, work)
}
#[test]
fn public_display_reports_quoted_vs_actual_before_asserting_tight_success() {
    let (d, b) = scene(2);
    let region = d.canvas_rect();
    let request = DisplayRequest::new(&d, region, 0).unwrap();
    let quoted = quote_direct_work(&d.layers[&d.roots[0]], region, &|| Ok(())).unwrap() * 2;
    assert_eq!(quoted, 512);
    let mut probes = Vec::new();
    for limit in [511, 512, 640, 768] {
        let a = factory(&b, limit);
        let mut c = DisplayCache::new(limits()).unwrap();
        let result = with_workspace(a.clone(), || {
            c.render(&d, request, Domain::Committed, || Ok(()))
        });
        probes.push((limit, a.work_used(), result.as_ref().err().map(|e| e.code)));
        if let Ok(frame) = result {
            assert_eq!(frame.pixels, [74, 66, 227, 255, 20, 115, 200, 255]);
        }
    }
    eprintln!("quoted={quoted} actual_public_display_probes={probes:?}");
    // The exact advertised bound must admit the public display path.
    assert!(
        probes
            .iter()
            .any(|(limit, used, error)| *limit == quoted && error.is_none() && *used <= quoted)
    );
}
#[test]
fn one_pixel_public_cache_roi_must_not_price_then_render_the_expanded_64_pixel_tile() {
    let (d, b) = scene(130);
    let region = Rect::new(0, 0, 1, 1).unwrap();
    let request = DisplayRequest::new(&d, region, 0).unwrap();
    let quoted = quote_direct_work(&d.layers[&d.roots[0]], region, &|| Ok(())).unwrap() * 2;
    assert_eq!(quoted, 256);
    let a = factory(&b, quoted);
    let mut c = CompositeCache::new(limits()).unwrap();
    let composite = with_workspace(a.clone(), || {
        c.render(&d, region, Domain::Committed, || Ok(()))
    });
    eprintln!(
        "quoted={quoted} composite_used={} error={:?}",
        a.work_used(),
        composite.as_ref().err()
    );
    let a = factory(&b, quoted);
    let mut c = DisplayCache::new(limits()).unwrap();
    let display = with_workspace(a.clone(), || {
        c.render(&d, request, Domain::Committed, || Ok(()))
    });
    eprintln!(
        "quoted={quoted} display_used={} error={:?}",
        a.work_used(),
        display.as_ref().err()
    );
    assert_eq!(composite.unwrap().pixels, [74, 66, 227, 255]);
    assert_eq!(display.unwrap().pixels, [74, 66, 227, 255]);
}

#[test]
fn clipped_cold_warm_disjoint_and_lod_views_stay_within_each_exact_request() {
    let (d, b) = scene(130);
    for x in [0, 65, 129] {
        for level in [0, 1] {
            let region = Rect::new(x, 0, 1, 1).unwrap();
            let request = DisplayRequest::new(&d, region, level).unwrap();
            let quoted = quote_direct_work(&d.layers[&d.roots[0]], request.region(), &|| Ok(()))
                .unwrap()
                * 2;
            let mut c = DisplayCache::new(limits()).unwrap();
            for _ in 0..2 {
                let a = factory(&b, quoted);
                let frame = with_workspace(a.clone(), || {
                    c.render(&d, request, Domain::Committed, || Ok(()))
                })
                .unwrap();
                let expected = if x == 0 && level == 0 {
                    [74, 66, 227, 255]
                } else if x == 0 {
                    [47, 91, 214, 255]
                } else {
                    [20, 115, 200, 255]
                };
                assert_eq!(frame.pixels, expected);
                assert!(a.work_used() <= quoted);
            }
            let a = factory(&b, quoted - 1);
            assert!(
                with_workspace(a.clone(), || c.render(
                    &d,
                    request,
                    Domain::Committed,
                    || Ok(())
                ))
                .is_err()
            );
            assert_eq!(a.work_used(), 0);
        }
    }
}

#[test]
fn active_hue_saturation_outside_roi_cannot_be_evaluated_by_an_expanded_cold_cache_tile() {
    let (mut d, b) = scene(130);
    let id = d.roots[0];
    let mut top = d.layers[&id].clone();
    d.layers.get_mut(&id).unwrap().filters = FilterStack::default();
    top.id = deterministic_id(&["hsl-outside-roi"]);
    top.origin = (32, 0);
    top.raster = paintcraft_raster::RasterLayer::new(1, 1, Rgba([20, 115, 200, 255]));
    d.roots.push(top.id);
    d.layers.insert(top.id, top);
    let region = Rect::new(0, 0, 1, 1).unwrap();
    let a = factory(&b, 0);
    let mut c = CompositeCache::new(limits()).unwrap();
    assert_eq!(
        with_workspace(a.clone(), || c.render(
            &d,
            region,
            Domain::Committed,
            || Ok(())
        ))
        .unwrap()
        .pixels,
        [20, 115, 200, 255]
    );
    let mut c = DisplayCache::new(limits()).unwrap();
    let request = DisplayRequest::new(&d, region, 0).unwrap();
    assert_eq!(
        with_workspace(a.clone(), || c.render(
            &d,
            request,
            Domain::Committed,
            || Ok(())
        ))
        .unwrap()
        .pixels,
        [20, 115, 200, 255]
    );
    assert_eq!(a.work_used(), 0);
}

#[test]
fn composite_dirty_patch_regions_remain_clipped_after_source_versions_change() {
    let (mut d, b) = scene(130);
    let region = Rect::new(65, 0, 1, 1).unwrap();
    let quoted = 256;
    let mut c = CompositeCache::new(limits()).unwrap();
    let a = factory(&b, quoted);
    assert_eq!(
        with_workspace(a, || c.render(&d, region, Domain::Committed, || Ok(())))
            .unwrap()
            .pixels,
        [20, 115, 200, 255]
    );
    // A first pixel mutation materializes a complete256x256 decoded tile.
    // This independent fixture admission is not the renderer's256-unit budget.
    let tile_pixels = u64::from(paintcraft_raster::TILE_EDGE).pow(2);
    let mut admission = paintcraft_raster::Admission::with_residency(
        tile_pixels,
        SharedBudget::new(paintcraft_raster::RasterLayer::TILE_BYTES as u64),
    );
    d.layers
        .get_mut(&d.roots[0])
        .unwrap()
        .raster
        .fill_rect(
            &region,
            Rgba([64, 16, 128, 255]),
            paintcraft_raster::BlendMode::SourceOver,
            &mut admission,
        )
        .unwrap();
    let a = factory(&b, quoted);
    let result = with_workspace(a.clone(), || {
        c.render(&d, region, Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(result.pixels, [64, 16, 128, 255]);
    assert!(a.work_used() <= quoted);
    assert_eq!(result.stats.recomposed_pixels, 1);
}

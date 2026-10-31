//! Real compositor paths against literal blur pixels, not a point fallback.
use paintcraft_core::types::{
    ErrorCode, Rect, Rgba,
    filters::{BoxBlurV1, Descriptor},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack, POLICY, new_document};
use paintcraft_raster::{Admission, BlendMode, RasterLayer};
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    display::{DisplayRequest, render_display_cancellable},
    parallel::{Partition, render_in_pool},
    render_flat, render_region,
    spatial::{WorkspaceAdmission, with_workspace},
};

fn filter() -> FilterStack {
    FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["spatial-composite"]),
        true,
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
    )])
    .unwrap()
}
fn cache() -> CompositeCache {
    CompositeCache::new(CacheLimits {
        tile_edge: 32,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 32,
    })
    .unwrap()
}
fn job<T>(budget: &SharedBudget, work: impl FnOnce() -> paintcraft_core::types::Result<T>) -> T {
    let scope = budget.scope(AllocationClass::SharedBudget);
    let admission = WorkspaceAdmission::new(scope, 8 << 20, u64::MAX);
    with_workspace(admission, work).unwrap()
}
fn edge_scene() -> Document {
    let mut doc = new_document(2, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.raster = RasterLayer::from_rgba(
        2,
        1,
        Rgba::TRANSPARENT,
        &[255, 0, 0, 255, 0, 255, 0, 0],
        &mut Admission::new(1 << 20),
    )
    .unwrap();
    layer.filters = filter();
    doc
}

#[test]
fn cpu_serial_pool_cache_and_display_share_admitted_full_resolution_appearance() {
    let doc = edge_scene();
    let original = doc.clone();
    assert_eq!(
        render_flat(&doc).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let budget = SharedBudget::new(8 << 20);
    let expected = [255, 0, 0, 170, 255, 0, 0, 85];
    job(&budget, || {
        assert_eq!(render_flat(&doc)?, expected);
        assert_eq!(
            render_region(&doc, Rect::new(1, 0, 1, 1).unwrap())?,
            expected[4..]
        );
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        for partition in [
            Partition::Bands { rows: 1 },
            Partition::Tiles { edge: 1 },
            Partition::Tiles { edge: 32 },
        ] {
            assert_eq!(
                render_in_pool(
                    &pool,
                    &doc,
                    doc.canvas_rect(),
                    partition,
                    1 << 20,
                    || Ok(())
                )?,
                expected
            );
        }
        let mut cache = cache();
        for domain in [Domain::Committed, Domain::Preview] {
            assert_eq!(
                cache
                    .render(&doc, doc.canvas_rect(), domain, || Ok(()))?
                    .pixels,
                expected
            );
        }
        let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1)?;
        assert_eq!(
            render_display_cancellable(&doc, request, || Ok(()))?.pixels,
            [255, 0, 0, 128]
        );
        Ok(())
    });
    assert_eq!(budget.usage().reserved, 0);
    assert!(doc.same_semantics(&original));
    assert_eq!(doc.state_id, original.state_id);
}

#[test]
fn transparent_roi_imports_halo_alpha_and_halo_only_edits_cannot_hit_a_stale_cache() {
    let mut doc = new_document(300, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.origin = (-1, 0);
    layer.filters = filter();
    layer
        .raster
        .fill_rect(
            &Rect::new(255, 0, 1, 1).unwrap(),
            Rgba([255, 0, 0, 255]),
            BlendMode::Replace,
            &mut Admission::new(1 << 20),
        )
        .unwrap();
    let roi = Rect::new(255, 0, 1, 1).unwrap(); // local 256; changed neighbor is local255.
    assert_eq!(doc.layers[&id].raster.get(256, 0), Rgba::TRANSPARENT);
    let identity = (doc.document_id, doc.state_id);
    let budget = SharedBudget::new(8 << 20);
    let mut cache = cache();
    job(&budget, || {
        assert_eq!(
            cache
                .render(&doc, roi, Domain::Committed, || Ok(()))?
                .pixels,
            [255, 0, 0, 85]
        );
        let request = DisplayRequest::new(&doc, roi, 0)?;
        assert_eq!(
            render_display_cancellable(&doc, request, || Ok(()))?.pixels,
            [255, 0, 0, 85]
        );
        Ok(())
    });
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .fill_rect(
            &Rect::new(255, 0, 1, 1).unwrap(),
            Rgba([0, 255, 0, 255]),
            BlendMode::Replace,
            &mut Admission::new(1 << 20),
        )
        .unwrap();
    assert_eq!((doc.document_id, doc.state_id), identity);
    job(&budget, || {
        let frame = cache.render(&doc, roi, Domain::Committed, || Ok(()))?;
        assert_eq!(frame.pixels, [0, 255, 0, 85]);
        assert_eq!(frame.stats.cache_hits, 0);
        assert_eq!(frame.stats.patched_tiles, 0);
        Ok(())
    });
    assert_eq!(budget.usage().reserved, 0);
}

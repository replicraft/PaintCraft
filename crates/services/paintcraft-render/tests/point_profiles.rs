//! Literal cross-channel, once-masked, cache and nonlinear reduction controls.
use paintcraft_core::types::{
    Rect, Rgba,
    filters::{BlackWhiteV1, Descriptor, PosterizeRgbV1, ThresholdLumaV1},
    ids::deterministic_id,
    resources::SharedBudget,
};
use paintcraft_document::{
    Document, FilterInstance, FilterMask, FilterStack, POLICY, new_document,
};
use paintcraft_raster::{
    Admission, RasterLayer,
    coverage_mask::{A8Mask, Limits},
};
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    display::{DisplayRequest, render_display_cancellable},
    render_flat, render_region,
};

const SOURCE: [u8; 8] = [20, 115, 0, 255, 0, 178, 4, 255];

fn scene(descriptor: Descriptor, raw: &[u8], masked: bool) -> Document {
    let mut doc = new_document(2, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.raster =
        RasterLayer::from_rgba(2, 1, Rgba::TRANSPARENT, raw, &mut Admission::new(1 << 20)).unwrap();
    let mut instance =
        FilterInstance::new(deterministic_id(&["new-point-source"]), true, descriptor);
    if masked {
        let mask = A8Mask::uniform(
            Rect::new(0, 0, 2, 1).unwrap(),
            128,
            &SharedBudget::new(4096),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        instance = instance.with_mask(Some(FilterMask::new(true, mask)));
    }
    layer.filters = FilterStack::new(vec![instance]).unwrap();
    doc
}

#[test]
fn literal_fractional_profiles_use_the_original_rgb_tuple_and_invalidate_parameter_caches() {
    let cases = [
        (
            Descriptor::BlackWhiteRgbV1(BlackWhiteV1::default()),
            [54, 101, 44, 255, 64, 153, 66, 255],
        ),
        (
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::default()),
            [10, 57, 0, 255, 0, 89, 2, 255],
        ),
        (
            Descriptor::PosterizeRgbV1(PosterizeRgbV1::default()),
            [10, 108, 0, 255, 0, 165, 2, 255],
        ),
        (Descriptor::SolarizeRgbV1, [20, 115, 0, 255, 0, 127, 4, 255]),
    ];
    for (descriptor, expected) in cases {
        let doc = scene(descriptor, &SOURCE, true);
        let layer = &doc.layers[&doc.roots[0]];
        let source = layer.raster.clone();
        assert_eq!(render_flat(&doc).unwrap(), expected);
        assert_eq!(
            render_region(&doc, Rect::new(1, 0, 1, 1).unwrap()).unwrap(),
            expected[4..]
        );
        assert_eq!(layer.raster, source);
    }
    let mut doc = scene(
        Descriptor::BlackWhiteRgbV1(BlackWhiteV1::default()),
        &SOURCE,
        true,
    );
    let state = doc.state_id;
    let id = doc.roots[0];
    let raster = doc.layers[&id].raster.clone();
    let original = doc.layers[&id].filters.as_slice()[0].clone();
    let digest = original.mask().unwrap().coverage().digest();
    let mut cache = CompositeCache::new(CacheLimits {
        tile_edge: 32,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 32,
    })
    .unwrap();
    let first = cache
        .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
        .unwrap();
    assert_eq!(first.pixels, [54, 101, 44, 255, 64, 153, 66, 255]);
    let hit = cache
        .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
        .unwrap();
    assert!(hit.stats.cache_hits > 0);
    doc.layers.get_mut(&id).unwrap().filters =
        FilterStack::new(vec![original.with_descriptor(Descriptor::BlackWhiteRgbV1(
            BlackWhiteV1::new(0).unwrap(),
        ))])
        .unwrap();
    let changed = cache
        .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
        .unwrap();
    assert_eq!(changed.pixels, SOURCE);
    assert_eq!(changed.stats.cache_hits, 0);
    assert_eq!(doc.state_id, state);
    assert_eq!(doc.layers[&id].raster, raster);
    assert_eq!(
        doc.layers[&id].filters.as_slice()[0]
            .mask()
            .unwrap()
            .coverage()
            .digest(),
        digest
    );
}

#[test]
fn threshold_cpu_display_evaluates_before_averaging_source_samples() {
    let doc = scene(
        Descriptor::ThresholdLumaV1(ThresholdLumaV1::default()),
        &[255, 0, 0, 255, 0, 255, 0, 255],
        false,
    );
    assert_eq!(
        render_flat(&doc).unwrap(),
        [0, 0, 0, 255, 255, 255, 255, 255]
    );
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1).unwrap();
    let reduced = render_display_cancellable(&doc, request, || Ok(())).unwrap();
    // Thresholding each red/green sample gives black/white, averaging to128.
    // Thresholding their averaged encoded source would incorrectly give black.
    assert_eq!(reduced.pixels, [128, 128, 128, 255]);
}

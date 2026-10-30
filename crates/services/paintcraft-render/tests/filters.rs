use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    filters::{BrightnessContrastV1, Descriptor},
    ids::deterministic_id,
    picker::SampleSize,
    resources::SharedBudget,
};
use paintcraft_document::{
    Document, POLICY,
    filters::{FilterInstance, FilterStack},
    new_document,
};
use paintcraft_raster::{Admission, BlendMode, selection::Selection};
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    clipboard::capture_visible,
    display::{DisplayRequest, render_display_cancellable},
    parallel::{Partition, render_in_pool},
    render_flat, render_region, render_region_cancellable,
    sampling::sample_color,
    thumbnail,
};
use std::cell::Cell;

fn bc(b: i16, c: i16) -> Descriptor {
    Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(b, c).unwrap())
}
fn stack(filters: &[(bool, Descriptor)]) -> FilterStack {
    FilterStack::new(
        filters
            .iter()
            .enumerate()
            .map(|(index, (enabled, descriptor))| {
                FilterInstance::new(
                    deterministic_id(&["render-filter", &index.to_string()]),
                    *enabled,
                    *descriptor,
                )
            })
            .collect(),
    )
    .unwrap()
}
fn document() -> Document {
    new_document(6, 3, Some(Rgba([0, 100, 255, 255])), 9, POLICY).unwrap()
}

#[test]
fn implicit_and_dense_appearances_precede_opacity_without_mutating_source() {
    let mut doc = document();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    let mut admission = Admission::new(1 << 20);
    for (x, color) in [(1, Rgba([100, 50, 0, 128])), (2, Rgba([9, 20, 30, 0]))] {
        layer
            .raster
            .fill_rect(
                &Rect::new(x, 1, 1, 1).unwrap(),
                color,
                BlendMode::Replace,
                &mut admission,
            )
            .unwrap();
    }
    let original = layer.raster.clone();
    let versions: Vec<_> = original
        .tile_leases()
        .map(|(coord, tile)| (coord, tile.version()))
        .collect();
    layer.opacity = 128;
    layer.filters = stack(&[(true, bc(10, 0)), (true, Descriptor::InvertRgbV1)]);
    let pixels = render_flat(&doc).unwrap();
    let mut expected = [229, 129, 0, 128].repeat(18);
    expected[(6 + 1) * 4..(6 + 2) * 4].copy_from_slice(&[129, 179, 229, 64]);
    expected[(6 + 2) * 4..(6 + 3) * 4].fill(0);
    assert_eq!(pixels, expected);
    assert_eq!(doc.layers[&id].raster, original);
    assert_eq!(
        doc.layers[&id]
            .raster
            .tile_leases()
            .map(|(coord, tile)| (coord, tile.version()))
            .collect::<Vec<_>>(),
        versions
    );
}

#[test]
fn parameter_enable_and_order_changes_invalidate_cached_default_pixels() {
    let mut doc = document();
    let id = doc.roots[0];
    let state = doc.state_id;
    let mut cache = CompositeCache::new(CacheLimits {
        tile_edge: 32,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 32,
    })
    .unwrap();
    for (filters, color) in [
        (vec![], [0, 100, 255, 255]),
        (vec![(true, bc(10, 0))], [26, 126, 255, 255]),
        (
            vec![(true, bc(10, 0)), (true, Descriptor::InvertRgbV1)],
            [229, 129, 0, 255],
        ),
        (
            vec![(true, Descriptor::InvertRgbV1), (true, bc(10, 0))],
            [255, 181, 26, 255],
        ),
        (
            vec![(false, Descriptor::InvertRgbV1), (true, bc(10, 0))],
            [26, 126, 255, 255],
        ),
    ] {
        doc.layers.get_mut(&id).unwrap().filters = stack(&filters);
        let frame = cache
            .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
            .unwrap();
        assert_eq!(frame.pixels, color.repeat(18));
        assert!(frame.stats.recomposed_pixels > 0);
        let hit = cache
            .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
            .unwrap();
        assert_eq!(hit.pixels, frame.pixels);
        assert_eq!(hit.stats.recomposed_pixels, 0);
        assert_eq!(
            doc.state_id, state,
            "cache depends on content, even without a host state-id update"
        );
        assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
    }
}

#[test]
fn huge_implicit_layer_reduces_without_materializing_source_or_native_canvas() {
    let mut doc = new_document(65536, 65536, Some(Rgba([10, 30, 50, 255])), 4, POLICY).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = stack(&[(true, Descriptor::InvertRgbV1)]);
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 12).unwrap();
    let frame = render_display_cancellable(&doc, request, || Ok(())).unwrap();
    assert_eq!(frame.pixels, [245, 225, 205, 255].repeat(16 * 16));
    assert_eq!(frame.stats.native_pixels, 0);
    assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
}

#[test]
fn region_parallel_thumbnail_picker_and_visible_copy_share_the_appearance() {
    let mut doc = document();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.origin = (-1, 1);
    layer.filters = stack(&[(true, Descriptor::InvertRgbV1)]);
    let expected = render_flat(&doc).unwrap();
    assert_eq!(&expected[..6 * 4], &[0; 24]);
    assert_eq!(&expected[6 * 4..6 * 4 + 4], &[255, 155, 0, 255]);
    assert_eq!(
        render_region(&doc, Rect::new(0, 1, 5, 2).unwrap()).unwrap(),
        [255, 155, 0, 255].repeat(10)
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    for partition in [Partition::Bands { rows: 1 }, Partition::Tiles { edge: 2 }] {
        assert_eq!(
            render_in_pool(
                &pool,
                &doc,
                doc.canvas_rect(),
                partition,
                1 << 20,
                || Ok(())
            )
            .unwrap(),
            expected
        );
    }
    assert_eq!(
        thumbnail::render_document(&doc, 16, || Ok(()))
            .unwrap()
            .pixels,
        expected
    );
    assert_eq!(
        sample_color(&doc, None, (0, 1), SampleSize::One, || Ok(())).unwrap(),
        Rgba([255, 155, 0, 255])
    );
    assert_eq!(
        sample_color(
            &doc,
            Some(&doc.layers[&id]),
            (0, 1),
            SampleSize::One,
            || Ok(())
        )
        .unwrap(),
        Rgba([0, 100, 255, 255]),
        "raw-layer sampling is explicitly source pixels"
    );
    let budget = SharedBudget::new(1 << 20);
    let copy = capture_visible(&doc, &Selection::Inactive, &budget, &|| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(copy.1, (0, 0));
    assert_eq!(copy.0.rgba(), expected);
}

#[test]
fn cancellation_discards_private_filtered_output_and_keeps_source_and_stack() {
    let mut doc = document();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = stack(&[(true, Descriptor::InvertRgbV1)]);
    let before = doc.clone();
    let calls = Cell::new(0);
    let result = render_region_cancellable(&doc, doc.canvas_rect(), || {
        calls.set(calls.get() + 1);
        if calls.get() == 5 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "test cancellation",
            ))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    assert!(doc.same_semantics(&before));
    assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
}

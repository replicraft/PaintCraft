//! Literal GPU framebuffer controls; expected images do not call CPU rendering.
use super::*;
use paintcraft_core::types::{Rect, filters::Descriptor, resources::SharedBudget};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};
use paintcraft_render::display::DisplayRequest;

const COLORS: [[u8; 4]; 6] = [
    [10, 60, 200, 255],
    [11, 61, 199, 255],
    [127, 127, 128, 255],
    [128, 128, 127, 255],
    [244, 194, 56, 255],
    [245, 195, 55, 255],
];
fn mask() -> A8Mask {
    let values = [0, 1, 127, 128, 254, 255];
    let runs: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(x, coverage)| Run {
            x0: x as i64,
            x1: x as i64 + 1,
            coverage: *coverage,
        })
        .collect();
    A8Mask::from_bands(
        Rect::new(0, 0, 6, 2).unwrap(),
        &[Band {
            y0: 0,
            y1: 2,
            runs: &runs,
        }],
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn scene() -> Document {
    let mut doc = new_document(6, 2, Some(Rgba([10, 60, 200, 255])), 8, POLICY).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(
            deterministic_id(&["metal-mask"]),
            true,
            Descriptor::InvertRgbV1,
        )
        .with_mask(Some(FilterMask::new(true, mask()))),
    ])
    .unwrap();
    doc
}

#[test]
fn literal_masked_implicit_pixels_reach_general_single_and_scaled_gpu_paths() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let gpu = GpuRenderer::new(device, queue)
            .unwrap()
            .with_single_layer_display(single);
        let mut doc = scene();
        let id = doc.roots[0];
        let state = doc.state_id;
        assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
        let expected = COLORS.as_flattened().repeat(2);
        assert_eq!(
            gpu.render_with_stats(&doc, || Ok(())).unwrap().pixels,
            expected
        );
        let hit = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert_eq!(hit.pixels, expected);
        assert!(hit.stats.source_hits > 0);
        for (level, expected) in [
            (0, COLORS.as_flattened().repeat(2)),
            (
                1,
                vec![11, 61, 200, 255, 128, 128, 128, 255, 245, 195, 56, 255],
            ),
            (2, vec![69, 94, 164, 255, 245, 195, 56, 255]),
        ] {
            let request = DisplayRequest::new(&doc, doc.canvas_rect(), level).unwrap();
            let frame = gpu
                .render_display_texture(&doc, request, || Ok(()))
                .unwrap();
            assert_eq!(
                frame.readback(|| Ok(())).unwrap(),
                expected,
                "single={single} level={level}"
            );
        }
        let old = doc.layers[&id].filters.as_slice()[0].clone();
        let zero = A8Mask::uniform(
            Rect::new(0, 0, 6, 2).unwrap(),
            0,
            &SharedBudget::new(4096),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        doc.layers.get_mut(&id).unwrap().filters =
            FilterStack::new(vec![old.with_mask(Some(FilterMask::new(true, zero)))]).unwrap();
        let changed = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert_eq!(changed.pixels, [10, 60, 200, 255].repeat(12));
        assert!(
            changed.stats.source_allocations > 0,
            "mask-only change cannot reuse old upload"
        );
        assert_eq!(doc.state_id, state);
        assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
    }
}

#[test]
fn masked_source_local_coordinates_and_alpha_precede_gpu_opacity_and_backdrop() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut doc = new_document(5, 2, Some(Rgba([3, 90, 150, 255])), 3, POLICY).unwrap();
    let id = deterministic_id(&["metal-mask-offset"]);
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "offset".into(),
            visible: true,
            opacity: 128,
            origin: (-1, 0),
            raster: RasterLayer::new(6, 2, Rgba([10, 60, 200, 128])),
            filters: FilterStack::new(vec![
                FilterInstance::new(
                    deterministic_id(&["metal-mask-offset-filter"]),
                    true,
                    Descriptor::InvertRgbV1,
                )
                .with_mask(Some(FilterMask::new(true, mask()))),
            ])
            .unwrap(),
        },
    );
    // Compute a distinct real-valued reference for the already literal masked
    // channels. Layer alpha128 at opacity128 rounds to64, then source-over an
    // opaque backdrop. Cropping at origin-1 selects source-local columns1..5.
    let expected_row: Vec<_> = COLORS[1..]
        .iter()
        .flat_map(|color| {
            let mut out = [0, 0, 0, 255];
            for (index, back) in [3.0, 90.0, 150.0].into_iter().enumerate() {
                out[index] =
                    ((f64::from(color[index]) * 64.0 + back * 191.0) / 255.0).round() as u8;
            }
            out
        })
        .collect();
    assert_eq!(
        gpu.render_with_stats(&doc, || Ok(())).unwrap().pixels,
        expected_row.repeat(2)
    );
    assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
    doc.layers.get_mut(&id).unwrap().visible = false;
    assert_eq!(
        gpu.render_with_stats(&doc, || Ok(())).unwrap().pixels,
        [3, 90, 150, 255].repeat(10)
    );
}

use super::*;
use paintcraft_core::types::filters::{BrightnessContrastV1, Descriptor};
use paintcraft_document::filters::{FilterInstance, FilterStack};
use paintcraft_render::display::{DisplayRequest, render_display_cancellable};

fn stack(descriptor: Descriptor) -> FilterStack {
    FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["gpu-filter"]),
        true,
        descriptor,
    )])
    .unwrap()
}

#[test]
fn prepared_cpu_filters_reach_gpu_composition_cache_and_display_paths() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue)
        .unwrap()
        .with_single_layer_display(true);
    let mut doc = new_document(259, 7, Some(Rgba([0, 100, 255, 255])), 12, POLICY).unwrap();
    let id = doc.roots[0];
    let mut admission = Admission::new(1 << 20);
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .fill_rect(
            &paintcraft_core::types::Rect::new(254, 0, 5, 7).unwrap(),
            Rgba([100, 50, 0, 128]),
            paintcraft_raster::BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    for descriptor in [
        Descriptor::InvertRgbV1,
        Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(10, 0).unwrap()),
    ] {
        doc.layers.get_mut(&id).unwrap().filters = stack(descriptor);
        let expected = render_flat(&doc).unwrap();
        let frame = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert_eq!(frame.pixels, expected);
        assert!(
            frame.stats.source_allocations > 0,
            "parameter-only edit cannot reuse raw/older appearances"
        );
        let hit = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert_eq!(hit.pixels, expected);
        assert!(hit.stats.source_hits > 0);
        for level in [0, 1, 3] {
            let request = DisplayRequest::new(&doc, doc.canvas_rect(), level).unwrap();
            let frame = gpu
                .render_display_texture(&doc, request, || Ok(()))
                .unwrap();
            let straight = render_display_cancellable(&doc, request, || Ok(()))
                .unwrap()
                .pixels;
            let premultiplied: Vec<_> = straight
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|pixel| {
                    let alpha = u32::from(pixel[3]);
                    [
                        ((u32::from(pixel[0]) * alpha + 127) / 255) as u8,
                        ((u32::from(pixel[1]) * alpha + 127) / 255) as u8,
                        ((u32::from(pixel[2]) * alpha + 127) / 255) as u8,
                        pixel[3],
                    ]
                })
                .collect();
            assert_eq!(frame.readback(|| Ok(())).unwrap(), premultiplied);
        }
    }
    assert_eq!(
        doc.layers[&id].raster.default_pixel(),
        Rgba([0, 100, 255, 255])
    );
}

use super::*;
use paintcraft_core::types::{ErrorCode, Rect};
use paintcraft_render::render_region;

#[test]
fn regional_gpu_output_rebases_offsets_and_preserves_source_scope() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut doc = new_document(601, 311, Some(Rgba([17, 29, 41, 123])), 0, POLICY).unwrap();
    let mut admission = Admission::new(2_000_000);
    for (n, origin) in [(-13, -9), (251, 65), (590, 300)].into_iter().enumerate() {
        let id = deterministic_id(&["regional-gpu", &n.to_string()]);
        let mut raster = RasterLayer::new(271, 99, Rgba([93, 71, 37, 81]));
        raster
            .fill_rect(
                &Rect::new(1, 2, 260, 90).unwrap(),
                Rgba([127, 203, 59, 167]),
                paintcraft_raster::BlendMode::SourceOver,
                &mut admission,
            )
            .unwrap();
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: "offset".into(),
                origin,
                visible: true,
                opacity: 137,
                raster,
            },
        );
    }
    for region in [
        Rect::new(0, 0, 17, 9).unwrap(),
        Rect::new(253, 63, 263, 101).unwrap(),
        Rect::new(589, 299, 12, 12).unwrap(),
        Rect::new(600, 310, 1, 1).unwrap(),
    ] {
        let expected = render_region(&doc, region).unwrap();
        let frame = gpu
            .render_region_with_stats(&doc, region, || Ok(()))
            .unwrap();
        assert_eq!(frame.pixels, expected);
        let warm = gpu
            .render_region_with_stats(&doc, region, || Ok(()))
            .unwrap();
        assert_eq!(warm.pixels, expected);
        assert_eq!(warm.stats.uploaded_bytes, 0);
        let display = gpu.render_region_texture(&doc, region, || Ok(())).unwrap();
        assert_eq!(display.region(), region);
        assert_eq!(
            display.size(),
            [region.width() as u32, region.height() as u32]
        );
        let premultiplied: Vec<_> = expected
            .chunks_exact(4)
            .flat_map(|p| {
                let a = u32::from(p[3]);
                [
                    ((u32::from(p[0]) * a + 127) / 255) as u8,
                    ((u32::from(p[1]) * a + 127) / 255) as u8,
                    ((u32::from(p[2]) * a + 127) / 255) as u8,
                    p[3],
                ]
            })
            .collect();
        assert_eq!(display.readback(|| Ok(())).unwrap(), premultiplied);
    }
    parity(&gpu, &doc); // Regional output must not poison later full exports.
}

#[test]
fn small_region_of_large_canvas_is_admitted_and_invalid_regions_are_rejected() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let doc = new_document(16384, 16384, Some(Rgba([9, 8, 7, 255])), 0, POLICY).unwrap();
    assert!(gpu.render(&doc).is_err());
    let region = Rect::new(16001, 16003, 127, 93).unwrap();
    assert_eq!(
        gpu.render_region_with_stats(&doc, region, || Ok(()))
            .unwrap()
            .pixels,
        [9, 8, 7, 255].repeat(127 * 93)
    );
    let frame = gpu.render_region_texture(&doc, region, || Ok(())).unwrap();
    assert_eq!(
        frame.readback(|| Ok(())).unwrap(),
        [9, 8, 7, 255].repeat(127 * 93)
    );
    for invalid in [
        Rect::new(-1, 0, 1, 1).unwrap(),
        Rect::new(16384, 0, 1, 1).unwrap(),
        Rect {
            x0: 4,
            y0: 4,
            x1: 4,
            y1: 5,
        },
        Rect {
            x0: i64::MIN,
            y0: 0,
            x1: i64::MAX,
            y1: 1,
        },
    ] {
        assert_eq!(
            gpu.render_region_with_stats(&doc, invalid, || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            gpu.render_region_texture(&doc, invalid, || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    let error = gpu
        .render_region_texture(&doc, region, || {
            Err(paintcraft_core::types::Error::new(
                ErrorCode::OperationCancelled,
                "cancel regional frame",
            ))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(gpu.source_cache_usage().buffer_bytes, 0);
    assert!(gpu.render_region_texture(&doc, region, || Ok(())).is_ok());
}

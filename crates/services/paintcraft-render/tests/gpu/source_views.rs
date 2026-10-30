use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect};
use paintcraft_render::{
    display::{DisplayRequest, render_display_cancellable},
    gpu::{DeviceEpoch, SourceCacheLimits},
};

fn fixture() -> Document {
    let mut doc = new_document(512, 256, None, 0, POLICY).unwrap();
    let mut admission = Admission::new(16 << 20);
    for n in 0..5u32 {
        let id = deterministic_id(&["regional-source", &n.to_string()]);
        let pixels: Vec<_> = (0..512 * 256u32)
            .flat_map(|p| {
                [
                    (p + n * 53) as u8,
                    (p / 512 * 17) as u8,
                    (p * 31 + n * 7) as u8,
                    [0, 1, 64, 128, 254, 255][p as usize % 6],
                ]
            })
            .collect();
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: format!("Source {n}"),
                origin: (0, 0),
                visible: true,
                opacity: 173,
                filters: Default::default(),
                raster: RasterLayer::from_rgba(
                    512,
                    256,
                    Rgba([31, 63, 127, 0]),
                    &pixels,
                    &mut admission,
                )
                .unwrap(),
            },
        );
    }
    doc
}
fn gpu(entries: usize, enabled: bool) -> Option<GpuRenderer> {
    let adapter = adapter()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    Some(
        GpuRenderer::with_source_cache(
            device,
            queue,
            DeviceEpoch::default(),
            SourceCacheLimits {
                entries,
                ..Default::default()
            },
        )
        .unwrap()
        .with_scaled_tile_edge(64)
        .unwrap()
        .with_region_source_cache(enabled),
    )
}
fn expected(doc: &Document, request: DisplayRequest) -> Vec<u8> {
    render_display_cancellable(doc, request, || Ok(()))
        .unwrap()
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = u32::from(p[3]);
            [
                ((u32::from(p[0]) * a + 127) / 255) as u8,
                ((u32::from(p[1]) * a + 127) / 255) as u8,
                ((u32::from(p[2]) * a + 127) / 255) as u8,
                p[3],
            ]
        })
        .collect()
}

#[test]
fn regional_sources_reuse_complete_scaled_working_set_and_patch_only_changed_rows() {
    let Some(gpu) = gpu(64, true) else { return };
    let mut doc = fixture();
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1).unwrap();
    let old_bytes = expected(&doc, request);
    let old = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    assert_eq!(old.stats.source_allocations, 40);
    assert_eq!(old.stats.retained.buffers, 40);
    let warm = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    assert_eq!(warm.stats.source_hits, 40);
    assert_eq!(warm.stats.uploaded_bytes, 0);
    assert_eq!(warm.readback(|| Ok(())).unwrap(), old_bytes);
    let id = doc.roots[4];
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            7,
            9,
            Rgba([211, 13, 97, 177]),
            paintcraft_raster::BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let patched = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    assert_eq!(patched.stats.source_hits, 39);
    assert_eq!(patched.stats.patched_sources, 1);
    assert_eq!(patched.stats.source_allocations, 0);
    assert_eq!(patched.stats.uploaded_bytes, 128 * 64 * 4);
    assert_eq!(
        patched.readback(|| Ok(())).unwrap(),
        expected(&doc, request)
    );
    doc.layers.get_mut(&id).unwrap().opacity = 71;
    doc.roots.reverse();
    let metadata = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    assert_eq!(metadata.stats.source_hits, 40);
    assert_eq!(metadata.stats.uploaded_bytes, 0);
    assert_eq!(
        metadata.readback(|| Ok(())).unwrap(),
        expected(&doc, request)
    );
    doc.layers.get_mut(&id).unwrap().origin = (-3, 5);
    let moved = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    assert_eq!(moved.stats.source_hits, 32);
    assert_eq!(moved.stats.source_allocations, 8);
    assert_eq!(moved.readback(|| Ok(())).unwrap(), expected(&doc, request));
    assert_eq!(old.readback(|| Ok(())).unwrap(), old_bytes);
    parity(&gpu, &doc);
    assert_eq!(old.readback(|| Ok(())).unwrap(), old_bytes);
}

#[test]
fn scaled_frame_protects_early_sources_and_streams_overflow_without_thrashing() {
    let Some(gpu) = gpu(3, true) else { return };
    let doc = fixture();
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1).unwrap();
    let pixels = expected(&doc, request);
    let first = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    assert_eq!(first.stats.source_allocations, 40);
    assert_eq!(first.stats.retained.buffers, 3);
    let second = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    assert_eq!(second.stats.source_hits, 3);
    assert_eq!(second.stats.source_allocations, 37);
    assert_eq!(second.stats.retained.buffers, 3);
    assert!(second.stats.retained.buffer_bytes <= 3 * 128 * 128 * 4);
    assert_eq!(second.readback(|| Ok(())).unwrap(), pixels);
    assert_eq!(first.readback(|| Ok(())).unwrap(), pixels);
}

#[test]
fn regional_source_cancellation_clears_partial_updates_and_old_frames_stay_readable() {
    use std::cell::Cell;
    let Some(gpu) = gpu(64, true) else { return };
    let mut doc = fixture();
    let request = DisplayRequest::new(&doc, Rect::new(0, 0, 129, 65).unwrap(), 1).unwrap();
    let old = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    let old_bytes = old.readback(|| Ok(())).unwrap();
    let original = doc.clone();
    let id = doc.roots[0];
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            1,
            1,
            Rgba([255; 4]),
            paintcraft_raster::BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let calls = Cell::new(0);
    gpu.render_display_texture(&doc, request, || {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let total = calls.get();
    // Rewarm the old revision so each checkpoint covers an actual dirty transfer.
    for stop in 1..=total {
        gpu.render_display_texture(&original, request, || Ok(()))
            .unwrap();
        calls.set(0);
        let result = gpu.render_display_texture(&doc, request, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "regional source checkpoint",
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(gpu.source_cache_usage().buffers, 0);
        assert_eq!(old.readback(|| Ok(())).unwrap(), old_bytes);
    }
    let retry = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    assert_eq!(retry.readback(|| Ok(())).unwrap(), expected(&doc, request));
}

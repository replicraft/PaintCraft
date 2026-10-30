use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect};
use paintcraft_raster::BlendMode;
use paintcraft_render::gpu::{DeviceEpoch, SourceCacheLimits};

fn renderer(retain: bool) -> Option<GpuRenderer> {
    let adapter = adapter()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    Some(
        GpuRenderer::with_source_cache(
            device,
            queue,
            DeviceEpoch::default(),
            if retain {
                SourceCacheLimits::default()
            } else {
                SourceCacheLimits {
                    entries: 0,
                    metadata_bytes: 0,
                    buffer_bytes: 0,
                }
            },
        )
        .unwrap()
        .with_single_layer_display(true),
    )
}
fn expected(doc: &Document, region: Rect) -> Vec<u8> {
    paintcraft_render::render_region(doc, region)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let alpha = u32::from(p[3]);
            [
                ((u32::from(p[0]) * alpha + 127) / 255) as u8,
                ((u32::from(p[1]) * alpha + 127) / 255) as u8,
                ((u32::from(p[2]) * alpha + 127) / 255) as u8,
                p[3],
            ]
        })
        .collect()
}

#[test]
fn fused_display_matches_independent_all_alpha_opacity_pairs() {
    let Some(gpu) = renderer(true) else { return };
    let mut doc = new_document(256, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let raw: Vec<_> = (0..=255u8)
        .flat_map(|a| [a.wrapping_mul(17), 63, 255 - a, a])
        .collect();
    doc.layers.get_mut(&id).unwrap().raster = RasterLayer::from_rgba(
        256,
        1,
        Rgba([19, 83, 167, 0]),
        &raw,
        &mut Admission::new(1 << 20),
    )
    .unwrap();
    for opacity in 0..=255u8 {
        doc.layers.get_mut(&id).unwrap().opacity = opacity;
        let frame = gpu.render_texture(&doc, || Ok(())).unwrap();
        assert_eq!(frame.stats.single_layer_displays, u64::from(opacity != 0));
        if opacity != 0 {
            assert_eq!(frame.stats.output_allocations, 0);
        }
        let oracle: Vec<_> = raw
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let alpha = (u32::from(p[3]) * u32::from(opacity) + 127) / 255;
                [
                    ((u32::from(p[0]) * alpha + 127) / 255) as u8,
                    ((u32::from(p[1]) * alpha + 127) / 255) as u8,
                    ((u32::from(p[2]) * alpha + 127) / 255) as u8,
                    alpha as u8,
                ]
            })
            .collect();
        assert_eq!(
            frame.readback(|| Ok(())).unwrap(),
            oracle,
            "opacity={opacity}"
        );
    }
    assert_eq!(doc.layers[&id].raster.get(0, 0), Rgba([0, 63, 255, 0]));
}

#[test]
fn fused_display_reuses_and_patches_sources_without_aliasing_returned_frames() {
    let Some(gpu) = renderer(true) else { return };
    let mut doc = new_document(263, 137, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.origin = (-3, -2);
    layer.raster = RasterLayer::new(270, 145, Rgba([31, 63, 127, 0]));
    let mut admission = Admission::new(4 << 20);
    layer
        .raster
        .blend_pixel(
            10,
            70,
            Rgba([199, 87, 31, 193]),
            BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    let region = Rect::new(2, 3, 257, 130).unwrap();
    let original = doc.clone();
    let before = expected(&doc, region);
    let old = gpu.render_region_texture(&doc, region, || Ok(())).unwrap();
    assert_eq!(old.stats.single_layer_displays, 1);
    assert_eq!(old.stats.output_allocations, 0);
    assert_eq!(old.stats.source_allocations, 1);
    assert_eq!(old.readback(|| Ok(())).unwrap(), before);
    assert!(doc.same_semantics(&original));
    doc.layers.get_mut(&id).unwrap().opacity = 137;
    let warm = gpu.render_region_texture(&doc, region, || Ok(())).unwrap();
    assert_eq!(warm.stats.source_hits, 1);
    assert_eq!(warm.stats.uploaded_bytes, 0);
    assert_eq!(warm.readback(|| Ok(())).unwrap(), expected(&doc, region));
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            11,
            71,
            Rgba([17, 88, 156, 128]),
            BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    let patched = gpu.render_region_texture(&doc, region, || Ok(())).unwrap();
    assert_eq!(patched.stats.patched_sources, 1);
    assert_eq!(patched.stats.uploaded_bytes, 257 * 64 * 4);
    assert_eq!(patched.stats.composition_submissions, 0);
    assert_eq!(patched.readback(|| Ok(())).unwrap(), expected(&doc, region));
    assert_eq!(old.readback(|| Ok(())).unwrap(), before);
    // Explicit straight export remains on the original ordered compositor.
    assert_eq!(
        gpu.render_region_with_stats(&doc, region, || Ok(()))
            .unwrap()
            .pixels,
        paintcraft_render::render_region(&doc, region).unwrap()
    );
}

#[test]
fn partial_multiple_and_noncontributing_layers_use_exact_eligibility() {
    let Some(gpu) = renderer(true) else { return };
    let mut doc = new_document(19, 13, Some(Rgba([17, 88, 156, 128])), 0, POLICY).unwrap();
    let id = deterministic_id(&["single-display-extra"]);
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "extra".into(),
            origin: (0, 0),
            visible: true,
            opacity: 137,
            filters: Default::default(),
            raster: RasterLayer::new(19, 13, Rgba([199, 87, 31, 193])),
        },
    );
    for state in 0..6 {
        let extra = doc.layers.get_mut(&id).unwrap();
        extra.visible = state != 1;
        extra.opacity = if state == 2 { 0 } else { 137 };
        extra.origin = if state == 3 { (100, 100) } else { (0, 0) };
        extra.raster = RasterLayer::new(
            19,
            13,
            if state == 4 {
                Rgba([31, 63, 127, 0])
            } else {
                Rgba([199, 87, 31, 193])
            },
        );
        if state == 5 {
            doc.roots.remove(0);
            doc.layers.get_mut(&id).unwrap().origin = (1, 1);
        }
        let frame = gpu.render_texture(&doc, || Ok(())).unwrap();
        assert_eq!(
            frame.stats.single_layer_displays,
            u64::from((1..=4).contains(&state))
        );
        assert_eq!(
            frame.readback(|| Ok(())).unwrap(),
            expected(&doc, doc.canvas_rect())
        );
    }
}

#[test]
fn streamed_fused_sources_cancel_at_every_checkpoint_and_reject_retired_epochs() {
    let Some(gpu) = renderer(false) else { return };
    let doc = new_document(17, 13, Some(Rgba([17, 88, 156, 128])), 0, POLICY).unwrap();
    let calls = std::cell::Cell::new(0);
    let old = gpu
        .render_texture(&doc, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(old.stats.single_layer_displays, 1);
    assert_eq!(gpu.source_cache_usage().buffers, 0);
    let before = expected(&doc, doc.canvas_rect());
    for at in 1..=calls.get() {
        let n = std::cell::Cell::new(0);
        let error = gpu
            .render_texture(&doc, || {
                n.set(n.get() + 1);
                if n.get() == at {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "single display checkpoint",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(gpu.source_cache_usage().buffers, 0);
        assert_eq!(old.readback(|| Ok(())).unwrap(), before);
    }
    gpu.epoch().retire();
    assert_eq!(
        gpu.render_texture(&doc, || Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        old.readback(|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}

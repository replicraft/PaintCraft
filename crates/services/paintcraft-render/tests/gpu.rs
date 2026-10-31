#![cfg(feature = "gpu")]
use paintcraft_core::types::{Rgba, ids::deterministic_id};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_raster::{Admission, RasterLayer};
use paintcraft_render::{gpu::GpuRenderer, render_flat};

#[test]
fn gpu_matches_scalar_for_order_opacity_offsets_tiles_and_hidden_rgb() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut doc = new_document(263, 19, None, 0, POLICY).unwrap();
    assert_eq!(gpu.render(&doc).unwrap(), render_flat(&doc).unwrap());
    // Real smooth-brush edge alphas must survive both compositors identically.
    let id = deterministic_id(&["smooth-brush-gpu"]);
    let mut raster = RasterLayer::new(263, 19, Rgba::TRANSPARENT);
    let mut admission = Admission::new(2_000_000);
    for run in
        paintcraft_raster::smooth::round_path(&[(2, 2), (260, 16)], 7, &doc.canvas_rect()).unwrap()
    {
        let alpha = ((128u16 * u16::from(run.samples) + 8) / 16) as u8;
        raster
            .fill_rect(
                &paintcraft_core::types::Rect::new(run.x0, run.y, run.x1 - run.x0 + 1, 1).unwrap(),
                Rgba([32, 64, 128, alpha]),
                paintcraft_raster::BlendMode::SourceOver,
                &mut admission,
            )
            .unwrap();
    }
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "Brush".into(),
            origin: (0, 0),
            visible: true,
            opacity: 137,
            filters: Default::default(),
            raster,
        },
    );
    parity(&gpu, &doc);
    // Odd extents exercise workgroup tails; width crosses the sparse tile boundary.
    for (i, (origin, opacity, visible)) in [
        ((-3, -1), 255, true),
        ((1, 2), 128, true),
        ((0, 0), 0, true),
        ((0, 0), 200, false),
        ((256, 0), 241, true),
        ((2000, 0), 255, true),
    ]
    .into_iter()
    .enumerate()
    {
        let (w, h) = (270, 23);
        let bytes: Vec<u8> = (0..w * h)
            .flat_map(|p| {
                let n = (p + i as u32 * 53) as u8;
                [
                    n,
                    n.wrapping_mul(17),
                    255 - n,
                    if p % 7 == 0 { 0 } else { n },
                ]
            })
            .collect();
        let raster = RasterLayer::from_rgba(
            w,
            h,
            Rgba([91, 37, 11, 0]),
            &bytes,
            &mut Admission::new(2_000_000),
        )
        .unwrap();
        let id = deterministic_id(&["gpu-test", &i.to_string()]);
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: format!("Layer {i}"),
                origin,
                visible,
                opacity,
                filters: Default::default(),
                raster,
            },
        );
        parity(&gpu, &doc);
    }
    doc.roots.reverse();
    parity(&gpu, &doc);
    // A transparent sparse default is different from a missing layer, and must stay harmless.
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().raster = RasterLayer::new(270, 23, Rgba([123, 45, 67, 0]));
    parity(&gpu, &doc);
    let too_large = new_document(16384, 16384, None, 0, POLICY).unwrap();
    assert!(gpu.render(&too_large).is_err());
}
fn parity(gpu: &GpuRenderer, doc: &Document) {
    assert_eq!(
        gpu.render(doc).unwrap(),
        render_flat(doc).unwrap(),
        "GPU must exactly preserve integer compositing policy"
    );
}

fn adapter() -> Option<wgpu::Adapter> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
        Ok(a) => a,
        Err(e) => {
            assert!(
                std::env::var_os("PAINTCRAFT_REQUIRE_GPU").is_none(),
                "GPU required: {e}"
            );
            eprintln!("GPU unavailable; parity test skipped: {e}");
            return None;
        }
    };
    eprintln!("GPU parity adapter: {:?}", adapter.get_info());
    Some(adapter)
}

#[test]
fn destroyed_device_cannot_render_and_fresh_device_restores_committed_host_history() {
    use paintcraft_core::types::ErrorCode;
    use paintcraft_editor::session::Session;
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device.clone(), queue).unwrap();
    let epoch = gpu.epoch();
    let mut session = Session::new(new_document(8, 8, Some(Rgba([255; 4])), 0, POLICY).unwrap());
    let ticket = session.ticket();
    let id = session.document().roots[0];
    let batch = format!(
        r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":8,"height":8}},"color":"#17589CCC","mode":"replace"}}]}}"##,
        ticket.state_id, ticket.publication_epoch
    );
    assert!(session.apply_json(batch.as_bytes()).unwrap().committed);
    let committed = session.snapshot();
    let committed_ticket = session.ticket();
    let expected = [23, 88, 156, 204].repeat(64);
    assert_eq!(gpu.render(&committed).unwrap(), expected);
    assert!(gpu.source_cache_usage().buffers > 0);
    device.destroy();
    let _ = device.poll(wgpu::PollType::Poll);
    assert!(
        !epoch.is_current(),
        "actual device destruction must retire callback epoch"
    );
    assert_eq!(
        gpu.render(&committed).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(gpu.source_cache_usage().buffers, 0);
    assert_eq!(session.ticket(), committed_ticket);
    assert!(std::sync::Arc::ptr_eq(&committed, &session.snapshot()));
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let replacement = GpuRenderer::new(device, queue).unwrap();
    assert_ne!(replacement.epoch(), epoch);
    assert_eq!(replacement.render(session.document()).unwrap(), expected);
    session.undo().unwrap().unwrap();
    assert_eq!(
        replacement.render(session.document()).unwrap(),
        vec![255; 8 * 8 * 4]
    );
    session.redo().unwrap().unwrap();
    assert_eq!(replacement.render(session.document()).unwrap(), expected);
}

#[test]
fn host_callback_survives_construction_and_loss_during_staging_rejects_output() {
    use paintcraft_core::types::{Error, ErrorCode};
    use paintcraft_render::gpu::DeviceEpoch;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let epoch = DeviceEpoch::default();
    let lost = epoch.clone();
    let observed = Arc::new(AtomicBool::new(false));
    let notified = observed.clone();
    device.set_device_lost_callback(move |_, _| {
        lost.retire();
        notified.store(true, Ordering::Release);
    });
    let gpu = GpuRenderer::with_epoch(device.clone(), queue, epoch.clone()).unwrap();
    let doc = new_document(16, 16, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let error = gpu
        .render_cancellable(&doc, || {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancelled by owner",
            ))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(epoch.is_current());
    assert_eq!(gpu.render(&doc).unwrap(), vec![255; 16 * 16 * 4]);
    let calls = AtomicUsize::new(0);
    let error = gpu
        .render_cancellable(&doc, || {
            if calls.fetch_add(1, Ordering::Relaxed) == 2 {
                device.destroy();
                let _ = device.poll(wgpu::PollType::Poll);
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert!(observed.load(Ordering::Acquire));
    assert!(!epoch.is_current());
    assert_eq!(render_flat(&doc).unwrap(), vec![255; 16 * 16 * 4]);
}

#[test]
fn resident_sources_skip_unchanged_uploads_and_patch_clipped_row_strips() {
    use paintcraft_raster::BlendMode;
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut doc = new_document(263, 129, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = deterministic_id(&["resident-source"]);
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "Ink".into(),
            origin: (-3, 5),
            visible: true,
            opacity: 173,
            filters: Default::default(),
            raster: RasterLayer::new(270, 140, Rgba([31, 63, 127, 0])),
        },
    );
    // Keep a resident source (including hidden RGB) so the warm-strip test is
    // independent of the separate empty-transparent-layer bypass.
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            4,
            4,
            Rgba([1, 2, 3, 0]),
            paintcraft_raster::BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let cold = gpu.render_with_stats(&doc, || Ok(())).unwrap();
    assert_eq!(cold.pixels, render_flat(&doc).unwrap());
    assert_eq!(cold.stats.uploaded_bytes, 263 * (129 + 124) * 4);
    assert_eq!(cold.stats.source_allocations, 2);
    let warm = gpu.render_with_stats(&doc, || Ok(())).unwrap();
    assert_eq!(warm.pixels, cold.pixels);
    assert_eq!(warm.stats.uploaded_bytes, 0);
    assert_eq!(warm.stats.source_hits, 2);
    assert_eq!(warm.stats.source_allocations, 0);
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            65,
            70,
            Rgba([17, 88, 156, 128]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let patched = gpu.render_with_stats(&doc, || Ok(())).unwrap();
    assert_eq!(patched.pixels, render_flat(&doc).unwrap());
    assert_eq!(patched.stats.uploaded_bytes, 263 * 60 * 4);
    assert_eq!(patched.stats.upload_strips, 1);
    assert_eq!(patched.stats.patched_sources, 1);
    assert_eq!(patched.stats.source_hits, 1);
    assert_eq!(patched.stats.source_allocations, 0);
    doc.layers.get_mut(&id).unwrap().opacity = 93;
    doc.roots.reverse();
    let reordered = gpu.render_with_stats(&doc, || Ok(())).unwrap();
    assert_eq!(reordered.pixels, render_flat(&doc).unwrap());
    assert_eq!(reordered.stats.uploaded_bytes, 0);
    // A separate document must not discover/reuse the private source namespace.
    doc.document_id = deterministic_id(&["other-document"]);
    let foreign = gpu.render_with_stats(&doc, || Ok(())).unwrap();
    assert_eq!(foreign.pixels, render_flat(&doc).unwrap());
    assert_eq!(foreign.stats.source_allocations, 2);
}

#[test]
fn source_cache_limits_stream_oversized_layers_and_evict_without_pixel_changes() {
    use paintcraft_render::gpu::{DeviceEpoch, SourceCacheLimits};
    let Some(adapter) = adapter() else { return };
    let mut doc = new_document(64, 64, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = deterministic_id(&["second-resident-source"]);
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "Top".into(),
            origin: (0, 0),
            visible: true,
            opacity: 123,
            filters: Default::default(),
            raster: RasterLayer::new(64, 64, Rgba([17, 88, 156, 128])),
        },
    );
    let expected = render_flat(&doc).unwrap();
    for limits in [
        SourceCacheLimits {
            buffer_bytes: 0,
            metadata_bytes: 0,
            entries: 0,
        },
        SourceCacheLimits {
            buffer_bytes: 64 * 64 * 4,
            metadata_bytes: 1024,
            entries: 1,
        },
        SourceCacheLimits {
            buffer_bytes: 1024,
            metadata_bytes: 1024,
            entries: 2,
        },
        SourceCacheLimits {
            buffer_bytes: 64 * 64 * 8,
            metadata_bytes: 0,
            entries: 2,
        },
    ] {
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let epoch = DeviceEpoch::default();
        let lost = epoch.clone();
        device.set_device_lost_callback(move |_, _| lost.retire());
        let gpu = GpuRenderer::with_source_cache(device, queue, epoch, limits).unwrap();
        for iteration in 0..3 {
            let frame = gpu.render_with_stats(&doc, || Ok(())).unwrap();
            assert_eq!(frame.pixels, expected);
            let retained = limits.buffer_bytes >= 64 * 64 * 4 && limits.metadata_bytes >= 1024;
            let allocations = if retained && iteration > 0 { 1 } else { 2 };
            assert_eq!(frame.stats.source_allocations, allocations);
            assert_eq!(frame.stats.uploaded_bytes, 64 * 64 * 4 * allocations);
            assert!(frame.stats.retained.buffer_bytes <= limits.buffer_bytes);
            assert!(frame.stats.retained.metadata_bytes <= limits.metadata_bytes);
            assert!(frame.stats.retained.buffers <= limits.entries);
        }
        if limits.entries == 1 {
            // A new frame may evict the formerly used source when it becomes
            // invisible, rather than keeping that slot pinned indefinitely.
            let first = doc.roots[0];
            doc.layers.get_mut(&first).unwrap().visible = false;
            let replacement = gpu.render_with_stats(&doc, || Ok(())).unwrap();
            assert_eq!(replacement.pixels, render_flat(&doc).unwrap());
            assert_eq!(replacement.stats.source_allocations, 1);
            assert_eq!(replacement.stats.retained.buffers, 1);
            let hit = gpu.render_with_stats(&doc, || Ok(())).unwrap();
            assert_eq!(hit.stats.source_hits, 1);
            assert_eq!(hit.stats.uploaded_bytes, 0);
            doc.layers.get_mut(&first).unwrap().visible = true;
            let returned = gpu.render_with_stats(&doc, || Ok(())).unwrap();
            assert_eq!(returned.pixels, expected);
            assert_eq!(returned.stats.source_allocations, 2);
        }
    }
}

#[test]
fn cancellation_at_each_resident_update_boundary_retires_partial_views_and_recovers() {
    use paintcraft_core::types::{Error, ErrorCode};
    use std::cell::Cell;
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let base = new_document(64, 128, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let mut changed = base.clone();
    let id = base.roots[0];
    changed
        .layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            4,
            70,
            Rgba([17, 88, 156, 128]),
            paintcraft_raster::BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    gpu.render(&base).unwrap();
    let calls = Cell::new(0);
    gpu.render_with_stats(&changed, || {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(calls.get() > 64);
    let expected = render_flat(&changed).unwrap();
    for stop in 1..=calls.get() {
        gpu.render(&base).unwrap();
        let seen = Cell::new(0);
        let error = gpu
            .render_with_stats(&changed, || {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel resident transfer",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(seen.get(), stop);
        assert_eq!(gpu.source_cache_usage().buffers, 0);
        assert!(gpu.epoch().is_current());
        let restored = gpu.render_with_stats(&changed, || Ok(())).unwrap();
        assert_eq!(restored.pixels, expected);
        assert_eq!(restored.stats.source_allocations, 1);
        assert_eq!(base.layers[&id].raster.get(4, 70), Rgba([255; 4]));
    }
}

#[test]
fn resident_gpu_views_do_not_retain_decoded_source_tiles() {
    use paintcraft_core::types::resources::SharedBudget;
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let budget = SharedBudget::new(RasterLayer::TILE_BYTES as u64);
    let mut doc = new_document(64, 64, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            3,
            4,
            Rgba([17, 88, 156, 128]),
            paintcraft_raster::BlendMode::Replace,
            &mut Admission::with_residency(65536, budget.clone()),
        )
        .unwrap();
    parity(&gpu, &doc);
    assert_eq!(gpu.source_cache_usage().buffers, 1);
    drop(doc);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(gpu.source_cache_usage().buffers, 1);
}

#[test]
fn poisoned_gpu_source_state_is_discarded_and_admission_queries_do_not_lock() {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let doc = new_document(64, 128, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    gpu.render(&doc).unwrap();
    let calls = Cell::new(0);
    assert!(
        catch_unwind(AssertUnwindSafe(|| gpu.render_with_stats(&doc, || {
            assert_eq!(gpu.source_cache_admission_bytes(), 76 << 20);
            calls.set(calls.get() + 1);
            if calls.get() == 4 {
                panic!("injected pre-submission GPU cache panic")
            }
            Ok(())
        })))
        .is_err()
    );
    let rebuilt = gpu.render_with_stats(&doc, || Ok(())).unwrap();
    assert_eq!(rebuilt.pixels, vec![255; 64 * 128 * 4]);
    assert_eq!(rebuilt.stats.source_allocations, 1);
}

#[test]
fn empty_transparent_layers_skip_uploads_without_losing_revealed_source_changes() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut doc = new_document(64, 64, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let empty = gpu.render_with_stats(&doc, || Ok(())).unwrap();
    assert_eq!(empty.pixels, vec![0; 64 * 64 * 4]);
    assert_eq!(empty.stats.uploaded_bytes, 0);
    assert_eq!(empty.stats.retained.buffers, 0);
    doc.layers.get_mut(&id).unwrap().raster = RasterLayer::new(64, 64, Rgba([17, 88, 156, 128]));
    parity(&gpu, &doc);
    doc.layers.get_mut(&id).unwrap().raster = RasterLayer::new(64, 64, Rgba([31, 63, 127, 0]));
    assert_eq!(
        gpu.render_with_stats(&doc, || Ok(()))
            .unwrap()
            .stats
            .uploaded_bytes,
        0
    );
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            2,
            3,
            Rgba([1, 2, 3, 255]),
            paintcraft_raster::BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    parity(&gpu, &doc);
}

#[test]
fn display_texture_matches_integer_premultiplication_for_every_color_alpha_pair() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let data: Vec<u8> = (0u16..256)
        .flat_map(|alpha| {
            (0u16..256).flat_map(move |color| {
                [
                    color as u8,
                    (255 - color) as u8,
                    (color as u8) ^ 0x5a,
                    alpha as u8,
                ]
            })
        })
        .collect();
    let mut doc = new_document(256, 256, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().raster = RasterLayer::from_rgba(
        256,
        256,
        Rgba::TRANSPARENT,
        &data,
        &mut Admission::new(65536),
    )
    .unwrap();
    let frame = gpu.render_texture(&doc, || Ok(())).unwrap();
    assert_eq!(frame.size(), [256, 256]);
    assert_eq!(frame.epoch(), gpu.epoch());
    let expected: Vec<u8> = data
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            [
                ((u16::from(p[0]) * u16::from(p[3]) + 127) / 255) as u8,
                ((u16::from(p[1]) * u16::from(p[3]) + 127) / 255) as u8,
                ((u16::from(p[2]) * u16::from(p[3]) + 127) / 255) as u8,
                p[3],
            ]
        })
        .collect();
    assert_eq!(frame.readback(|| Ok(())).unwrap(), expected);
    // Texture readback has padded GPU rows; odd clipped extents must remove padding.
    doc.canvas = (263, 19);
    doc.layers.get_mut(&id).unwrap().origin = (-3, -1);
    let frame = gpu.render_texture(&doc, || Ok(())).unwrap();
    let expected: Vec<u8> = render_flat(&doc)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            [
                ((u16::from(p[0]) * u16::from(p[3]) + 127) / 255) as u8,
                ((u16::from(p[1]) * u16::from(p[3]) + 127) / 255) as u8,
                ((u16::from(p[2]) * u16::from(p[3]) + 127) / 255) as u8,
                p[3],
            ]
        })
        .collect();
    assert_eq!(frame.readback(|| Ok(())).unwrap(), expected);
}

#[test]
fn display_texture_cancellation_and_device_loss_preserve_host_pixels_and_retire_frames() {
    use paintcraft_core::types::{Error, ErrorCode};
    use std::cell::Cell;
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device.clone(), queue).unwrap();
    let doc = new_document(19, 17, Some(Rgba([31, 63, 127, 128])), 0, POLICY).unwrap();
    let calls = Cell::new(0);
    let frame = gpu
        .render_texture(&doc, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    let expected = frame.readback(|| Ok(())).unwrap();
    assert_eq!(expected, [16, 32, 64, 128].repeat(19 * 17));
    // Cover all observed cold composition/presentation checks; failed work cannot
    // poison the next presentation or turn a display frame into a document edit.
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        // A failed attempt clears source data; force a cold source for each count.
        let _ = gpu.render_texture(&doc, || {
            Err(Error::new(ErrorCode::OperationCancelled, "clear source"))
        });
        let error = gpu
            .render_texture(&doc, || {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel display"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(
            gpu.render_texture(&doc, || Ok(()))
                .unwrap()
                .readback(|| Ok(()))
                .unwrap(),
            expected
        );
    }
    let error = frame
        .readback(|| {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel display readback",
            ))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(frame.readback(|| Ok(())).unwrap(), expected);
    device.destroy();
    let _ = device.poll(wgpu::PollType::Poll);
    assert_eq!(
        frame.readback(|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        gpu.render_texture(&doc, || Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.default_pixel(),
        Rgba([31, 63, 127, 128])
    );
}

#[path = "gpu/regions.rs"]
mod regions;

#[path = "gpu/output_pool.rs"]
mod output_pool;

#[path = "gpu/scaled.rs"]
mod scaled;

#[path = "gpu/source_views.rs"]
mod source_views;

#[path = "gpu/batches.rs"]
mod batches;

#[path = "gpu/single.rs"]
mod single;

#[path = "gpu/filters.rs"]
mod filters;
#[path = "gpu/masks.rs"]
mod masks;

#[path = "gpu/point_profiles.rs"]
mod point_profiles;

#[path = "gpu/spatial.rs"]
mod spatial;

#[path = "gpu/levels.rs"]
mod levels;

#[path = "gpu/exposure.rs"]
mod exposure;

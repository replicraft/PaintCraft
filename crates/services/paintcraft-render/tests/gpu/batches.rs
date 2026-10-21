use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect};
use paintcraft_render::{
    display::{DisplayRequest, render_display_cancellable},
    gpu::{DeviceEpoch, SourceCacheLimits},
};

fn fixture(width: u32, height: u32, count: u32) -> Document {
    let mut doc = new_document(width, height, None, 0, POLICY).unwrap();
    let mut admission = Admission::new(128 << 20);
    for n in 0..count {
        let id = deterministic_id(&["batched-layer", &n.to_string()]);
        let pixels: Vec<_> = (0..(width + 7) * (height + 5))
            .flat_map(|p| {
                [
                    (p * 17 + n * 31) as u8,
                    (p / (width + 7) * 7 + n) as u8,
                    (p * 3 + n * 13) as u8,
                    [0, 1, 17, 64, 128, 254, 255][p as usize % 7],
                ]
            })
            .collect();
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: format!("Batch {n}"),
                origin: (-i64::from(n % 4), -i64::from(n % 3)),
                visible: true,
                opacity: 17 + (n * 71 % 239) as u8,
                raster: RasterLayer::from_rgba(
                    width + 7,
                    height + 5,
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
fn gpu(enabled: bool, entries: usize, output: u64) -> Option<GpuRenderer> {
    let adapter = adapter()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    Some(
        GpuRenderer::with_working_cache_limits(
            device,
            queue,
            DeviceEpoch::default(),
            SourceCacheLimits {
                entries,
                ..Default::default()
            },
            output,
        )
        .unwrap()
        .with_submission_batching(enabled)
        .with_region_source_cache(true),
    )
}
fn premultiply(pixels: &[u8]) -> Vec<u8> {
    pixels
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
fn ordered_batches_match_scalar_for_one_five_twenty_and_hundred_layers() {
    let Some(serial) = gpu(false, 64, 0) else {
        return;
    };
    let Some(batched) = gpu(true, 64, 0) else {
        return;
    };
    assert_eq!(serial.submission_admission_bytes(), 0);
    assert_eq!(batched.submission_admission_bytes(), 4 << 20);
    for count in [1u32, 5, 20, 100] {
        let mut doc = fixture(33, 19, count);
        for _ in 0..2 {
            let expected = render_flat(&doc).unwrap();
            let baseline = serial.render_with_stats(&doc, || Ok(())).unwrap();
            let candidate = batched.render_with_stats(&doc, || Ok(())).unwrap();
            assert_eq!(baseline.pixels, expected);
            assert_eq!(candidate.pixels, expected);
            assert_eq!(baseline.stats.composition_submissions, u64::from(count));
            assert_eq!(
                candidate.stats.composition_submissions,
                u64::from(count.div_ceil(16))
            );
            assert_eq!(candidate.stats.max_submission_layers, count.min(16));
            assert!(candidate.stats.max_submission_source_bytes <= 1 << 20);
            doc.roots.reverse();
        }
    }
}

#[test]
fn source_byte_bound_splits_batches_and_oversized_views_run_alone() {
    let Some(gpu) = gpu(true, 0, 0) else { return };
    for (width, height, submissions, max_layers) in [(512, 256, 3, 2), (1025, 257, 5, 1)] {
        let doc = fixture(width, height, 5);
        let frame = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert_eq!(frame.pixels, render_flat(&doc).unwrap());
        assert_eq!(frame.stats.composition_submissions, submissions);
        assert_eq!(frame.stats.max_submission_layers, max_layers);
        assert_eq!(frame.stats.retained.buffers, 0);
        if max_layers == 1 {
            assert!(frame.stats.max_submission_source_bytes > 1 << 20)
        } else {
            assert_eq!(frame.stats.max_submission_source_bytes, 1 << 20)
        }
    }
}

#[test]
fn batched_scaled_views_respect_cache_limits_and_keep_old_display_frames_immutable() {
    for entries in [0, 3, 64] {
        let Some(gpu) = gpu(true, entries, 4 << 20) else {
            return;
        };
        let doc = fixture(73, 51, 20);
        let request = DisplayRequest::new(&doc, Rect::new(5, 3, 61, 41).unwrap(), 2).unwrap();
        let expected = premultiply(
            &render_display_cancellable(&doc, request, || Ok(()))
                .unwrap()
                .pixels,
        );
        let old = gpu
            .render_display_texture(&doc, request, || Ok(()))
            .unwrap();
        assert_eq!(old.stats.composition_submissions, 2);
        assert_eq!(old.stats.max_submission_layers, 16);
        assert!(old.stats.retained.buffers <= entries);
        let newer = gpu
            .render_display_texture(&doc, request, || Ok(()))
            .unwrap();
        assert_eq!(newer.stats.source_hits, entries.min(20) as u64);
        assert_eq!(newer.stats.retained.buffers, entries.min(20));
        assert_eq!(newer.readback(|| Ok(())).unwrap(), expected);
        assert_eq!(old.readback(|| Ok(())).unwrap(), expected);
        assert_eq!(newer.stats.output_reuses, 1);
        parity(&gpu, &doc);
        assert_eq!(old.readback(|| Ok(())).unwrap(), expected);
    }
}

#[test]
fn cancelling_each_batched_checkpoint_never_publishes_partial_pixels_or_cache_keys() {
    use std::cell::Cell;
    let Some(gpu) = gpu(true, 64, 4 << 20) else {
        return;
    };
    let mut doc = fixture(8, 7, 20);
    let original = doc.clone();
    let old = gpu.render_texture(&doc, || Ok(())).unwrap();
    let old_pixels = old.readback(|| Ok(())).unwrap();
    let id = doc.roots[19];
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            5,
            4,
            Rgba([231, 19, 97, 137]),
            paintcraft_raster::BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    assert_ne!(render_flat(&doc).unwrap(), render_flat(&original).unwrap());
    let calls = Cell::new(0);
    gpu.render_with_stats(&doc, || {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        gpu.render_with_stats(&original, || Ok(())).unwrap();
        calls.set(0);
        let error = gpu
            .render_with_stats(&doc, || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "batch checkpoint",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(gpu.source_cache_usage().buffers, 0);
        assert_eq!(gpu.output_pool_usage(), 0);
        assert_eq!(old.readback(|| Ok(())).unwrap(), old_pixels);
    }
    assert_eq!(gpu.render(&doc).unwrap(), render_flat(&doc).unwrap());
    gpu.epoch().retire();
    assert_eq!(
        gpu.render(&doc).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(gpu.source_cache_usage().buffers, 0);
}

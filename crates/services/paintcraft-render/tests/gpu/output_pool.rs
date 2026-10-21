use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect};
use paintcraft_render::gpu::{DeviceEpoch, SourceCacheLimits};

fn renderer(output_bytes: u64) -> Option<GpuRenderer> {
    let adapter = adapter()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    Some(
        GpuRenderer::with_working_cache_limits(
            device,
            queue,
            DeviceEpoch::default(),
            SourceCacheLimits::default(),
            output_bytes,
        )
        .unwrap(),
    )
}

#[test]
fn completed_composite_reuse_keeps_live_display_textures_immutable_and_clears_empty_output() {
    let Some(gpu) = renderer(65536) else { return };
    let region = Rect::new(7, 9, 73, 51).unwrap();
    let a = new_document(90, 71, Some(Rgba([17, 37, 53, 255])), 0, POLICY).unwrap();
    let original = render_flat(&a).unwrap();
    let first = gpu.render_region_texture(&a, region, || Ok(())).unwrap();
    assert_eq!(first.stats.output_allocations, 1);
    assert_eq!(first.stats.output_reuses, 0);
    assert_eq!(first.stats.output_cleared_bytes, 0);
    assert_eq!(gpu.output_pool_usage(), region.area() as u64 * 4);
    let b = new_document(90, 71, Some(Rgba([91, 83, 67, 255])), 0, POLICY).unwrap();
    let second = gpu.render_region_texture(&b, region, || Ok(())).unwrap();
    assert_eq!(second.stats.output_allocations, 0);
    assert_eq!(second.stats.output_reuses, 1);
    assert_eq!(second.stats.output_cleared_bytes, region.area() as u64 * 4);
    assert_eq!(
        second.readback(|| Ok(())).unwrap(),
        [91, 83, 67, 255].repeat(73 * 51)
    );
    assert_eq!(
        first.readback(|| Ok(())).unwrap(),
        [17, 37, 53, 255].repeat(73 * 51)
    );
    let empty = new_document(90, 71, None, 0, POLICY).unwrap();
    for area in [Rect::new(1, 2, 5, 3).unwrap(), region] {
        let frame = gpu
            .render_region_with_stats(&empty, area, || Ok(()))
            .unwrap();
        assert_eq!(frame.stats.output_reuses, 1);
        assert_eq!(frame.pixels, vec![0; area.area() as usize * 4]);
        assert_eq!(frame.stats.retained_output_bytes, region.area() as u64 * 4);
    }
    assert_eq!(
        second.readback(|| Ok(())).unwrap(),
        [91, 83, 67, 255].repeat(73 * 51)
    );
    assert_eq!(render_flat(&a).unwrap(), original);
    gpu.clear_output_pool();
    assert_eq!(gpu.output_pool_usage(), 0);
    assert_eq!(
        first.readback(|| Ok(())).unwrap(),
        [17, 37, 53, 255].repeat(73 * 51)
    );
}

#[test]
fn output_retention_obeys_its_budget_and_streams_oversized_or_disabled_requests() {
    let Some(gpu) = renderer(8192) else { return };
    assert_eq!(gpu.output_pool_admission_bytes(), 8192);
    let doc = new_document(64, 64, Some(Rgba([97, 31, 13, 177])), 0, POLICY).unwrap();
    let small = Rect::new(0, 0, 16, 16).unwrap();
    assert_eq!(
        gpu.render_region_with_stats(&doc, small, || Ok(()))
            .unwrap()
            .stats
            .retained_output_bytes,
        1024
    );
    for _ in 0..2 {
        let frame = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert_eq!(frame.pixels, render_flat(&doc).unwrap());
        assert_eq!(frame.stats.output_allocations, 1);
        assert_eq!(frame.stats.output_reuses, 0);
        assert_eq!(gpu.output_pool_usage(), 0);
    }
    assert_eq!(
        gpu.render_region_with_stats(&doc, small, || Ok(()))
            .unwrap()
            .stats
            .output_allocations,
        1
    );
    assert_eq!(
        gpu.render_region_with_stats(&doc, small, || Ok(()))
            .unwrap()
            .stats
            .output_reuses,
        1
    );
    let Some(disabled) = renderer(0) else { return };
    for _ in 0..2 {
        let frame = disabled.render_with_stats(&doc, || Ok(())).unwrap();
        assert_eq!(frame.pixels, render_flat(&doc).unwrap());
        assert_eq!(frame.stats.output_allocations, 1);
        assert_eq!(frame.stats.output_reuses, 0);
        assert_eq!(disabled.output_pool_usage(), 0);
    }
}

#[test]
fn failed_or_cancelled_work_never_recycles_an_output_and_device_epochs_retire_the_pool() {
    use std::cell::Cell;
    let Some(gpu) = renderer(65536) else { return };
    let doc = new_document(41, 31, Some(Rgba([91, 83, 67, 127])), 0, POLICY).unwrap();
    drop(gpu.render_texture(&doc, || Ok(())).unwrap());
    let calls = Cell::new(0);
    drop(
        gpu.render_texture(&doc, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    let checkpoints = calls.get();
    for fail_at in 1..=checkpoints {
        drop(gpu.render_texture(&doc, || Ok(())).unwrap());
        calls.set(0);
        let result = gpu.render_texture(&doc, || {
            calls.set(calls.get() + 1);
            if calls.get() == fail_at {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel pooled output",
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(gpu.output_pool_usage(), 0);
        let retry = gpu.render(&doc).unwrap();
        assert_eq!(retry, render_flat(&doc).unwrap());
    }
    gpu.epoch().retire();
    assert!(gpu.render(&doc).is_err());
    assert_eq!(gpu.output_pool_usage(), 0);
}

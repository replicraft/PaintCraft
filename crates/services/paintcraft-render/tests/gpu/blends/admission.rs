use super::*;
use std::cell::Cell;
fn opaque_scene() -> Document {
    let mut d = new_document(64, 64, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let mut l = d.layers[&d.roots[0]].clone();
    l.id = deterministic_id(&["gpu-tight-blend"]);
    l.raster = RasterLayer::new(64, 64, Rgba([204, 77, 13, 128]));
    l.blend_mode = Mode::Multiply;
    l.opacity = 64;
    d.roots.push(l.id);
    d.layers.insert(l.id, l);
    d
}
fn workspace(b: &SharedBudget, cap: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 2 << 20, cap)
}
#[test]
fn clipped_one_pixel_dispatch_cold_dirty_and_public_reentry_obey_actual_parent() {
    use paintcraft_render::spatial::capture_workspace_or_current;
    let Some(gpu) = gpu() else { return };
    let mut d = opaque_scene();
    // Seed a resident tile, then restore the raw source. The next change has
    // stable tile topology and exercises a real dirty strip rather than insertion.
    let mut admission = Admission::new(1 << 20);
    top(&mut d)
        .raster
        .blend_pixel(
            0,
            0,
            Rgba([1; 4]),
            paintcraft_raster::BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    top(&mut d)
        .raster
        .blend_pixel(
            0,
            0,
            Rgba([204, 77, 13, 128]),
            paintcraft_raster::BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    let roi = Rect::new(0, 0, 1, 1).unwrap();
    let b = SharedBudget::new(4 << 20);
    let a = workspace(&b, 544);
    let once = Cell::new(false);
    let nested = Cell::new(0);
    let out = with_workspace(a.clone(), || {
        gpu.render_region_texture(&d, roi, || {
            if !once.replace(true) {
                assert_eq!(a.work_used(), 512);
                for (area, cap) in [(Rect::new(0, 0, 2, 1).unwrap(), 544), (roi, 543)] {
                    let before = a.work_used();
                    let cache = gpu.source_cache_usage();
                    let child = capture_workspace_or_current(
                        || panic!("Some never falls back"),
                        2 << 20,
                        cap,
                    )?;
                    assert!(child.same_job(&a));
                    let checks = Cell::new(0);
                    let e = with_workspace(child, || {
                        gpu.render_region_texture(&d, area, || {
                            checks.set(checks.get() + 1);
                            Ok(())
                        })
                    })
                    .unwrap_err();
                    assert_eq!(e.code, ErrorCode::ResourceLimit);
                    // Two inventory callbacks; no render body/output/source allocation.
                    assert_eq!(checks.get(), 2);
                    assert_eq!(gpu.source_cache_usage(), cache);
                    assert_eq!(a.work_used(), before);
                    nested.set(nested.get() + 1);
                }
            }
            Ok(())
        })
    })
    .unwrap();
    assert_eq!(out.readback(|| Ok(())).unwrap(), [249, 233, 225, 255]);
    assert_eq!(nested.get(), 2);
    assert_eq!(a.work_used(), 544);
    top(&mut d)
        .raster
        .blend_pixel(
            0,
            0,
            Rgba([255; 4]),
            paintcraft_raster::BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    let next = workspace(&b, 544);
    let request = DisplayRequest::new(&d, roi, 0).unwrap();
    let patch = with_workspace(next.clone(), || {
        gpu.render_display_texture(&d, request, || Ok(()))
    })
    .unwrap();
    assert_eq!(patch.readback(|| Ok(())).unwrap(), [255; 4]);
    assert_eq!(patch.stats.patched_sources, 1);
    assert_eq!(next.work_used(), 544);
}
#[test]
fn cancellation_and_repeated_some_calls_never_refund_or_reset_counter() {
    let Some(gpu) = gpu() else { return };
    let d = opaque_scene();
    let before = d.clone();
    let roi = Rect::new(0, 0, 1, 1).unwrap();
    let b = SharedBudget::new(4 << 20);
    let cancelled = workspace(&b, 544);
    let usage = gpu.source_cache_usage();
    let error = with_workspace(cancelled.clone(), || {
        gpu.render_region_texture(&d, roi, || {
            Err(paintcraft_core::types::Error::new(
                ErrorCode::OperationCancelled,
                "caller cancelled GPU blend",
            ))
        })
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(cancelled.work_used(), 512);
    assert_eq!(gpu.source_cache_usage(), usage);
    let e = with_workspace(cancelled.clone(), || {
        gpu.render_region_texture(&d, roi, || Ok(()))
    })
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::ResourceLimit);
    assert_eq!(cancelled.work_used(), 512);
    let a = workspace(&b, 1088);
    for _ in 0..2 {
        let frame =
            with_workspace(a.clone(), || gpu.render_region_texture(&d, roi, || Ok(()))).unwrap();
        assert_eq!(frame.readback(|| Ok(())).unwrap(), [249, 233, 225, 255]);
    }
    assert_eq!(a.work_used(), 1088);
    assert_eq!(
        with_workspace(a.clone(), || gpu.render_region_texture(&d, roi, || Ok(())))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), 1088);
    assert!(d.same_semantics(&before));
}
#[test]
fn noise_pair_quote_denial_is_atomic_and_actual_gpu_dispatch_uses_the_same_pair() {
    use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution};
    use paintcraft_render::noise_work::{
        NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace,
    };
    let Some(gpu) = gpu() else { return };
    let mut d = scene(
        2,
        Rgba([255; 4]),
        &[127, 128, 129, 255, 127, 128, 129, 255],
        Mode::Multiply,
    );
    top(&mut d).filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["gpu-blend-noise"]),
        true,
        Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(100, 65535, false, NoiseDistribution::Uniform).unwrap(),
        ),
    )])
    .unwrap();
    let before = d.clone();
    let b = SharedBudget::new(4 << 20);
    for (base, noise, success) in [(575, 2048, false), (576, 2047, false), (576, 2048, true)] {
        let job = NoiseAdmission::new_root(
            b.scope(AllocationClass::SharedBudget),
            NoiseJobPolicy::BrowserV1,
            NoiseLimits {
                bytes: 2 << 20,
                base_work: base,
                noise_work: noise,
            },
        )
        .unwrap();
        let result = with_noise_workspace(job.clone(), || gpu.render(&d));
        if success {
            // Frozen independent afc142 Uniform/seed65535/color corpus; white
            // opaque backdrop means Multiply reproduces these exact RGB values.
            assert_eq!(result.unwrap(), [255, 255, 0, 255, 114, 0, 98, 255]);
            assert_eq!(job.used().unwrap().base, 576);
            assert_eq!(job.used().unwrap().noise, 1024);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(job.used().unwrap().base, 512);
            assert_eq!(job.used().unwrap().noise, 0);
        }
    }
    assert!(d.same_semantics(&before));
}

#[test]
fn cpu_certificate_cannot_authorize_gpu_without_its_own_setup_and_preflight() {
    let Some(gpu) = gpu() else { return };
    let d = opaque_scene();
    let b = SharedBudget::new(4 << 20);
    let a = workspace(&b, 544);
    let once = Cell::new(false);
    let roi = Rect::new(0, 0, 1, 1).unwrap();
    let usage = gpu.source_cache_usage();
    let out = with_workspace(a.clone(), || {
        paintcraft_render::render_region_cancellable(&d, roi, || {
            if !once.replace(true) {
                assert_eq!(a.work_used(), 512);
                let checks = Cell::new(0);
                let e = gpu
                    .render_region_texture(&d, roi, || {
                        checks.set(checks.get() + 1);
                        Ok(())
                    })
                    .unwrap_err();
                assert_eq!(e.code, ErrorCode::ResourceLimit);
                assert_eq!(checks.get(), 0);
                assert_eq!(a.work_used(), 512);
                assert_eq!(gpu.source_cache_usage(), usage);
                // Public Normal-only program APIs do not acquire either certificate.
                assert_eq!(
                    paintcraft_render::prepared::quote_document_programs(&d, &|| Ok(()))
                        .unwrap_err()
                        .code,
                    ErrorCode::CapabilityUnsupported
                );
            }
            Ok(())
        })
    })
    .unwrap();
    assert_eq!(out, [249, 233, 225, 255]);
    assert_eq!(a.work_used(), 544);
    assert!(once.get());
}

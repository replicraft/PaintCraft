//! Literal alpha/order/cache controls. Authored, not executed at source freeze.
use super::*;
use crate::spatial::{WorkspaceAdmission, with_workspace};
use paintcraft_core::types::{
    filters::{BoxBlurV1, Descriptor},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack, LayerMask};
use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};
use std::cell::Cell;
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn doc(w: u32, h: u32, p: Rgba) -> Document {
    paintcraft_document::new_document(w, h, Some(p), 0, paintcraft_document::POLICY).unwrap()
}
fn mask(domain: Rect, m: u8) -> LayerMask {
    LayerMask::new(
        true,
        A8Mask::uniform(
            domain,
            m,
            &SharedBudget::new(1 << 20),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    )
}
fn run<T>(f: impl FnOnce(&WorkspaceAdmission) -> Result<T>) -> Result<T> {
    let b = SharedBudget::new(4 << 20);
    let a = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 4 << 20, u64::MAX);
    with_workspace(a.clone(), || f(&a))
}
#[test]
fn alpha_then_opacity_are_two_roundings_and_source_rgb_stays_intact() {
    let mut d = doc(2, 1, Rgba([20, 115, 200, 128]));
    let id = d.roots[0];
    let l = d.layers.get_mut(&id).unwrap();
    l.mask = Some(mask(rect(0, 0, 2, 1), 128));
    l.opacity = 128;
    let original = l.raster.clone();
    run(|a| {
        let mut row = [20, 115, 200, 128, 20, 115, 200, 128];
        crate::appearance::pixels(&d.layers[&id], (0, 0), &mut row, &|| Ok(()))?;
        assert_eq!(row, [20, 115, 200, 64, 20, 115, 200, 64]);
        assert_eq!(a.work_used(), work(&d.layers[&id], 2)?);
        assert_eq!(
            crate::render_flat(&d)?,
            [20, 115, 200, 32, 20, 115, 200, 32]
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(d.layers[&id].raster.get(0, 0), original.get(0, 0));
    assert_eq!(d.layers[&id].raster.resident_tiles(), 0);
}
#[test]
fn finite_domain_gaps_disabled_and_translation_keep_source_coordinates() {
    let mut d = doc(4, 1, Rgba([9, 8, 7, 128]));
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().mask = Some(mask(rect(1, 0, 2, 1), 128));
    run(|_| {
        let mut row = [9, 8, 7, 128].repeat(4);
        crate::appearance::pixels(&d.layers[&id], (0, 0), &mut row, &|| Ok(()))?;
        assert_eq!(row, [9, 8, 7, 0, 9, 8, 7, 64, 9, 8, 7, 64, 9, 8, 7, 0]);
        Ok(())
    })
    .unwrap();
    let l = d.layers.get_mut(&id).unwrap();
    l.origin = (1, 0);
    run(|_| {
        assert_eq!(
            crate::render_flat(&d)?,
            [0, 0, 0, 0, 0, 0, 0, 0, 9, 8, 7, 64, 9, 8, 7, 64]
        );
        Ok(())
    })
    .unwrap();
    let l = d.layers.get_mut(&id).unwrap();
    l.mask = Some(l.mask.as_ref().unwrap().with_enabled(false));
    assert_eq!(
        crate::render_flat(&d).unwrap(),
        [0, 0, 0, 0, 9, 8, 7, 128, 9, 8, 7, 128, 9, 8, 7, 128]
    );
}
#[test]
fn full_box_prefix_runs_before_layer_alpha_and_native_pixels_before_lod() {
    let mut d = paintcraft_document::import_image(
        2,
        1,
        &[255, 0, 0, 255, 0, 0, 255, 255],
        &"ab".repeat(32),
    )
    .unwrap();
    let id = d.roots[0];
    let l = d.layers.get_mut(&id).unwrap();
    l.filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["layer-mask-box"]),
        true,
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
    )])
    .unwrap();
    l.mask = Some(mask(rect(0, 0, 1, 1), 255));
    run(|a| {
        let plan =
            crate::spatial::LayerAppearancePlan::new(&d.layers[&id], d.canvas_rect(), &|| Ok(()))?;
        let mut output = [Rgba::TRANSPARENT; 2];
        plan.evaluate_into(a, &mut output, &|| Ok(()))?;
        assert_eq!(output, [Rgba([170, 0, 85, 255]), Rgba([85, 0, 170, 0])]);
        let req = crate::display::DisplayRequest::new(&d, d.canvas_rect(), 1)?;
        let frame = crate::display::render_display_cancellable(&d, req, || Ok(()))?;
        assert_eq!(frame.pixels, [170, 0, 85, 128]);
        Ok(())
    })
    .unwrap();
}
#[test]
fn visible_sampling_and_thumbnail_apply_mask_but_raw_layer_sampling_does_not() {
    let mut d = doc(1, 1, Rgba([20, 115, 200, 128]));
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().mask = Some(mask(d.canvas_rect(), 128));
    run(|_| {
        let size = paintcraft_core::types::picker::SampleSize::One;
        assert_eq!(
            crate::sampling::sample_color(&d, None, (0, 0), size, || Ok(()))?,
            Rgba([20, 115, 200, 64])
        );
        assert_eq!(
            crate::sampling::sample_color(&d, Some(&d.layers[&id]), (0, 0), size, || Ok(()))?,
            Rgba([20, 115, 200, 128])
        );
        assert_eq!(
            crate::thumbnail::render_layer(&d, &d.layers[&id], 16, || Ok(()))?.pixels,
            [20, 115, 200, 64]
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn unchanged_source_state_mask_only_edits_miss_cache_then_reuse_exact_key() {
    use crate::cache::{CacheLimits, CompositeCache, Domain};
    let mut d = doc(1, 1, Rgba([20, 115, 200, 128]));
    let id = d.roots[0];
    let doc_id = d.document_id;
    let state = d.state_id;
    let filters = d.layers[&id].filters.digest();
    let mut c = CompositeCache::new(CacheLimits {
        tile_edge: 16,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 8,
    })
    .unwrap();
    run(|_| {
        assert_eq!(
            c.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))?
                .pixels,
            [20, 115, 200, 128]
        );
        assert_eq!(
            c.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))?
                .stats
                .cache_hits,
            1
        );
        for (m, enabled, domain, alpha) in [
            (128, true, rect(0, 0, 1, 1), 64),
            (128, false, rect(0, 0, 1, 1), 128),
            (128, false, rect(1, 0, 1, 1), 128),
            (128, true, rect(1, 0, 1, 1), 0),
            (128, true, rect(0, 0, 1, 1), 64),
            (255, true, rect(0, 0, 1, 1), 128),
        ] {
            let layer = d.layers.get_mut(&id).unwrap();
            layer.mask = Some(mask(domain, m).with_enabled(enabled));
            let f = c.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))?;
            assert_eq!(f.stats.cache_hits, 0);
            assert_eq!(f.pixels[3], alpha);
            assert_eq!(
                c.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))?
                    .stats
                    .cache_hits,
                1
            );
        }
        assert_eq!(d.document_id, doc_id);
        assert_eq!(d.state_id, state);
        assert_eq!(d.layers[&id].filters.digest(), filters);
        assert_eq!(d.layers[&id].raster.resident_tiles(), 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn quote_binds_retained_backing_and_short_work_denies_before_callbacks_or_writes() {
    let mut d = doc(2, 1, Rgba([1, 2, 3, 128]));
    let id = d.roots[0];
    let runs1 = [Run {
        x0: 0,
        x1: 2,
        coverage: 128,
    }];
    let runs2 = [Run {
        x0: 0,
        x1: 2,
        coverage: 64,
    }];
    let full = LayerMask::new(
        true,
        A8Mask::from_bands(
            rect(0, 0, 2, 2),
            &[
                Band {
                    y0: 0,
                    y1: 1,
                    runs: &runs1,
                },
                Band {
                    y0: 1,
                    y1: 2,
                    runs: &runs2,
                },
            ],
            &SharedBudget::new(1 << 20),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    );
    let clipped = full
        .coverage()
        .cropped_checked(rect(0, 0, 2, 1), 1000, &|| Ok(()))
        .unwrap();
    assert_eq!(clipped.stats().bands, 1);
    assert_eq!(full.coverage().stats().bands, 2);
    assert_eq!(clipped.storage_usage(), full.coverage().storage_usage());
    d.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(true, clipped));
    let l = &d.layers[&id];
    let b = full.coverage().storage_usage().unwrap().1;
    assert_eq!(
        work(l, 2).unwrap(),
        2 * (48 + 2 * u64::from(64 - b.leading_zeros()))
    );
    let p = crate::spatial::LayerAppearancePlan::new(l, d.canvas_rect(), &|| Ok(())).unwrap();
    let a = WorkspaceAdmission::new(
        SharedBudget::new(1 << 20).scope(AllocationClass::SharedBudget),
        1 << 20,
        p.quote().work - 1,
    );
    let calls = Cell::new(0);
    let mut output = [Rgba([33; 4]); 2];
    let e = p
        .evaluate_into(&a, &mut output, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::ResourceLimit);
    assert_eq!(calls.get(), 0);
    assert_eq!(output, [Rgba([33; 4]); 2]);
    assert_eq!(
        crate::render_flat(&d).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn mask_work_spends_same_paired_base_counter_and_cancel_discards_private_prefix() {
    let mut d = doc(128, 1, Rgba([20, 115, 200, 128]));
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().mask = Some(mask(d.canvas_rect(), 128));
    let b = SharedBudget::new(1 << 20);
    let q = work(&d.layers[&id], 128).unwrap();
    let a = crate::noise_work::NoiseAdmission::new_root(
        b.scope(AllocationClass::SharedBudget),
        crate::noise_work::NoiseJobPolicy::BrowserV1,
        crate::noise_work::NoiseLimits {
            bytes: 1 << 18,
            base_work: q,
            noise_work: 512,
        },
    )
    .unwrap();
    let base = a.workspace().clone();
    crate::noise_work::with_noise_workspace(a, || {
        let mut row = [20, 115, 200, 128].repeat(128);
        crate::appearance::pixels(&d.layers[&id], (0, 0), &mut row, &|| Ok(()))?;
        assert_eq!(base.work_used(), q);
        assert_eq!(
            crate::appearance::pixels(&d.layers[&id], (0, 0), &mut row, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        Ok(())
    })
    .unwrap();
    run(|_| {
        let calls = Cell::new(0);
        let e = crate::render_cancellable(&d, || {
            calls.set(calls.get() + 1);
            if calls.get() == 8 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "layer-mask-cancel",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::OperationCancelled);
        Ok(())
    })
    .unwrap();
    assert_eq!(d.layers[&id].raster.get(0, 0), Rgba([20, 115, 200, 128]));
}

#[test]
fn exact_tight_view_does_not_spend_mask_work_outside_cpu_cache_or_display_roi() {
    use crate::cache::{CacheLimits, CompositeCache, Domain};
    let mut d = doc(128, 1, Rgba([20, 115, 200, 128]));
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().mask = Some(mask(d.canvas_rect(), 128));
    let roi = rect(37, 0, 1, 1);
    let q = work(&d.layers[&id], 1).unwrap();
    for display in [false, true] {
        let b = SharedBudget::new(1 << 20);
        let a = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, q);
        with_workspace(a.clone(), || {
            if display {
                let req = crate::display::DisplayRequest::new(&d, roi, 0)?;
                assert_eq!(
                    crate::display::render_display_cancellable(&d, req, || Ok(()))?.pixels,
                    [20, 115, 200, 64]
                );
            } else {
                let mut c = CompositeCache::new(CacheLimits {
                    tile_edge: 64,
                    committed_bytes: 1 << 20,
                    preview_bytes: 0,
                    entries_per_domain: 4,
                })?;
                assert_eq!(
                    c.render(&d, roi, Domain::Committed, || Ok(()))?.pixels,
                    [20, 115, 200, 64]
                );
            }
            assert_eq!(a.work_used(), q);
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn row_initial_mid_and_final_cancellation_never_change_the_source() {
    let mut d = doc(128, 1, Rgba([20, 115, 200, 128]));
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().mask = Some(mask(d.canvas_rect(), 128));
    for stop in [1, 2, 3] {
        run(|a| {
            let calls = Cell::new(0);
            let mut private = [20, 115, 200, 128].repeat(128);
            let e = crate::appearance::pixels(&d.layers[&id], (0, 0), &mut private, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "private mask row",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(e.code, ErrorCode::OperationCancelled);
            assert_eq!(
                a.work_used(),
                work(&d.layers[&id], ((stop - 1) * 64) as u64)?
            );
            assert_eq!(d.layers[&id].raster.get(0, 0), Rgba([20, 115, 200, 128]));
            // The private buffer is deliberately discarded on Err, never published.
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn stored_zero_and_full_masks_keep_the_priced_pass_without_a_fresh_realm() {
    let mut d = doc(1, 1, Rgba([20, 115, 200, 128]));
    let id = d.roots[0];
    for (coverage, expected) in [(0, vec![0, 0, 0, 0]), (255, vec![20, 115, 200, 128])] {
        d.layers.get_mut(&id).unwrap().mask = Some(mask(d.canvas_rect(), coverage));
        assert!(work(&d.layers[&id], 1).unwrap() >= 48);
        assert_eq!(
            crate::render_flat(&d).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        run(|_| {
            assert_eq!(crate::render_flat(&d)?, expected);
            Ok(())
        })
        .unwrap();
        assert_eq!(d.layers[&id].raster.resident_tiles(), 0);
    }
}

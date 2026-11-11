use super::*;
use paintcraft_core::types::ids::parse_uuid;
use paintcraft_core::types::{
    filters::Descriptor,
    json::{self, Value},
    resources::{AllocationClass, SharedBudget},
    styles::{ColorOverlayRgbV1, EffectBlendMode},
};
use paintcraft_document::{
    Document, FilterInstance, FilterMask, FilterStack, LayerBlendMode, LayerMask, StyleDescriptor,
    StyleInstance, StyleStack,
};
use paintcraft_raster::{
    RasterLayer,
    coverage_mask::{A8Mask, Limits},
};
fn byte(v: &Value) -> u8 {
    u8::try_from(v.as_i64().unwrap()).unwrap()
}
fn tuple(v: &Value) -> Rgba {
    let a = v.as_array().unwrap();
    Rgba(std::array::from_fn(|i| byte(&a[i])))
}
fn rect(w: i64, h: i64) -> Rect {
    Rect::new(0, 0, w, h).unwrap()
}
fn host(b: &SharedBudget, bytes: u64, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(
        b.reserve_capacity(bytes)
            .unwrap()
            .scope(AllocationClass::SharedBudget),
        bytes,
        work,
    )
}
fn policy(mode: u8) -> StyleDescriptor {
    StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
        [220, 40, 0],
        128,
        EffectBlendMode::from_code(mode).unwrap(),
    ))
}
fn styled(w: u32, h: u32) -> Document {
    let mut d = paintcraft_document::new_document(
        w,
        h,
        Some(Rgba([20, 80, 200, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let l = d.layers.get_mut(&d.roots[0]).unwrap();
    l.styles = StyleStack::new(vec![
        StyleInstance::new(
            parse_uuid("00000000-0000-0000-0000-00000000000e").unwrap(),
            true,
            policy(0),
        )
        .unwrap(),
    ])
    .unwrap();
    d
}
fn coverage(value: u8, b: &SharedBudget) -> A8Mask {
    A8Mask::uniform(rect(1, 1), value, b, Limits::default(), &|| Ok(())).unwrap()
}
#[test]
fn actual648_filter_style_ownmask_opacity_blend_pipeline_literals_original_counter() {
    let data = json::parse(include_bytes!(
        "../../tests/color-overlay-product-fixtures/pipeline.json"
    ))
    .unwrap();
    let rows = data.get("rows").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 648);
    let budget = SharedBudget::new(16 << 20);
    let original = host(&budget, 8 << 20, 268_435_456);
    for (index, row) in rows.iter().enumerate() {
        let r = row.as_array().unwrap();
        let mut d = styled(1, 1);
        let bottom = d.roots[0];
        d.layers.get_mut(&bottom).unwrap().raster =
            RasterLayer::new(1, 1, Rgba([31, 127, 241, 128]));
        d.layers.get_mut(&bottom).unwrap().styles = StyleStack::empty();
        let mut top = d.layers[&bottom].clone();
        top.id = parse_uuid("00000000-0000-0000-0000-00000000000d").unwrap();
        top.raster = RasterLayer::new(1, 1, tuple(&r[5]));
        top.opacity = byte(&r[3]);
        top.blend_mode = match byte(&r[4]) {
            0 => LayerBlendMode::Normal,
            1 => LayerBlendMode::Multiply,
            _ => LayerBlendMode::Screen,
        };
        top.filters = FilterStack::new(vec![
            FilterInstance::new(
                parse_uuid("00000000-0000-0000-0000-00000000002c").unwrap(),
                true,
                Descriptor::InvertRgbV1,
            )
            .with_mask(Some(FilterMask::new(true, coverage(127, &budget)))),
        ])
        .unwrap();
        top.styles = StyleStack::new(vec![
            StyleInstance::new(
                parse_uuid("00000000-0000-0000-0000-00000000000e").unwrap(),
                true,
                policy(byte(&r[0])),
            )
            .unwrap(),
        ])
        .unwrap();
        top.mask = Some(LayerMask::new(true, coverage(byte(&r[2]), &budget)));
        let raw = top.raster.clone();
        let id = top.id;
        d.roots.push(id);
        d.layers.insert(id, top);
        let out =
            crate::spatial::with_workspace(original.clone(), || crate::render_flat(&d)).unwrap();
        assert_eq!(out, tuple(&r[10]).0, "pipeline row {index}");
        assert_eq!(d.layers[&id].raster, raw);
        assert_eq!(original.scope().footprint().live, 0);
    }
    assert!(original.work_used() > 0);
    assert_eq!(budget.usage().reserved, 8 << 20);
    drop(original);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn two_ordered_styles_follow_complete_filter_prefix_not_lower_backdrop() {
    let data = json::parse(include_bytes!(
        "../../tests/color-overlay-product-fixtures/ordered.json"
    ))
    .unwrap();
    // Fixture root funds immutable source coverage separately from the unchanged
    // 1MiB original renderer reservation. This is not a fallback W grant.
    let budget = SharedBudget::new((1 << 20) + 4096);
    let original = host(&budget, 1 << 20, 268_435_456);
    for row in data.as_array().unwrap() {
        let mut d = styled(1, 1);
        let l = d.layers.get_mut(&d.roots[0]).unwrap();
        l.filters = FilterStack::new(vec![
            FilterInstance::new(
                parse_uuid("00000000-0000-0000-0000-00000000002c").unwrap(),
                true,
                Descriptor::InvertRgbV1,
            )
            .with_mask(Some(FilterMask::new(true, coverage(127, &budget)))),
        ])
        .unwrap();
        let first = policy(byte(row.get("first").unwrap()));
        let second = StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
            [17, 93, 201],
            127,
            EffectBlendMode::from_code(byte(row.get("second").unwrap())).unwrap(),
        ));
        l.styles = StyleStack::new(vec![
            StyleInstance::new(
                parse_uuid("00000000-0000-0000-0000-00000000000e").unwrap(),
                true,
                first,
            )
            .unwrap(),
            StyleInstance::new(
                parse_uuid("00000000-0000-0000-0000-00000000000f").unwrap(),
                true,
                second,
            )
            .unwrap(),
        ])
        .unwrap();
        let mut bytes = [20, 80, 200, 127];
        crate::spatial::with_workspace(original.clone(), || {
            crate::appearance::pixels(l, (0, 0), &mut bytes, &|| Ok(()))
        })
        .unwrap();
        assert_eq!(bytes, tuple(row.get("expected").unwrap()).0);
    }
}
#[test]
fn actual_row_work_and_control_exact_short_keep_same_debt_no_source_writes() {
    for (bytes, ceiling, ok, expected_debt) in [
        (CONTROL_BYTES, 17 + 12800, true, 17 + 12800),
        (CONTROL_BYTES - 1, 17 + 12800, false, 17 + 8192),
        (CONTROL_BYTES, 17 + 12800 - 1, false, 17 + 8192 + 4096),
    ] {
        let budget = SharedBudget::new(8192);
        let original = host(&budget, bytes, ceiling);
        original.reserve_work(17).unwrap();
        let d = styled(2, 1);
        let l = &d.layers[&d.roots[0]];
        assert_eq!(quote_work(l, rect(2, 1)).unwrap(), 12800);
        let source = [20, 80, 200, 127, 20, 80, 200, 127];
        let mut out = source;
        let result = crate::spatial::with_workspace(original.clone(), || {
            super::bytes(l, &mut out, Some(&original), &|| Ok(()))
        });
        assert_eq!(result.is_ok(), ok);
        assert_eq!(original.work_used(), expected_debt);
        assert_eq!(original.scope().footprint().live, 0);
        if ok {
            assert_eq!(out, [120, 60, 100, 127, 120, 60, 100, 127]);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(out, source);
            let prior = original.work_used();
            assert!(
                crate::spatial::with_workspace(original.clone(), || super::bytes(
                    l,
                    &mut out,
                    Some(&original),
                    &|| Ok(())
                ))
                .is_err()
            );
            // The control-short case can pay another stored traversal; it never
            // creates a replacement counter or reaches source writes.
            assert!(original.work_used() >= prior);
            assert_eq!(out, source);
        }
        assert_eq!(l.raster.default_pixel(), Rgba([20, 80, 200, 127]));
    }
}
#[test]
fn default_canvas_single_style_is_usable_under_original268m_no_cap_raise() {
    let budget = SharedBudget::new(8 << 20);
    let original = host(&budget, 8 << 20, 268_435_456);
    let d = styled(1000, 486);
    let raw = d.layers[&d.roots[0]].raster.clone();
    assert_eq!(
        quote_work(&d.layers[&d.roots[0]], d.canvas_rect()).unwrap(),
        219_967_488
    );
    let out = crate::spatial::with_workspace(original.clone(), || crate::render_flat(&d)).unwrap();
    assert_eq!(out.len(), 1000 * 486 * 4);
    for p in out.as_chunks::<4>().0 {
        assert_eq!(*p, [120, 60, 100, 127]);
    }
    assert!(original.work_used() > 0 && original.work_used() <= 268_435_456);
    assert_eq!(d.layers[&d.roots[0]].raster, raw);
    assert_eq!(original.scope().footprint().live, 0);
}

#[test]
fn spatial_uniform_blur_styles_before_own_mask_and_paired_zero_noise_keep_original_owners() {
    use paintcraft_core::types::filters::{AddNoiseRgbV1, BoxBlurV1, NoiseDistribution};
    for noise in [false, true] {
        let budget = SharedBudget::new(4 << 20);
        let mut d = styled(3, 1);
        let l = d.layers.get_mut(&d.roots[0]).unwrap();
        let mut filters = vec![];
        if noise {
            filters.push(FilterInstance::new(
                parse_uuid("00000000-0000-0000-0000-00000000002a").unwrap(),
                true,
                Descriptor::AddNoiseRgbV1(
                    AddNoiseRgbV1::new(0, 65535, false, NoiseDistribution::Uniform).unwrap(),
                ),
            ));
        }
        filters.push(FilterInstance::new(
            parse_uuid("00000000-0000-0000-0000-00000000002b").unwrap(),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        ));
        l.filters = FilterStack::new(filters).unwrap();
        l.mask = Some(LayerMask::new(
            true,
            A8Mask::uniform(rect(3, 1), 128, &budget, Limits::default(), &|| Ok(())).unwrap(),
        ));
        l.opacity = 128;
        let source = l.raster.clone();
        let out = if noise {
            use crate::noise_work::{NoiseAdmission, NoiseJobPolicy, NoiseLimits};
            let original = NoiseAdmission::new_root(
                budget
                    .reserve_capacity(1 << 20)
                    .unwrap()
                    .scope(AllocationClass::SharedBudget),
                NoiseJobPolicy::BrowserV1,
                NoiseLimits {
                    bytes: 1 << 20,
                    base_work: 268_435_456,
                    noise_work: 1 << 35,
                },
            )
            .unwrap();
            original.workspace().reserve_work(17).unwrap();
            let out = crate::noise_work::with_noise_workspace(original.clone(), || {
                crate::render_flat(&d)
            })
            .unwrap();
            let used = original.used().unwrap();
            assert!(used.base > 17);
            assert!(used.noise > 0);
            assert_eq!(original.workspace().work_used(), used.base);
            out
        } else {
            let original = host(&budget, 1 << 20, 268_435_456);
            crate::spatial::with_workspace(original, || crate::render_flat(&d)).unwrap()
        };
        // Old primitive literal120/60/100, alpha127→own128 rounds64→opacity128 rounds32.
        assert_eq!(out, [120, 60, 100, 32].repeat(3));
        assert_eq!(d.layers[&d.roots[0]].raster, source);
    }
}
#[test]
fn style_digest_invalidates_final_cache_and_cancellation_preserves_source() {
    use crate::cache::{CacheLimits, CompositeCache, Domain};
    let budget = SharedBudget::new(1 << 20);
    let original = host(&budget, 1 << 20, 268_435_456);
    let mut d = styled(2, 1);
    let raw = d.layers[&d.roots[0]].raster.clone();
    let mut cache = CompositeCache::new(CacheLimits {
        tile_edge: 64,
        committed_bytes: 1 << 16,
        preview_bytes: 1 << 16,
        entries_per_domain: 8,
    })
    .unwrap();
    let first = crate::spatial::with_workspace(original.clone(), || {
        cache.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(first.pixels, [120, 60, 100, 127].repeat(2));
    let l = d.layers.get_mut(&d.roots[0]).unwrap();
    l.styles = StyleStack::new(vec![
        StyleInstance::new(
            parse_uuid("00000000-0000-0000-0000-00000000000e").unwrap(),
            false,
            policy(0),
        )
        .unwrap(),
    ])
    .unwrap();
    let second = crate::spatial::with_workspace(original.clone(), || {
        cache.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(second.pixels, [20, 80, 200, 127].repeat(2));
    assert_eq!(second.stats.cache_hits, 0);
    let l = &d.layers[&d.roots[0]];
    let mut out = [20, 80, 200, 127, 20, 80, 200, 127];
    let error = crate::spatial::with_workspace(original.clone(), || {
        super::bytes(l, &mut out, Some(&original), &|| {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "actual style traversal cancellation",
            ))
        })
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(out, [20, 80, 200, 127, 20, 80, 200, 127]);
    assert_eq!(l.raster, raw);
    assert_eq!(original.scope().footprint().live, 0);
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_guard_refuses_every_stored_style_before_callback_or_output() {
    use std::cell::Cell;
    for (enabled, opacity) in [(true, 128), (false, 128), (true, 0)] {
        let budget = SharedBudget::new(1 << 20);
        let original = host(&budget, 1 << 20, 268_435_456);
        original.reserve_work(17).unwrap();
        let mut doc = styled(2, 1);
        doc.layers.get_mut(&doc.roots[0]).unwrap().styles = StyleStack::new(vec![
            StyleInstance::new(
                parse_uuid("00000000-0000-0000-0000-00000000000e").unwrap(),
                enabled,
                StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                    [220, 40, 0],
                    opacity,
                    EffectBlendMode::Normal,
                )),
            )
            .unwrap(),
        ])
        .unwrap();
        let expected = doc.require_no_styles().unwrap_err();
        let callbacks = Cell::new(0);
        let outputs = Cell::new(0);
        let error = crate::spatial::with_workspace(original.clone(), || {
            crate::blend::with_gpu_programs(
                &doc,
                doc.canvas_rect(),
                &|| {
                    callbacks.set(callbacks.get() + 1);
                    Ok(())
                },
                || {
                    outputs.set(outputs.get() + 1);
                    Ok(())
                },
            )
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(error.message, expected.message);
        assert_eq!(callbacks.get(), 0);
        assert_eq!(outputs.get(), 0);
        assert_eq!(original.work_used(), 17);
        assert_eq!(original.scope().footprint().live, 0);
    }
}

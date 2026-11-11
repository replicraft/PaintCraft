//! Literal style-only public paths: dense source offsets, LOD/partial cells and both caches.
//! Whole-ROI style quotes are not grants for a display's nested native calls.
use paintcraft_core::types::{
    Error, ErrorCode, LayerBlendMode, Rect, Rgba,
    ids::parse_uuid,
    json::{self, Value},
    resources::{AllocationClass, SharedBudget},
    styles::{ColorOverlayRgbV1, EffectBlendMode},
};
use paintcraft_document::{Document, POLICY, StyleDescriptor, StyleInstance, StyleStack};
use paintcraft_raster::{Admission, RasterLayer};
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    display::{DisplayCache, DisplayRequest},
    spatial::{WorkspaceAdmission, with_workspace},
};

fn oracle() -> Value {
    json::parse(include_bytes!(
        "color-overlay-product-fixtures/style-geometry.json"
    ))
    .unwrap()
}
fn field<'a>(v: &'a Value, name: &str) -> &'a Value {
    v.get(name).unwrap()
}
fn text<'a>(v: &'a Value, name: &str) -> &'a str {
    field(v, name).as_str().unwrap()
}
fn hex(s: &str) -> Vec<u8> {
    assert_eq!(s.len() % 2, 0);
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn rectangle(v: &Value) -> Rect {
    let a = v.as_array().unwrap();
    assert_eq!(a.len(), 4);
    Rect {
        x0: a[0].as_i64().unwrap(),
        y0: a[1].as_i64().unwrap(),
        x1: a[2].as_i64().unwrap(),
        y1: a[3].as_i64().unwrap(),
    }
}
fn limits(edge: u32) -> CacheLimits {
    CacheLimits {
        tile_edge: edge,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 4096,
    }
}
fn source(data: &Value, mode: LayerBlendMode) -> Document {
    let mut doc =
        paintcraft_document::new_document(514, 5, Some(Rgba([31, 127, 241, 128])), 0, POLICY)
            .unwrap();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = parse_uuid("00000000-0000-0000-0000-00000000000d").unwrap();
    layer.origin = (1, 1);
    layer.opacity = 173;
    layer.blend_mode = mode;
    layer.raster = RasterLayer::from_rgba(
        512,
        3,
        Rgba([7, 9, 11, 0]),
        &hex(text(data, "source_rgba_hex")),
        &mut Admission::new(2 * 65536),
    )
    .unwrap();
    assert!(layer.filters.is_empty() && layer.mask.is_none());
    assert_eq!(layer.raster.resident_tiles(), 2);
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
    doc
}
fn style(doc: &mut Document, state: &str) {
    let top = *doc.roots.last().unwrap();
    doc.layers.get_mut(&top).unwrap().styles = StyleStack::new(vec![
        StyleInstance::new(
            parse_uuid("00000000-0000-0000-0000-00000000000f").unwrap(),
            state != "disabled",
            StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                [220, 40, 0],
                if state == "opacity-zero" { 0 } else { 128 },
                EffectBlendMode::Normal,
            )),
        )
        .unwrap(),
    ])
    .unwrap();
}
fn original() -> (SharedBudget, WorkspaceAdmission) {
    let budget = SharedBudget::new(2 << 20);
    let admission = WorkspaceAdmission::new(
        budget
            .reserve_capacity(2 << 20)
            .unwrap()
            .scope(AllocationClass::SharedBudget),
        2 << 20,
        268_435_456,
    );
    admission.reserve_work(17).unwrap();
    (budget, admission)
}

#[test]
fn dense_style_only_three_blends_offset_partial_lod_and_both_caches_exact_literal_pixels() {
    let data = oracle();
    let region = rectangle(field(&data, "region"));
    for (name, mode) in [
        ("normal", LayerBlendMode::Normal),
        ("multiply", LayerBlendMode::Multiply),
        ("screen", LayerBlendMode::Screen),
    ] {
        let mut doc = source(&data, mode);
        let top = *doc.roots.last().unwrap();
        let raw = doc.layers[&top].raster.clone();
        let source_state = doc.state_id;
        let (budget, admission) = original();
        let mut native = CompositeCache::new(limits(64)).unwrap();
        let mut display_default = DisplayCache::new(limits(32)).unwrap();
        let mut display_one = DisplayCache::new(limits(1)).unwrap();
        for row in field(&data, "scenes")
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| text(row, "mode") == name)
        {
            style(&mut doc, text(row, "state"));
            let prior = admission.work_used();
            let expected = hex(text(row, "native_hex"));
            let partial = hex(text(row, "region_rgba_hex"));
            assert_eq!(
                with_workspace(admission.clone(), || paintcraft_render::render_flat(&doc)).unwrap(),
                expected,
                "{name}/{} full",
                text(row, "state")
            );
            assert_eq!(
                with_workspace(admission.clone(), || {
                    paintcraft_render::render_region_cancellable(&doc, region, || Ok(()))
                })
                .unwrap(),
                partial,
                "{name}/{} region",
                text(row, "state")
            );
            let first = with_workspace(admission.clone(), || {
                native.render(&doc, region, Domain::Committed, || Ok(()))
            })
            .unwrap();
            assert_eq!(first.pixels, partial);
            assert!(
                first.stats.recomposed_tiles > 0,
                "style intent must invalidate native cache even when pixels match"
            );
            let warm = with_workspace(admission.clone(), || {
                native.render(&doc, region, Domain::Committed, || Ok(()))
            })
            .unwrap();
            assert_eq!(warm.pixels, partial);
            assert!(warm.stats.cache_hits > 0);
            for sample in field(row, "display").as_array().unwrap() {
                let level = field(sample, "level").as_i64().unwrap() as u8;
                let request = DisplayRequest::new(&doc, region, level).unwrap();
                assert_eq!(request.region(), rectangle(field(sample, "region")));
                assert_eq!(request.footprint(), rectangle(field(sample, "footprint")));
                let wanted = hex(text(sample, "rgba_hex"));
                let plain = with_workspace(admission.clone(), || {
                    paintcraft_render::display::render_display_cancellable(&doc, request, || Ok(()))
                })
                .unwrap();
                assert_eq!(
                    plain.pixels,
                    wanted,
                    "{name}/{} level{level} uncached",
                    text(row, "state")
                );
                for cache in [&mut display_default, &mut display_one] {
                    let got = with_workspace(admission.clone(), || {
                        cache.render(&doc, request, Domain::Committed, || Ok(()))
                    })
                    .unwrap();
                    assert!(
                        got.stats.recomposed_tiles > 0,
                        "stored style intent invalidates display cache"
                    );
                    assert_eq!(
                        got.pixels,
                        wanted,
                        "{name}/{} level{level} cache",
                        text(row, "state")
                    );
                    let warm = with_workspace(admission.clone(), || {
                        cache.render(&doc, request, Domain::Committed, || Ok(()))
                    })
                    .unwrap();
                    assert_eq!(warm.pixels, wanted);
                    assert!(warm.stats.cache_hits > 0);
                }
            }
            // A stored disabled/zero-opacity style still needs the actual admission,
            // including on an otherwise warm output. No source-based budget is minted.
            let before_unadmitted = admission.work_used();
            assert_eq!(
                native
                    .render(&doc, region, Domain::Committed, || Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            let request = DisplayRequest::new(&doc, region, 1).unwrap();
            assert_eq!(
                display_one
                    .render(&doc, request, Domain::Committed, || Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(admission.work_used(), before_unadmitted);
            assert!(admission.work_used() > prior && admission.work_used() <= 268_435_456);
            assert_eq!(admission.scope().footprint().live, 0);
            assert_eq!(doc.layers[&top].raster, raw);
            assert_eq!(doc.state_id, source_state);
        }
        // Hidden style intent cannot turn a warm public request into an unadmitted path.
        doc.layers.get_mut(&top).unwrap().visible = false;
        assert_eq!(
            native
                .render(&doc, region, Domain::Committed, || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        let expected = [31, 127, 241, 128].repeat(451 * 4);
        assert_eq!(
            with_workspace(admission.clone(), || {
                paintcraft_render::render_region_cancellable(&doc, region, || Ok(()))
            })
            .unwrap(),
            expected
        );
        let error = with_workspace(admission.clone(), || {
            paintcraft_render::render_region_cancellable(&doc, region, || {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "geometry cancellation",
                ))
            })
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(doc.layers[&top].raster, raw);
        assert_eq!(budget.usage().reserved, 2 << 20);
        assert_eq!(admission.scope().footprint().live, 0);
        drop(admission);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn pure_whole_quote_and_per_leaf_bounds_are_distinct_without_grant_or_debt_changes() {
    let data = oracle();
    let mut doc = source(&data, LayerBlendMode::Multiply);
    style(&mut doc, "enabled");
    let top = *doc.roots.last().unwrap();
    let (budget, a) = original();
    let before = a.work_used();
    let footprint = a.scope().footprint();
    // Aligned public region intersects top in source-local[61,512)x[0,3).
    let output = Rect::new(61, 0, 451, 3).unwrap();
    let quoted = paintcraft_render::styles::quote_work(&doc.layers[&top], output).unwrap();
    assert_eq!(quoted, 24 * (8192 + 4096) + 1353 * 256);
    let cuts = field(field(&data, "cutpoints"), "source_local_intervals")
        .as_array()
        .unwrap();
    let mut row_forms = 0;
    let mut quote_groups = 0;
    let mut sum = 0;
    for interval in cuts {
        let r = interval.as_array().unwrap();
        let x0 = r[0].as_i64().unwrap();
        let x1 = r[1].as_i64().unwrap();
        let mut x = x0;
        while x < x1 {
            let end = x1.min((x / 256 + 1) * 256);
            row_forms += (end - x + 63) / 64;
            x = end;
        }
        quote_groups += (x0 % 64 + x1 - x0 + 63) / 64;
        sum += paintcraft_render::styles::quote_work(
            &doc.layers[&top],
            Rect::new(x0, 0, x1 - x0, 3).unwrap(),
        )
        .unwrap();
    }
    let raster = &doc.layers[&top].raster;
    let actual_forms: usize = cuts
        .iter()
        .map(|interval| {
            let r = interval.as_array().unwrap();
            let x0 = r[0].as_i64().unwrap() as u32;
            let x1 = r[1].as_i64().unwrap() as u32;
            raster
                .row_spans(0, x0, x1 - x0)
                .unwrap()
                .map(|span| span.pixel_count().div_ceil(64))
                .sum::<usize>()
        })
        .sum();
    assert_eq!(actual_forms, 10);
    let mut tiny_forms = 0usize;
    let mut tiny_quote_groups = 0u64;
    for x in (62u32..514).step_by(2) {
        let x0 = x.max(1) - 1;
        let x1 = (x + 2).min(513) - 1;
        tiny_forms += raster
            .row_spans(0, x0, x1 - x0)
            .unwrap()
            .map(|span| span.pixel_count().div_ceil(64))
            .sum::<usize>();
        tiny_quote_groups += u64::from((x0 % 64 + x1 - x0).div_ceil(64));
    }
    assert_eq!((tiny_forms, tiny_quote_groups), (227, 233));
    assert_eq!((row_forms, quote_groups), (10, 16));
    assert_eq!(sum, 48 * (8192 + 4096) + 1353 * 256);
    assert!(sum > quoted);
    assert_eq!(a.work_used(), before);
    assert_eq!(a.scope().footprint(), footprint);
    assert_eq!(budget.usage().reserved, 2 << 20);
    // Each public nested native call is independently prepaid on the same
    // counter; these pure values are not an entire DisplayCache admission.
}

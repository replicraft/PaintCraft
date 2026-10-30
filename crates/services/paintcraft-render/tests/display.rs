use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, ids::deterministic_id};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_raster::{Admission, BlendMode, RasterLayer};
use paintcraft_render::{
    cache::{CacheLimits, Domain},
    display::{DisplayCache, DisplayRequest},
    render_flat,
};

fn cache() -> DisplayCache {
    DisplayCache::new(CacheLimits {
        tile_edge: 32,
        committed_bytes: 4 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 128,
    })
    .unwrap()
}
fn fixture() -> Document {
    let mut doc = new_document(73, 51, None, 0, POLICY).unwrap();
    let mut admission = Admission::new(6 * 65536);
    for n in 0..3u32 {
        let bytes: Vec<_> = (0..79 * 57u32)
            .flat_map(|p| {
                [
                    p.wrapping_mul(17 + n) as u8,
                    (p / 79 * 7) as u8,
                    (p + n * 31) as u8,
                    [0, 1, 17, 128, 255][p as usize % 5],
                ]
            })
            .collect();
        let id = deterministic_id(&["display-reference", &n.to_string()]);
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: "offset".into(),
                origin: (-3 + n as i64 * 7, -2 + n as i64 * 3),
                opacity: 137 + n as u8 * 31,
                visible: true,
                filters: Default::default(),
                raster: RasterLayer::from_rgba(
                    79,
                    57,
                    Rgba([47, 71, 97, 0]),
                    &bytes,
                    &mut admission,
                )
                .unwrap(),
            },
        );
    }
    doc
}
fn reference(doc: &Document, request: DisplayRequest) -> Vec<u8> {
    let native = render_flat(doc).unwrap();
    let rect = request.region();
    let step = i64::from(request.step());
    let [w, h] = request.size();
    let mut output = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let cell = Rect::new(
                rect.x0 + i64::from(x) * step,
                rect.y0 + i64::from(y) * step,
                step,
                step,
            )
            .unwrap()
            .intersect(&rect)
            .unwrap();
            let mut sums = [0u128; 4];
            for sy in cell.y0..cell.y1 {
                for sx in cell.x0..cell.x1 {
                    let p = &native[(sy as usize * doc.canvas.0 as usize + sx as usize) * 4..][..4];
                    for c in 0..3 {
                        sums[c] += u128::from(p[c]) * u128::from(p[3]);
                    }
                    sums[3] += u128::from(p[3]);
                }
            }
            let count = cell.area();
            let alpha = (sums[3] + count / 2) / count;
            let pixel = if alpha == 0 {
                [0; 4]
            } else {
                [
                    ((sums[0] + sums[3] / 2) / sums[3]) as u8,
                    ((sums[1] + sums[3] / 2) / sums[3]) as u8,
                    ((sums[2] + sums[3] / 2) / sums[3]) as u8,
                    alpha as u8,
                ]
            };
            output.extend(pixel);
        }
    }
    output
}

#[test]
fn box_display_matches_independent_composite_then_filter_with_partial_canvas_edges() {
    let doc = fixture();
    let original = render_flat(&doc).unwrap();
    let mut cache = cache();
    for level in 0..=7 {
        for area in [
            doc.canvas_rect(),
            Rect::new(17, 13, 45, 33).unwrap(),
            Rect::new(72, 50, 1, 1).unwrap(),
        ] {
            let request = DisplayRequest::new(&doc, area, level).unwrap();
            let expected = reference(&doc, request);
            let first = cache
                .render(&doc, request, Domain::Committed, || Ok(()))
                .unwrap();
            assert_eq!(
                first.pixels.len(),
                request.size()[0] as usize * request.size()[1] as usize * 4
            );
            assert!(first.pixels == expected, "cold level={level} area={area:?}");
            let warm = cache
                .render(&doc, request, Domain::Committed, || Ok(()))
                .unwrap();
            assert!(warm.pixels == expected, "warm level={level} area={area:?}");
            assert!(warm.stats.cache_hits > 0);
        }
    }
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 3).unwrap();
    assert_eq!(request.size(), [10, 7]);
    assert_eq!(request.footprint(), Rect::new(0, 0, 80, 56).unwrap());
    assert_eq!(render_flat(&doc).unwrap(), original);
}

#[test]
fn layerwise_downsampling_is_not_used_and_levels_and_previews_have_separate_validity() {
    let mut doc = new_document(2, 1, None, 0, POLICY).unwrap();
    for (n, bytes) in [
        [255, 0, 0, 255, 255, 0, 0, 0],
        [0, 0, 255, 0, 0, 0, 255, 255],
    ]
    .into_iter()
    .enumerate()
    {
        let id = deterministic_id(&["display-order", &n.to_string()]);
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: "order".into(),
                origin: (0, 0),
                visible: true,
                opacity: 255,
                filters: Default::default(),
                raster: RasterLayer::from_rgba(
                    2,
                    1,
                    Rgba::TRANSPARENT,
                    &bytes,
                    &mut Admission::new(65536),
                )
                .unwrap(),
            },
        );
    }
    let mut cache = cache();
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1).unwrap();
    assert_eq!(
        cache
            .render(&doc, request, Domain::Committed, || Ok(()))
            .unwrap()
            .pixels,
        [128, 0, 128, 255]
    );
    let native = DisplayRequest::new(&doc, doc.canvas_rect(), 0).unwrap();
    assert_eq!(
        cache
            .render(&doc, native, Domain::Committed, || Ok(()))
            .unwrap()
            .pixels,
        render_flat(&doc).unwrap()
    );
    assert!(
        cache
            .render(&doc, request, Domain::Committed, || Ok(()))
            .unwrap()
            .stats
            .cache_hits
            > 0
    );
    let committed = cache.usage(Domain::Committed);
    let mut preview = doc.clone();
    let id = *preview.roots.last().unwrap();
    preview.layers.get_mut(&id).unwrap().opacity = 137;
    assert_eq!(
        cache
            .render(&preview, request, Domain::Preview, || Ok(()))
            .unwrap()
            .pixels,
        reference(&preview, request)
    );
    assert_eq!(cache.usage(Domain::Committed), committed);
    cache.clear_preview();
    assert_eq!(cache.usage(Domain::Preview).pixel_bytes, 0);
    assert_eq!(
        cache
            .render(&doc, request, Domain::Committed, || Ok(()))
            .unwrap()
            .pixels,
        [128, 0, 128, 255]
    );
}

#[test]
fn sparse_million_square_canvas_uses_bounded_native_work_and_output() {
    let mut doc = new_document(1_048_576, 1_048_576, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 10).unwrap();
    let mut cache = cache();
    let blank = cache
        .render(&doc, request, Domain::Committed, || Ok(()))
        .unwrap();
    assert_eq!(request.size(), [1024, 1024]);
    assert_eq!(blank.stats.native_pixels, 0);
    assert_eq!(blank.pixels, vec![255; 1024 * 1024 * 4]);
    let id = doc.roots[0];
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .fill_rect(
            &Rect::new(1024, 1024, 256, 256).unwrap(),
            Rgba([0, 0, 0, 255]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let drawn = cache
        .render(&doc, request, Domain::Committed, || Ok(()))
        .unwrap();
    assert!(drawn.stats.native_pixels <= 65536);
    assert_eq!(&drawn.pixels[(1024 + 1) * 4..][..4], &[239, 239, 239, 255]);
    assert_eq!(&drawn.pixels[..4], &[255; 4]);
    assert!(cache.usage(Domain::Committed).weight() <= 4 << 20);
}

#[test]
fn cancelled_reduction_never_returns_partial_pixels_and_retry_is_exact() {
    use std::cell::Cell;
    let doc = fixture();
    let request = DisplayRequest::new(&doc, Rect::new(0, 0, 8, 8).unwrap(), 2).unwrap();
    let calls = Cell::new(0);
    let mut baseline = cache();
    baseline
        .render(&doc, request, Domain::Committed, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    for fail in 1..=calls.get() {
        let mut cache = cache();
        let now = Cell::new(0);
        let result = cache.render(&doc, request, Domain::Committed, || {
            now.set(now.get() + 1);
            if now.get() == fail {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel display"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(
            cache
                .render(&doc, request, Domain::Committed, || Ok(()))
                .unwrap()
                .pixels,
            reference(&doc, request)
        );
    }
    assert_eq!(
        DisplayRequest::new(&doc, doc.canvas_rect(), 21)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}

#[test]
fn dense_reduction_across_unaligned_recursive_splits_matches_the_full_pixel_oracle() {
    let mut doc = new_document(519, 301, None, 0, POLICY).unwrap();
    let mut admission = Admission::new(12 * 65536);
    for n in 0..2u32 {
        let bytes: Vec<_> = (0..523 * 307u32)
            .flat_map(|p| {
                [
                    (p * 17 + n) as u8,
                    (p / 523 * 29) as u8,
                    (p * 11 + n * 71) as u8,
                    [0, 1, 97, 254, 255][p as usize % 5],
                ]
            })
            .collect();
        let id = deterministic_id(&["display-split", &n.to_string()]);
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: "recursive split".into(),
                origin: (-7 + i64::from(n) * 11, 5 - i64::from(n) * 9),
                visible: true,
                opacity: 173 + n as u8 * 31,
                filters: Default::default(),
                raster: RasterLayer::from_rgba(523, 307, Rgba::TRANSPARENT, &bytes, &mut admission)
                    .unwrap(),
            },
        );
    }
    let mut cache = cache();
    for level in [3, 5, 8] {
        let request =
            DisplayRequest::new(&doc, Rect::new(17, 13, 501, 287).unwrap(), level).unwrap();
        let expected = reference(&doc, request);
        let frame = cache
            .render(&doc, request, Domain::Committed, || Ok(()))
            .unwrap();
        assert!(frame.stats.native_pixels > 0);
        assert!(frame.pixels == expected, "cold recursive level {level}");
        assert!(
            cache
                .render(&doc, request, Domain::Committed, || Ok(()))
                .unwrap()
                .pixels
                == expected
        );
    }
}

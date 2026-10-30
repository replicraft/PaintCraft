//! Independent literal/channel and source-coordinate controls for point masks.
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    filters::{BrightnessContrastV1, Descriptor},
    ids::deterministic_id,
    picker::SampleSize,
    resources::SharedBudget,
};
use paintcraft_document::{
    Document, FilterInstance, FilterMask, FilterStack, Layer, POLICY, new_document,
};
use paintcraft_raster::{
    Admission, BlendMode, RasterLayer, Selection,
    coverage_mask::{A8Mask, Band, Limits, Run},
};
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    clipboard::capture_visible,
    display::{DisplayRequest, render_display_cancellable},
    render_flat, render_region, render_region_cancellable,
    sampling::sample_color,
    thumbnail,
};
use std::cell::Cell;

const COVERAGE: [u8; 6] = [0, 1, 127, 128, 254, 255];
const RAW: [u8; 3] = [10, 60, 200];
const INVERTED: [[u8; 3]; 6] = [
    [10, 60, 200],
    [11, 61, 199],
    [127, 127, 128],
    [128, 128, 127],
    [244, 194, 56],
    [245, 195, 55],
];

fn field(width: u32, height: u32, y0: i64, y1: i64) -> A8Mask {
    let runs: Vec<_> = (0..i64::from(width))
        .map(|x| Run {
            x0: x,
            x1: x + 1,
            coverage: COVERAGE[x as usize % COVERAGE.len()],
        })
        .collect();
    A8Mask::from_bands(
        Rect::new(0, 0, width.into(), height.into()).unwrap(),
        &[Band {
            y0,
            y1,
            runs: &runs,
        }],
        &SharedBudget::new(8 << 20),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn uniform(width: u32, height: u32, value: u8) -> A8Mask {
    A8Mask::uniform(
        Rect::new(0, 0, width.into(), height.into()).unwrap(),
        value,
        &SharedBudget::new(8 << 20),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn instance(name: &str, descriptor: Descriptor, coverage: A8Mask, enabled: bool) -> FilterInstance {
    FilterInstance::new(deterministic_id(&[name]), true, descriptor)
        .with_mask(Some(FilterMask::new(enabled, coverage)))
}
fn document(alpha: u8) -> Document {
    let mut doc =
        new_document(6, 2, Some(Rgba([RAW[0], RAW[1], RAW[2], alpha])), 8, POLICY).unwrap();
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![instance(
        "coverage-literals",
        Descriptor::InvertRgbV1,
        field(6, 2, 0, 2),
        true,
    )])
    .unwrap();
    doc
}
fn literal_row(alpha: u8) -> Vec<u8> {
    INVERTED
        .iter()
        .flat_map(|rgb| {
            if alpha == 0 {
                [0; 4]
            } else {
                [rgb[0], rgb[1], rgb[2], alpha]
            }
        })
        .collect()
}

#[test]
fn fractional_masks_affect_implicit_default_pixels_and_preserve_alpha_and_source() {
    for alpha in [0, 1, 127, 128, 254, 255] {
        let doc = document(alpha);
        let source = doc.layers[&doc.roots[0]].raster.clone();
        assert_eq!(source.resident_tiles(), 0);
        assert_eq!(render_flat(&doc).unwrap(), literal_row(alpha).repeat(2));
        for x in 0..6 {
            assert_eq!(
                render_region(&doc, Rect::new(x, 0, 1, 2).unwrap()).unwrap(),
                literal_row(alpha)[x as usize * 4..x as usize * 4 + 4].repeat(2)
            );
        }
        assert_eq!(doc.layers[&doc.roots[0]].raster, source);
        assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
    }
}

#[test]
fn zero_disabled_outside_and_absent_masks_are_distinct() {
    let mut doc = document(255);
    let id = doc.roots[0];
    let original = doc.layers[&id].filters.as_slice()[0].clone();
    let full = [245, 195, 55, 255].repeat(12);
    let raw = [10, 60, 200, 255].repeat(12);
    for (mask, expected) in [
        (None, full.clone()),
        (Some(FilterMask::new(false, uniform(6, 2, 0))), full),
        (Some(FilterMask::new(true, uniform(6, 2, 0))), raw.clone()),
        (
            Some(FilterMask::new(
                true,
                uniform(6, 2, 255)
                    .translated_checked(10, -4, 1000, &|| Ok(()))
                    .unwrap(),
            )),
            raw,
        ),
    ] {
        doc.layers.get_mut(&id).unwrap().filters =
            FilterStack::new(vec![original.clone().with_mask(mask)]).unwrap();
        assert_eq!(render_flat(&doc).unwrap(), expected);
    }
}

#[test]
fn each_ordered_filter_mixes_its_own_coverage_once_before_layer_opacity() {
    let mut doc = new_document(1, 1, Some(Rgba([10, 60, 200, 255])), 6, POLICY).unwrap();
    let invert = instance(
        "ordered-invert",
        Descriptor::InvertRgbV1,
        uniform(1, 1, 128),
        true,
    );
    let brightness = instance(
        "ordered-brightness",
        Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(20, 0).unwrap()),
        uniform(1, 1, 64),
        true,
    );
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().filters =
        FilterStack::new(vec![invert.clone(), brightness.clone()]).unwrap();
    assert_eq!(render_flat(&doc).unwrap(), [141, 141, 140, 255]);
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![brightness, invert]).unwrap();
    assert_eq!(render_flat(&doc).unwrap(), [128, 128, 127, 255]);
}

// A separate real interpolation reference, not the integer production numerator.
// Denominator255 cannot produce a half-integer; nearest-half distance>=1/510.
fn masked_channel(source: u8, candidate: u8, coverage: u8) -> u8 {
    (f64::from(source) + (f64::from(candidate) - f64::from(source)) * f64::from(coverage) / 255.0)
        .round() as u8
}
fn over_opaque(source: [u8; 4], opacity: u8, background: [u8; 3]) -> [u8; 4] {
    let alpha = (f64::from(source[3]) * f64::from(opacity) / 255.0).round();
    let mut out = [0, 0, 0, 255];
    for i in 0..3 {
        out[i] = ((f64::from(source[i]) * alpha + f64::from(background[i]) * (255.0 - alpha))
            / 255.0)
            .round() as u8;
    }
    out
}

#[test]
fn source_local_coordinates_cross_dense_tiles_negative_origins_and_backdrops() {
    let mut doc = new_document(259, 3, Some(Rgba([3, 90, 150, 255])), 1, POLICY).unwrap();
    let id = deterministic_id(&["offset-mask"]);
    let mut raster = RasterLayer::new(520, 4, Rgba([10, 60, 200, 200]));
    raster
        .fill_rect(
            &Rect::new(256, 1, 3, 2).unwrap(),
            Rgba([100, 10, 220, 128]),
            BlendMode::Replace,
            &mut Admission::new(1 << 20),
        )
        .unwrap();
    let source = raster.clone();
    let versions: Vec<_> = source
        .tile_leases()
        .map(|(p, tile)| (p, tile.version()))
        .collect();
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "offset".into(),
            origin: (-255, -1),
            visible: true,
            opacity: 128,
            raster,
            filters: FilterStack::new(vec![instance(
                "offset-invert",
                Descriptor::InvertRgbV1,
                field(520, 4, 1, 3),
                true,
            )])
            .unwrap(),
        },
    );
    let expected: Vec<_> = (0..3)
        .flat_map(|y| {
            let source = &source;
            (0..259).flat_map(move |x| {
                let (lx, ly) = (x + 255, y + 1);
                let raw = source.get(lx, ly).0;
                let coverage = if ly < 3 { COVERAGE[lx as usize % 6] } else { 0 };
                let incoming = [
                    masked_channel(raw[0], 255 - raw[0], coverage),
                    masked_channel(raw[1], 255 - raw[1], coverage),
                    masked_channel(raw[2], 255 - raw[2], coverage),
                    raw[3],
                ];
                over_opaque(incoming, 128, [3, 90, 150])
            })
        })
        .collect();
    assert_eq!(render_flat(&doc).unwrap(), expected);
    let area = Rect::new(254, 0, 5, 3).unwrap();
    let regional: Vec<_> = (0..3)
        .flat_map(|y| {
            expected[(y * 259 + 254) * 4..(y * 259 + 259) * 4]
                .iter()
                .copied()
        })
        .collect();
    assert_eq!(render_region(&doc, area).unwrap(), regional);
    assert_eq!(doc.layers[&id].raster, source);
    assert_eq!(
        doc.layers[&id]
            .raster
            .tile_leases()
            .map(|(p, tile)| (p, tile.version()))
            .collect::<Vec<_>>(),
        versions
    );
}

#[test]
fn mask_only_changes_invalidate_caches_and_all_visible_consumers_use_appearance() {
    let mut doc = document(255);
    let id = doc.roots[0];
    let state = doc.state_id;
    let original = doc.layers[&id].raster.clone();
    let mut cache = CompositeCache::new(CacheLimits {
        tile_edge: 32,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 32,
    })
    .unwrap();
    let expected = literal_row(255).repeat(2);
    assert_eq!(
        cache
            .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
            .unwrap()
            .pixels,
        expected
    );
    let hit = cache
        .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
        .unwrap();
    assert!(hit.stats.cache_hits > 0);
    assert_eq!(
        thumbnail::render_document(&doc, 16, || Ok(()))
            .unwrap()
            .pixels,
        expected
    );
    assert_eq!(
        sample_color(&doc, None, (3, 0), SampleSize::One, || Ok(())).unwrap(),
        Rgba([128, 128, 127, 255])
    );
    assert_eq!(
        sample_color(
            &doc,
            Some(&doc.layers[&id]),
            (3, 0),
            SampleSize::One,
            || Ok(())
        )
        .unwrap(),
        Rgba([10, 60, 200, 255])
    );
    let copy = capture_visible(
        &doc,
        &Selection::Inactive,
        &SharedBudget::new(1 << 20),
        &|| Ok(()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(copy.0.rgba(), expected);
    let filter = doc.layers[&id].filters.as_slice()[0]
        .clone()
        .with_mask(Some(FilterMask::new(true, uniform(6, 2, 255))));
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![filter]).unwrap();
    let changed = cache
        .render(&doc, doc.canvas_rect(), Domain::Committed, || Ok(()))
        .unwrap();
    assert_eq!(changed.pixels, [245, 195, 55, 255].repeat(12));
    assert!(changed.stats.recomposed_tiles > 0);
    assert_eq!(
        doc.state_id, state,
        "mask digest, not state changes, invalidates cached content"
    );
    assert_eq!(doc.layers[&id].raster, original);
}

#[test]
fn million_square_sparse_reduction_proves_mask_regions_without_materializing_pixels() {
    let side = 1_000_000;
    let mut doc = new_document(side, side, Some(Rgba([10, 60, 200, 255])), 7, POLICY).unwrap();
    let mask = A8Mask::from_bands(
        Rect::new(0, 0, side.into(), side.into()).unwrap(),
        &[Band {
            y0: 0,
            y1: side.into(),
            runs: &[Run {
                x0: 0,
                x1: i64::from(side / 2),
                coverage: 255,
            }],
        }],
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(vec![instance(
        "million-half",
        Descriptor::InvertRgbV1,
        mask,
        true,
    )])
    .unwrap();
    let checks = Cell::new(0);
    let frame = render_display_cancellable(
        &doc,
        DisplayRequest::new(&doc, doc.canvas_rect(), 20).unwrap(),
        || {
            checks.set(checks.get() + 1);
            if checks.get() > 2000 {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "unexpected sparse pixel traversal",
                ));
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(frame.pixels, [128, 128, 128, 255]);
    assert_eq!(frame.stats.native_pixels, 0);
    assert_eq!(frame.stats.uniform_pixels, 1_000_000_000_000);
    assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
}

#[test]
fn every_observed_masked_render_checkpoint_can_cancel_without_source_changes() {
    let doc = document(255);
    let original = doc.clone();
    let checks = Cell::new(0);
    render_region_cancellable(&doc, doc.canvas_rect(), || {
        checks.set(checks.get() + 1);
        Ok(())
    })
    .unwrap();
    let total = checks.get();
    assert!(total > 4);
    for stop in 1..=total {
        let observed = Cell::new(0);
        let error = render_region_cancellable(&doc, doc.canvas_rect(), || {
            observed.set(observed.get() + 1);
            if observed.get() == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "point-mask cancellation sentinel",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "point-mask cancellation sentinel");
        assert_eq!(observed.get(), stop);
        assert!(doc.same_semantics(&original));
    }
}

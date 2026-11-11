//! Local row-spans versus globally cut LOD callers. No new runtime authority.
use super::*;
use paintcraft_core::types::{
    ids::parse_uuid,
    resources::{AllocationClass, SharedBudget},
    styles::{ColorOverlayRgbV1, EffectBlendMode},
};
use paintcraft_document::{Document, StyleStack};
use paintcraft_raster::{Admission, RasterLayer};
fn scene(width: u32, enabled: bool) -> Document {
    let mut doc = paintcraft_document::new_document(
        width,
        1,
        Some(Rgba([0; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    let rgba: Vec<u8> = (0..width)
        .flat_map(|x| [x as u8, (x * 17) as u8, 255_u8.wrapping_sub(x as u8), 255])
        .collect();
    // Source fixture storage is immutable and outside the renderer's original W.
    layer.raster =
        RasterLayer::from_rgba(width, 1, Rgba([0; 4]), &rgba, &mut Admission::new(8 << 20))
            .unwrap();
    layer.styles = StyleStack::new(vec![
        StyleInstance::new(
            parse_uuid("00000000-0000-0000-0000-00000000000e").unwrap(),
            enabled,
            StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                [220, 40, 0],
                128,
                EffectBlendMode::Normal,
            )),
        )
        .unwrap(),
    ])
    .unwrap();
    doc
}
fn workspace(limit: u64) -> WorkspaceAdmission {
    let root = SharedBudget::new(CONTROL_BYTES);
    WorkspaceAdmission::new(
        root.reserve_capacity(CONTROL_BYTES)
            .unwrap()
            .scope(AllocationClass::SharedBudget),
        CONTROL_BYTES,
        limit,
    )
}
#[test]
fn single_compose_source256_restarts_fit_existing_source64_quote_exhaustively() {
    let doc = scene(1280, true);
    let layer = &doc.layers[&doc.roots[0]];
    for x in 0..256_u32 {
        for width in 1..=1025_u32 {
            let actual: usize = layer
                .raster
                .row_spans(0, x, width)
                .unwrap()
                .map(|span| span.pixel_count().div_ceil(64))
                .sum();
            let quoted = (quote_work(
                layer,
                Rect::new(i64::from(x), 0, i64::from(width), 1).unwrap(),
            )
            .unwrap()
                - 256 * u64::from(width))
                / 12288;
            assert!(actual as u64 <= quoted, "source x={x}, width={width}");
        }
    }
}
#[test]
fn offset_global64_calls_requote_each_leaf_under_same_original_debt() {
    let mut doc = scene(512, true);
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.origin = (1, 0);
    let source = layer.raster.clone();
    let whole = quote_work(layer, Rect::new(0, 0, 512, 1).unwrap()).unwrap();
    assert_eq!(whole, 8 * 12288 + 512 * 256);
    let original = workspace(17 + 16 * 12288 + 512 * 256);
    original.reserve_work(17).unwrap();
    let mut global = 1_u32;
    let mut actual = 0;
    let mut quoted = 0;
    while global < 513 {
        let end = ((global / 64 + 1) * 64).min(513);
        let local = global - 1;
        let width = end - global;
        let per_leaf = quote_work(
            layer,
            Rect::new(i64::from(local), 0, i64::from(width), 1).unwrap(),
        )
        .unwrap();
        quoted += (per_leaf - 256 * u64::from(width)) / 12288;
        for span in layer.raster.row_spans(0, local, width).unwrap() {
            let paintcraft_raster::RowSpan::Pixels(input) = span else {
                panic!("nonuniform fixture must exercise stored source spans")
            };
            for row in input.chunks(64 * 4) {
                actual += 1;
                let mut private = row.to_vec();
                crate::spatial::with_workspace(original.clone(), || {
                    super::bytes(layer, &mut private, Some(&original), &|| Ok(()))
                })
                .unwrap();
            }
        }
        global = end;
    }
    assert_eq!((actual, quoted), (10, 16));
    assert_eq!(original.work_used(), 17 + 10 * 12288 + 512 * 256);
    assert_eq!(original.scope().footprint().live, 0);
    assert_eq!(layer.raster, source);
    // Whole request's8 is not a complete decomposed-display grant. Real
    // Reduction leaves run cumulative preflight again, rather than renewing W.
    assert!(original.work_used() > 17 + whole);
}
#[test]
fn rooted_direct_fact_retains_original_base_even_for_disabled_styles() {
    for enabled in [true, false] {
        let doc = scene(1, enabled);
        let original = workspace(268_435_456);
        original.reserve_work(17).unwrap();
        let facts = crate::blend::rooted::RootFacts::capture(&doc, &|| Ok(()), &|n| {
            original.reserve_work(n)
        })
        .unwrap();
        assert!(facts.has_direct());
        assert!(!facts.has_coordinate());
        assert!(!facts.has_mask());
        assert_eq!(original.work_used(), 17 + 64 + 2048);
        assert_eq!(original.scope().footprint().live, 0);
    }
}

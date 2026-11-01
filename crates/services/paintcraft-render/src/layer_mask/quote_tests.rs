//! Source-only pure subtotal controls; no execution admission is installed.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_document::LayerMask;
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn layer() -> Layer {
    let doc = paintcraft_document::new_document(
        128,
        64,
        Some(Rgba([3, 4, 5, 128])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers[&doc.roots[0]].clone()
}
fn field(value: u8) -> LayerMask {
    LayerMask::new(
        true,
        A8Mask::uniform(
            rect(0, 0, 128, 64),
            value,
            &SharedBudget::new(1 << 20),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    )
}
#[test]
fn absent_disabled_and_enabled_empty_preserve_their_distinct_prices() {
    let mut l = layer();
    let tile = rect(0, 0, 64, 64);
    assert_eq!(quote_work(&l, tile).unwrap(), 0);
    l.mask = Some(field(0));
    assert!(
        l.mask
            .as_ref()
            .unwrap()
            .coverage()
            .storage_usage()
            .is_none()
    );
    assert_eq!(quote_work(&l, tile).unwrap(), 4096 * 48);
    l.mask = Some(l.mask.as_ref().unwrap().with_enabled(false));
    assert_eq!(quote_work(&l, tile).unwrap(), 0);
    assert!(crate::spatial::current().is_none());
}
#[test]
fn tiled_subtotal_is_output_only_and_uses_retained_backing_for_views() {
    let mut l = layer();
    l.mask = Some(field(255));
    let bytes = l
        .mask
        .as_ref()
        .unwrap()
        .coverage()
        .storage_usage()
        .unwrap()
        .1;
    let per_sample = 48 + 2 * u64::from(u64::BITS - bytes.leading_zeros());
    let whole = quote_work(&l, rect(0, 0, 128, 64)).unwrap();
    assert_eq!(whole, 8192 * per_sample);
    assert_eq!(
        whole,
        quote_work(&l, rect(0, 0, 64, 64)).unwrap() + quote_work(&l, rect(64, 0, 64, 64)).unwrap()
    );
    let clipped = l
        .mask
        .as_ref()
        .unwrap()
        .coverage()
        .clipped_checked(rect(0, 0, 1, 1), u64::MAX, &|| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(clipped.storage_usage().unwrap().1, bytes);
    l.mask = Some(LayerMask::new(true, clipped));
    // Domain/gaps never remove final-pass query units; no halo is added.
    assert_eq!(quote_work(&l, rect(0, 0, 128, 64)).unwrap(), whole);
    l.origin = (-100, 99);
    assert_eq!(quote_work(&l, rect(0, 0, 128, 64)).unwrap(), whole);
}
#[test]
fn invalid_public_rects_refuse_even_without_mask_or_workspace() {
    let l = layer();
    for output in [
        Rect {
            x0: i64::MIN,
            y0: 0,
            x1: i64::MAX,
            y1: 1,
        },
        Rect {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 1,
        },
        rect(128, 0, 1, 1),
        rect(0, 64, 1, 1),
    ] {
        assert_eq!(
            quote_work(&l, output).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
}

use super::*;
use paintcraft_core::types::{policy::POLICY, text::MaskSpan};
use paintcraft_raster::selection::{BinaryMask, MaskLimits};
use std::cell::Cell;

fn fixture() -> Document {
    let mut document =
        paintcraft_document::new_document(5, 4, Some(Rgba([20, 60, 100, 128])), 0, POLICY).unwrap();
    let other = paintcraft_document::new_document(3, 2, Some(Rgba([200, 100, 50, 128])), 0, POLICY)
        .unwrap();
    let mut top = other.layers[&other.roots[0]].clone();
    top.origin = (-1, 1);
    top.opacity = 127;
    document.roots.push(top.id);
    document.layers.insert(top.id, top);
    document
}

// Independent integer straight-alpha source-over oracle, including rounding.
fn over(source: [u8; 4], dest: [u8; 4]) -> [u8; 4] {
    let sa = u64::from(source[3]);
    let da = u64::from(dest[3]);
    let alpha = sa * 255 + da * (255 - sa);
    if alpha == 0 {
        return [0; 4];
    }
    let mut out = [0; 4];
    for c in 0..3 {
        out[c] =
            ((u64::from(source[c]) * sa * 255 + u64::from(dest[c]) * da * (255 - sa) + alpha / 2)
                / alpha) as u8;
    }
    out[3] = ((alpha + 127) / 255) as u8;
    out
}

#[test]
fn visible_capture_uses_order_opacity_finite_extents_and_visibility() {
    let budget = SharedBudget::new(4096);
    let mut document = fixture();
    for hidden in [false, true] {
        let top = document.roots[1];
        document.layers.get_mut(&top).unwrap().visible = !hidden;
        let before = document.clone();
        let (image, origin) = capture_visible(&document, &Selection::Inactive, &budget, &|| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(origin, (0, 0));
        assert_eq!((image.width(), image.height()), (5, 4));
        for y in 0..4 {
            for x in 0..5 {
                let expected = if !hidden && x < 2 && (1..3).contains(&y) {
                    over([200, 100, 50, 64], [20, 60, 100, 128])
                } else {
                    [20, 60, 100, 128]
                };
                let offset = (y * 5 + x) * 4;
                assert_eq!(&image.rgba()[offset..offset + 4], &expected);
            }
        }
        assert!(document.same_semantics(&before));
    }
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn tight_masks_inversion_and_rectangles_preserve_only_selected_composite() {
    let document = fixture();
    let budget = SharedBudget::new(1 << 20);
    let mask = BinaryMask::from_spans(
        document.canvas_rect(),
        false,
        &[
            MaskSpan {
                y: 1,
                x0: 0,
                x1: 2,
                coverage: 255,
            },
            MaskSpan {
                y: 2,
                x0: 0,
                x1: 0,
                coverage: 255,
            },
        ],
        &budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    for inverted in [false, true] {
        let selection = Selection::Mask(if inverted {
            mask.inverted()
        } else {
            mask.clone()
        });
        let (image, origin) = capture_visible(&document, &selection, &budget, &|| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(origin, if inverted { (0, 0) } else { (0, 1) });
        assert_eq!(
            (image.width(), image.height()),
            if inverted { (5, 4) } else { (3, 2) }
        );
        for y in 0..image.height() {
            for x in 0..image.width() {
                let (dx, dy) = (origin.0 + x as i64, origin.1 + y as i64);
                let selected = (dy == 1 && dx <= 2) || (dy == 2 && dx == 0);
                let expected = if selected == inverted {
                    [0; 4]
                } else if dx < 2 && (1..3).contains(&dy) {
                    over([200, 100, 50, 64], [20, 60, 100, 128])
                } else {
                    [20, 60, 100, 128]
                };
                let offset = ((y * image.width() + x) * 4) as usize;
                assert_eq!(&image.rgba()[offset..offset + 4], &expected);
            }
        }
    }
    let (image, origin) = capture_visible(
        &document,
        &Selection::Rect(Rect::new(-1, 2, 4, 4).unwrap()),
        &budget,
        &|| Ok(()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(origin, (0, 2));
    assert_eq!((image.width(), image.height()), (3, 2));
    assert!(
        capture_visible(&document, &Selection::Empty, &budget, &|| Ok(()))
            .unwrap()
            .is_none()
    );
}

#[test]
fn denial_and_every_cancellation_leave_no_output_or_pixel_charge() {
    let document = fixture();
    assert_eq!(
        capture_visible(
            &document,
            &Selection::Inactive,
            &SharedBudget::new(79),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let budget = SharedBudget::new(4096);
    let mask_budget = SharedBudget::new(1 << 20);
    let mask = BinaryMask::from_spans(
        document.canvas_rect(),
        false,
        &[
            MaskSpan {
                y: 1,
                x0: 0,
                x1: 2,
                coverage: 255,
            },
            MaskSpan {
                y: 2,
                x0: 0,
                x1: 0,
                coverage: 255,
            },
        ],
        &mask_budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    for selection in [Selection::Inactive, Selection::Mask(mask)] {
        let calls = Cell::new(0);
        let count = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        drop(capture_visible(&document, &selection, &budget, &count).unwrap());
        for stop in 1..=calls.get() {
            let current = Cell::new(0);
            let check = || {
                current.set(current.get() + 1);
                if current.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            };
            assert_eq!(
                capture_visible(&document, &selection, &budget, &check)
                    .unwrap_err()
                    .code,
                ErrorCode::OperationCancelled
            );
            assert_eq!(budget.usage().reserved, 0);
        }
    }
}

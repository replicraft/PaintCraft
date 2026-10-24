use super::*;
use paintcraft_core::types::Rect;
use paintcraft_core::types::policy::POLICY;
use paintcraft_core::types::text::MaskSpan;
use paintcraft_raster::selection::{BinaryMask, MaskLimits};
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::Cell;

fn fixture() -> (Document, Uuid) {
    let mut document =
        paintcraft_document::new_document(7, 6, Some(Rgba([7, 8, 9, 255])), 0, POLICY).unwrap();
    let id = document.roots[0];
    let layer = document.layers.get_mut(&id).unwrap();
    layer.origin = (-1, 2);
    let pixels: Vec<_> = (0..12)
        .flat_map(|n| [n + 1, 80, 90, if n & 1 == 0 { 0 } else { 1 }])
        .collect();
    layer.raster = RasterLayer::from_rgba(
        4,
        3,
        Rgba([99, 99, 99, 255]),
        &pixels,
        &mut Admission::new(1 << 20),
    )
    .unwrap();
    (document, id)
}

#[test]
fn active_layer_capture_ignores_presentation_and_finite_default_outside_extent() {
    let (mut document, id) = fixture();
    document.layers.get_mut(&id).unwrap().visible = false;
    document.layers.get_mut(&id).unwrap().opacity = 0;
    let budget = SharedBudget::new(4096);
    let captured = capture(&document, Selection::Inactive, id, &budget, &|| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(captured.origin, (0, 0));
    assert_eq!((captured.image.width(), captured.image.height()), (7, 6));
    assert_eq!(captured.selection, Selection::Inactive);
    for y in 0..6 {
        for x in 0..7 {
            let expected = if x < 3 && (2..5).contains(&y) {
                let n = (y - 2) * 4 + x + 1;
                [n + 1, 80, 90, if n & 1 == 0 { 0 } else { 1 }]
            } else {
                [0; 4]
            };
            let index = ((y * 7 + x) * 4) as usize;
            assert_eq!(&captured.image.rgba()[index..index + 4], &expected);
        }
    }
}

#[test]
fn exact_sparse_mask_bounds_holes_and_inversion_have_independent_pixel_oracle() {
    let (document, id) = fixture();
    let budget = SharedBudget::new(1 << 20);
    let domain = document.canvas_rect();
    let spans = [
        MaskSpan {
            y: 2,
            x0: 0,
            x1: 2,
            coverage: 255,
        },
        MaskSpan {
            y: 3,
            x0: 0,
            x1: 0,
            coverage: 255,
        },
        MaskSpan {
            y: 4,
            x0: 2,
            x1: 2,
            coverage: 255,
        },
    ];
    let mask = BinaryMask::from_spans(
        domain,
        false,
        &spans,
        &budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    for inverted in [false, true] {
        let mask = if inverted {
            mask.inverted()
        } else {
            mask.clone()
        };
        let selection = Selection::Mask(mask);
        let result = capture(&document, selection.clone(), id, &budget, &|| Ok(()))
            .unwrap()
            .unwrap();
        let bounds = if inverted {
            domain
        } else {
            Rect::new(0, 2, 3, 3).unwrap()
        };
        assert_eq!(result.origin, (bounds.x0, bounds.y0));
        assert_eq!(result.selection, selection);
        let mut expected = Vec::new();
        for y in bounds.y0..bounds.y1 {
            for x in bounds.x0..bounds.x1 {
                let selected = (y == 2 && x <= 2) || (y == 3 && x == 0) || (y == 4 && x == 2);
                let pixel = if selected != inverted && x < 3 && (2..5).contains(&y) {
                    let n = ((y - 2) * 4 + x + 1) as u8;
                    [n + 1, 80, 90, if n & 1 == 0 { 0 } else { 1 }]
                } else {
                    [0; 4]
                };
                expected.extend_from_slice(&pixel);
            }
        }
        assert_eq!(result.image.rgba(), expected);
    }
}

#[test]
fn empty_clipped_rect_invalid_target_denial_and_cancel_publish_nothing() {
    let (document, id) = fixture();
    let budget = SharedBudget::new(4096);
    for selection in [
        Selection::Empty,
        Selection::Rect(Rect::new(-4, -4, 2, 2).unwrap()),
        Selection::Mask(BinaryMask::uniform(document.canvas_rect(), false).unwrap()),
    ] {
        assert!(
            capture(&document, selection, id, &budget, &|| Ok(()))
                .unwrap()
                .is_none()
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    assert!(
        capture(
            &document,
            Selection::Inactive,
            Uuid::nil(),
            &budget,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        capture(
            &document,
            Selection::Inactive,
            id,
            &SharedBudget::new(1),
            &|| Ok(())
        )
        .is_err()
    );
    let count = Cell::new(0);
    let check = || {
        count.set(count.get() + 1);
        Ok(())
    };
    drop(capture(&document, Selection::Inactive, id, &budget, &check).unwrap());
    for stop in 1..=count.get() {
        let visits = Cell::new(0);
        let check = || {
            visits.set(visits.get() + 1);
            if visits.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        };
        assert!(capture(&document, Selection::Inactive, id, &budget, &check).is_err());
        assert_eq!(budget.usage().reserved, 0);
    }
    let clipped = capture(
        &document,
        Selection::Rect(Rect::new(-2, 1, 4, 3).unwrap()),
        id,
        &budget,
        &|| Ok(()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(clipped.origin, (0, 1));
    assert_eq!((clipped.image.width(), clipped.image.height()), (2, 3));
}

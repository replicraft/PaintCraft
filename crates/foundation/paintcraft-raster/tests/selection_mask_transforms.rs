use paintcraft_core::types::{
    Error, ErrorCode, Rect, orthogonal::OrthogonalTransform as T, resources::SharedBudget,
    text::MaskSpan,
};
use paintcraft_raster::selection::{BinaryMask, MaskLimits};
use std::cell::Cell;
const TRANSFORMS: [T; 5] = [
    T::RotateRight,
    T::RotateLeft,
    T::Rotate180,
    T::FlipHorizontal,
    T::FlipVertical,
];

fn oracle(mask: &BinaryMask, area: Rect, transform: T) -> (i64, i64, Vec<u8>) {
    let (w, h) = (area.width(), area.height());
    let (dw, dh) = if matches!(transform, T::RotateRight | T::RotateLeft) {
        (h, w)
    } else {
        (w, h)
    };
    let mut output = vec![0; (dw * dh) as usize];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match transform {
                T::RotateRight => (h - y - 1, x),
                T::RotateLeft => (y, w - x - 1),
                T::Rotate180 => (w - x - 1, h - y - 1),
                T::FlipHorizontal => (w - x - 1, y),
                T::FlipVertical => (x, h - y - 1),
            };
            output[(dy * dw + dx) as usize] = mask.coverage(area.x0 + x, area.y0 + y);
        }
    }
    (dw, dh, output)
}

#[test]
fn every_small_mask_inversion_and_translated_source_area_matches_forward_pixel_oracle() {
    let budget = SharedBudget::new(8 << 20);
    for selected in 0u8..64 {
        let domain = Rect::new(-2, -1, 3, 2).unwrap();
        let spans: Vec<_> = (0..6)
            .filter(|index| selected >> index & 1 != 0)
            .map(|index| MaskSpan {
                y: index / 3 - 1,
                x0: index % 3 - 2,
                x1: index % 3 - 2,
                coverage: 255,
            })
            .collect();
        let original = BinaryMask::from_spans(
            domain,
            false,
            &spans,
            &budget,
            MaskLimits::default(),
            &|| Ok(()),
        )
        .unwrap();
        for inverse in [false, true] {
            let source = if inverse {
                original.inverted()
            } else {
                original.clone()
            };
            for delta in [(0, 0), (-255, 257)] {
                let source = source.translated(delta.0, delta.1).unwrap();
                let area = Rect::new(-3 + delta.0, -2 + delta.1, 5, 4).unwrap();
                for transform in TRANSFORMS {
                    let (dw, dh, expected) = oracle(&source, area, transform);
                    let transformed = source
                        .transformed_checked(
                            area,
                            transform,
                            &budget,
                            MaskLimits::default(),
                            &|| Ok(()),
                        )
                        .unwrap();
                    assert_eq!(
                        transformed.domain(),
                        Rect::new(area.x0, area.y0, dw, dh).unwrap()
                    );
                    for y in 0..dh {
                        for x in 0..dw {
                            assert_eq!(
                                transformed.coverage(area.x0 + x, area.y0 + y),
                                expected[(y * dw + x) as usize]
                            );
                        }
                    }
                    assert_eq!(transformed.coverage(area.x0 - 1, area.y0), 0);
                    assert_eq!(transformed.coverage(area.x0, area.y0 + dh), 0);
                    // Opposite turns and self-inverse flips restore every source sample.
                    let restored = transformed
                        .transformed_checked(
                            transformed.domain(),
                            transform.inverse(),
                            &budget,
                            MaskLimits::default(),
                            &|| Ok(()),
                        )
                        .unwrap();
                    for y in area.y0..area.y1 {
                        for x in area.x0..area.x1 {
                            assert_eq!(restored.coverage(x, y), source.coverage(x, y));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn huge_uniform_transform_is_metadata_only_and_bounds_are_checked_before_construction() {
    let area = Rect::new(-500_000, -500_000, 1_048_576, 524_288).unwrap();
    let budget = SharedBudget::new(0);
    for selected in [false, true] {
        let source = BinaryMask::uniform(area, selected).unwrap();
        for transform in TRANSFORMS {
            let output = source
                .transformed_checked(
                    area,
                    transform,
                    &budget,
                    MaskLimits {
                        work: 1,
                        ..Default::default()
                    },
                    &|| Ok(()),
                )
                .unwrap();
            assert_eq!(
                output.uniform_coverage(),
                Some(if selected { 255 } else { 0 })
            );
            assert_eq!(output.storage_usage(), None);
        }
    }
    assert_eq!(budget.usage().reserved, 0);
    let max = paintcraft_core::types::policy::COORD_ABS_MAX;
    let area = Rect::new(max - 2, 0, 2, 4).unwrap();
    let source = BinaryMask::uniform(area, true).unwrap();
    assert_eq!(
        source
            .transformed_checked(
                area,
                T::RotateRight,
                &budget,
                MaskLimits::default(),
                &|| Ok(())
            )
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn mask_transform_cancellation_and_work_or_byte_denial_release_every_private_charge() {
    let source_budget = SharedBudget::new(1 << 20);
    let area = Rect::new(-3, -2, 32, 24).unwrap();
    let source = BinaryMask::from_spans(
        area,
        false,
        &[MaskSpan {
            y: 3,
            x0: 2,
            x1: 18,
            coverage: 255,
        }],
        &source_budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let output_budget = SharedBudget::new(2 << 20);
    let original_bytes = source_budget.usage().reserved;
    let calls = Cell::new(0);
    drop(
        source
            .transformed_checked(
                area,
                T::RotateRight,
                &output_budget,
                MaskLimits::default(),
                &|| {
                    calls.set(calls.get() + 1);
                    Ok(())
                },
            )
            .unwrap(),
    );
    assert_eq!(output_budget.usage().reserved, 0);
    for failure in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = source
            .transformed_checked(
                area,
                T::RotateRight,
                &output_budget,
                MaskLimits::default(),
                &|| {
                    seen.set(seen.get() + 1);
                    if seen.get() == failure {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel mapped mask",
                        ))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(output_budget.usage().reserved, 0);
        assert_eq!(source_budget.usage().reserved, original_bytes);
    }
    let denied = SharedBudget::new(0);
    assert!(
        source
            .transformed_checked(
                area,
                T::RotateRight,
                &denied,
                MaskLimits::default(),
                &|| Ok(())
            )
            .is_err()
    );
    assert_eq!(denied.usage().reserved, 0);
    assert!(
        source
            .transformed_checked(
                area,
                T::RotateRight,
                &output_budget,
                MaskLimits {
                    work: 50,
                    ..Default::default()
                },
                &|| Ok(())
            )
            .is_err()
    );
    assert_eq!(output_budget.usage().reserved, 0);
    let mut remaining = 1 << 20;
    let transformed = source
        .transformed_with_remaining_checked(
            area,
            T::RotateRight,
            &output_budget,
            MaskLimits::default(),
            &mut remaining,
            &|| Ok(()),
        )
        .unwrap();
    assert!(
        remaining < (1 << 20) - area.area() as u64,
        "source visits plus span construction share admission"
    );
    drop(transformed);
    assert_eq!(output_budget.usage().reserved, 0);
}

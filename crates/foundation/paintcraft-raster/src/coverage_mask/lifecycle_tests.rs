use super::*;
use crate::selection::{BinaryMask, MaskLimits};
use paintcraft_core::types::{orthogonal::OrthogonalTransform as T, text::MaskSpan};
fn ok() -> Result<()> {
    Ok(())
}
fn r(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn samples() -> A8Mask {
    let a = [
        Run {
            x0: 1,
            x1: 2,
            coverage: 1,
        },
        Run {
            x0: 2,
            x1: 3,
            coverage: 127,
        },
    ];
    let b = [
        Run {
            x0: 0,
            x1: 1,
            coverage: 128,
        },
        Run {
            x0: 1,
            x1: 2,
            coverage: 254,
        },
        Run {
            x0: 2,
            x1: 3,
            coverage: 255,
        },
    ];
    A8Mask::from_bands(
        r(0, 0, 3, 2),
        &[
            Band {
                y0: 0,
                y1: 1,
                runs: &a,
            },
            Band {
                y0: 1,
                y1: 2,
                runs: &b,
            },
        ],
        &SharedBudget::new(1 << 20),
        Limits::default(),
        &ok,
    )
    .unwrap()
}
#[test]
fn cached_stats_and_grouped_visit_describe_canonical_view_and_reserved_decode_account() {
    let mask = samples();
    assert_eq!(mask.stats(), Stats { bands: 2, runs: 5 });
    let clipped = mask
        .clipped_checked(r(1, 0, 1, 2), 1000, &ok)
        .unwrap()
        .unwrap();
    assert_eq!(clipped.stats(), Stats { bands: 2, runs: 2 });
    let mut events = Vec::new();
    clipped
        .visit_bands_checked(1000, &ok, |e| {
            events.push(e);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        events,
        vec![
            Visit::Band {
                y0: 0,
                y1: 1,
                runs: 1
            },
            Visit::Run(Run {
                x0: 1,
                x1: 2,
                coverage: 1
            }),
            Visit::Band {
                y0: 1,
                y1: 2,
                runs: 1
            },
            Visit::Run(Run {
                x0: 1,
                x1: 2,
                coverage: 254
            })
        ]
    );
    let quote = retained_quote(1, 1).unwrap();
    let budget = SharedBudget::new(quote + 32);
    let scope = budget
        .reserve_capacity(quote + 32)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let mut scratch = scope.account();
    scratch.resize(32).unwrap();
    let runs = [Run {
        x0: 0,
        x1: 2,
        coverage: 127,
    }];
    let decoded = A8Mask::from_bands_in_scope(
        r(0, 0, 2, 2),
        &[Band {
            y0: 0,
            y1: 2,
            runs: &runs,
        }],
        &scope,
        Limits::default(),
        &ok,
    )
    .unwrap();
    assert_eq!(decoded.storage_usage().unwrap().1, quote);
    assert_eq!(budget.usage().reserved, quote + 32);
    drop(scratch);
    drop(scope);
    assert_eq!(budget.usage().reserved, quote);
    drop(decoded);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(retained_quote(0, 0).unwrap(), 0);
    assert!(retained_quote(0, 1).is_err());
}
#[test]
fn uniform_region_proof_includes_holes_finite_outside_and_fractional_samples() {
    let mask = samples();
    for (y, row) in [[0, 1, 127], [128, 254, 255]].iter().enumerate() {
        for (x, &value) in row.iter().enumerate() {
            assert_eq!(
                mask.uniform_region_checked(r(x as i64, y as i64, 1, 1), 1000, &ok)
                    .unwrap(),
                Some(value)
            );
        }
    }
    assert_eq!(
        mask.uniform_region_checked(r(0, 0, 3, 2), 1000, &ok)
            .unwrap(),
        None
    );
    assert_eq!(
        mask.uniform_region_checked(r(-20, -20, 10, 10), 1000, &ok)
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        mask.uniform_region_checked(r(2, 1, 2, 1), 1000, &ok)
            .unwrap(),
        None
    );
    assert_eq!(
        mask.uniform_region_checked(r(0, 0, 1, 2), 0, &ok)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn exact_orthogonal_mapping_matches_independent_literal_sample_planes() {
    let source = samples();
    let digest = source.digest();
    let budget = SharedBudget::new(1 << 20);
    for (t, size, expected) in [
        (T::RotateRight, (2, 3), vec![128, 0, 254, 1, 255, 127]),
        (T::RotateLeft, (2, 3), vec![127, 255, 1, 254, 0, 128]),
        (T::Rotate180, (3, 2), vec![255, 254, 128, 127, 1, 0]),
        (T::FlipHorizontal, (3, 2), vec![127, 1, 0, 255, 254, 128]),
        (T::FlipVertical, (3, 2), vec![128, 254, 255, 0, 1, 127]),
    ] {
        let mapped = source
            .orthogonal_checked((3, 2), t, &budget, Limits::default(), &ok)
            .unwrap();
        assert_eq!(mapped.domain(), r(0, 0, size.0, size.1));
        let mut actual = Vec::new();
        for y in 0..size.1 {
            for x in 0..size.0 {
                actual.push(mapped.coverage(x, y));
            }
        }
        assert_eq!(actual, expected, "{}", t.name());
        let restored = mapped
            .orthogonal_checked(
                (size.0 as u32, size.1 as u32),
                t.inverse(),
                &budget,
                Limits::default(),
                &ok,
            )
            .unwrap();
        assert_eq!(restored, source);
        assert_eq!(restored.digest(), digest);
    }
}
#[test]
fn crop_is_full_new_source_domain_and_disjoint_coverage_stays_zero_intent() {
    let source = samples();
    let crop = source.cropped_checked(r(1, 0, 2, 2), 1000, &ok).unwrap();
    assert_eq!(crop.domain(), r(0, 0, 2, 2));
    assert_eq!(
        [
            crop.coverage(0, 0),
            crop.coverage(1, 0),
            crop.coverage(0, 1),
            crop.coverage(1, 1)
        ],
        [1, 127, 254, 255]
    );
    let zero = source
        .cropped_checked(r(100, 100, 3, 4), 1000, &ok)
        .unwrap();
    assert_eq!(zero.domain(), r(0, 0, 3, 4));
    assert_eq!(zero.stats(), Stats::default());
    assert_eq!(
        zero.uniform_region_checked(zero.domain(), 100, &ok)
            .unwrap(),
        Some(0)
    );
}
#[test]
fn selection_capture_covers_rectangle_empty_arbitrary_inverted_and_negative_layer_origin() {
    let budget = SharedBudget::new(4 << 20);
    let domain = r(-5, -4, 12, 10);
    let extent = r(-2, -1, 6, 4);
    let spans = [
        MaskSpan {
            y: 0,
            x0: -1,
            x1: 0,
            coverage: 255,
        },
        MaskSpan {
            y: 2,
            x0: 2,
            x1: 3,
            coverage: 255,
        },
    ];
    let original =
        BinaryMask::from_spans(domain, false, &spans, &budget, MaskLimits::default(), &ok).unwrap();
    for selection in [
        crate::Selection::Empty,
        crate::Selection::Rect(r(-1, 0, 4, 2)),
        crate::Selection::Mask(original.clone()),
        crate::Selection::Mask(original.inverted()),
    ] {
        let mask =
            A8Mask::from_selection(&selection, extent, &budget, Limits::default(), &ok).unwrap();
        assert_eq!(mask.domain(), r(0, 0, 6, 4));
        for y in 0..4 {
            for x in 0..6 {
                let expected = match &selection {
                    crate::Selection::Empty => 0,
                    crate::Selection::Rect(r) => {
                        if r.contains(x - 2, y - 1) {
                            255
                        } else {
                            0
                        }
                    }
                    crate::Selection::Mask(m) => m.coverage(x - 2, y - 1),
                    _ => unreachable!(),
                };
                assert_eq!(mask.coverage(x, y), expected);
            }
        }
    }
    assert_eq!(
        A8Mask::from_selection(
            &crate::Selection::Inactive,
            extent,
            &budget,
            Limits::default(),
            &ok
        )
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
}
#[test]
fn huge_full_default_selection_with_one_hole_does_not_walk_or_allocate_canvas_area() {
    let budget = SharedBudget::new(4 << 20);
    let domain = r(0, 0, 1_000_000, 1_000_000);
    let binary = BinaryMask::from_spans(
        domain,
        true,
        &[MaskSpan {
            y: 4,
            x0: 3,
            x1: 3,
            coverage: 0,
        }],
        &budget,
        MaskLimits::default(),
        &ok,
    )
    .unwrap();
    let mask = A8Mask::from_selection(
        &crate::Selection::Mask(binary),
        domain,
        &budget,
        Limits {
            bands: 3,
            runs: 4,
            ..Limits::default()
        },
        &ok,
    )
    .unwrap();
    assert_eq!(mask.coverage(3, 4), 0);
    assert_eq!(mask.coverage(2, 4), 255);
    assert_eq!(mask.coverage(999_999, 999_999), 255);
    assert_eq!(mask.stats(), Stats { bands: 3, runs: 4 });
    assert!(mask.storage_usage().unwrap().1 < 1024);
}
#[test]
fn every_observed_mapping_and_selection_capture_checkpoint_preserves_source_and_releases_work() {
    let source = samples();
    let budget = SharedBudget::new(1 << 20);
    let calls = Cell::new(0usize);
    drop(
        source
            .orthogonal_checked((3, 2), T::RotateRight, &budget, Limits::default(), &|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    for fail in 1..=calls.get() {
        let n = Cell::new(0);
        let error = source
            .orthogonal_checked((3, 2), T::RotateRight, &budget, Limits::default(), &|| {
                n.set(n.get() + 1);
                if n.get() == fail {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "mapping sentinel",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.message, "mapping sentinel");
        assert_eq!(n.get(), fail);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(source.coverage(2, 1), 255);
    }
    let selection = crate::Selection::Rect(r(0, 0, 2, 1));
    let calls = Cell::new(0);
    drop(
        A8Mask::from_selection(
            &selection,
            r(0, 0, 3, 2),
            &budget,
            Limits::default(),
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap(),
    );
    for fail in 1..=calls.get() {
        let n = Cell::new(0);
        assert!(
            A8Mask::from_selection(
                &selection,
                r(0, 0, 3, 2),
                &budget,
                Limits::default(),
                &|| {
                    n.set(n.get() + 1);
                    if n.get() == fail {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "capture sentinel",
                        ))
                    } else {
                        Ok(())
                    }
                }
            )
            .is_err()
        );
        assert_eq!(budget.usage().reserved, 0);
    }
}

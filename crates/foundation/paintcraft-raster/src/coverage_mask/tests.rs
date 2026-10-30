use super::*;
use std::cell::Cell;
fn ok() -> Result<()> {
    Ok(())
}
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn limits() -> Limits {
    Limits::default()
}
fn abort() -> Error {
    Error::new(ErrorCode::OperationCancelled, "A8 cancellation sentinel")
}
#[test]
fn genuine_all_256_a8_samples_finite_edges_and_canonical_rectangle_storage() {
    let budget = SharedBudget::new(1 << 20);
    let runs: Vec<_> = (0..256)
        .map(|x| Run {
            x0: x,
            x1: x + 1,
            coverage: x as u8,
        })
        .collect();
    let domain = rect(0, 0, 256, 3);
    let mask = A8Mask::from_bands(
        domain,
        &[Band {
            y0: 0,
            y1: 3,
            runs: &runs,
        }],
        &budget,
        limits(),
        &ok,
    )
    .unwrap();
    for y in -1..4 {
        for x in -1..257 {
            assert_eq!(
                mask.coverage(x, y),
                if (0..256).contains(&x) && (0..3).contains(&y) {
                    x as u8
                } else {
                    0
                }
            );
        }
    }
    let huge = rect(-1_000_000, -1_000_000, 2_000_000, 2_000_000);
    let solid = A8Mask::uniform(huge, 127, &budget, limits(), &ok).unwrap();
    assert_eq!(solid.coverage(999_999, 999_999), 127);
    assert_eq!(solid.coverage(1_000_000, 0), 0);
    assert!(
        solid.storage_usage().unwrap().1 < 1024,
        "uniform rectangle never allocates logical pixels"
    );
    let zero = A8Mask::uniform(
        huge,
        0,
        &budget,
        Limits {
            bands: 0,
            runs: 0,
            ..limits()
        },
        &ok,
    )
    .unwrap();
    assert!(zero.storage_usage().is_none());
    assert_ne!(
        zero.digest(),
        A8Mask::uniform(domain, 0, &budget, limits(), &ok)
            .unwrap()
            .digest()
    );
}
#[test]
fn equivalent_zero_runs_splits_and_bands_have_exact_same_semantics_and_digest() {
    let budget = SharedBudget::new(8192);
    let domain = rect(-2, -3, 8, 6);
    let original = A8Mask::uniform(domain, 128, &budget, limits(), &ok).unwrap();
    let runs = [
        Run {
            x0: -2,
            x1: 0,
            coverage: 128,
        },
        Run {
            x0: 0,
            x1: 6,
            coverage: 128,
        },
    ];
    let split = A8Mask::from_bands(
        domain,
        &[
            Band {
                y0: -3,
                y1: -1,
                runs: &runs,
            },
            Band {
                y0: -1,
                y1: 3,
                runs: &runs,
            },
        ],
        &budget,
        limits(),
        &ok,
    )
    .unwrap();
    assert_eq!(original, split);
    assert_eq!(original.digest(), split.digest());
    assert!(original.same_coverage_checked(&split, 100, &ok).unwrap());
    let zero = [Run {
        x0: -2,
        x1: 6,
        coverage: 0,
    }];
    let empty = A8Mask::from_bands(
        domain,
        &[Band {
            y0: -3,
            y1: 3,
            runs: &zero,
        }],
        &budget,
        limits(),
        &ok,
    )
    .unwrap();
    assert_eq!(
        empty,
        A8Mask::from_bands(domain, &[], &budget, limits(), &ok).unwrap()
    );
}
#[test]
fn clipped_canonical_bands_merge_and_reframe_never_resurrects_hidden_samples() {
    let budget = SharedBudget::new(8192);
    let domain = rect(0, 0, 4, 2);
    let a = [
        Run {
            x0: 0,
            x1: 2,
            coverage: 128,
        },
        Run {
            x0: 3,
            x1: 4,
            coverage: 1,
        },
    ];
    let b = [
        Run {
            x0: 0,
            x1: 2,
            coverage: 128,
        },
        Run {
            x0: 3,
            x1: 4,
            coverage: 2,
        },
    ];
    let mask = A8Mask::from_bands(
        domain,
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
        &budget,
        limits(),
        &ok,
    )
    .unwrap();
    let clip = mask
        .clipped_checked(rect(0, 0, 2, 2), 1000, &ok)
        .unwrap()
        .unwrap();
    let solid = A8Mask::uniform(rect(0, 0, 2, 2), 128, &budget, limits(), &ok).unwrap();
    assert_eq!(clip, solid);
    assert_eq!(clip.digest(), solid.digest());
    let expanded = clip.reframed_checked(domain, 1000, &ok).unwrap();
    assert_eq!(expanded.coverage(3, 0), 0);
    assert_eq!(expanded.coverage(3, 1), 0);
    let independent = A8Mask::from_bands(
        domain,
        &[Band {
            y0: 0,
            y1: 2,
            runs: &a[..1],
        }],
        &budget,
        limits(),
        &ok,
    )
    .unwrap();
    assert_eq!(expanded, independent);
    assert!(
        mask.clipped_checked(rect(20, 20, 1, 1), 100, &ok)
            .unwrap()
            .is_none()
    );
}
#[test]
fn translations_are_single_depth_share_charge_and_hash_actual_field_not_view_history() {
    let budget = SharedBudget::new(8192);
    let domain = rect(0, 0, 4, 2);
    let mask = A8Mask::uniform(domain, 254, &budget, limits(), &ok).unwrap();
    let usage = mask.storage_usage();
    let charge = budget.usage().reserved;
    let mut shifted = mask.clone();
    for _ in 0..100 {
        shifted = shifted.translated_checked(-1, 2, 100, &ok).unwrap();
    }
    assert_eq!(shifted.storage_usage(), usage);
    assert_eq!(budget.usage().reserved, charge);
    let independent = A8Mask::uniform(
        rect(-100, 200, 4, 2),
        254,
        &SharedBudget::new(8192),
        limits(),
        &ok,
    )
    .unwrap();
    assert_eq!(shifted, independent);
    assert_eq!(shifted.digest(), independent.digest());
    assert_eq!(shifted.coverage(-100, 200), 254);
    drop(mask);
    assert_eq!(budget.usage().reserved, charge);
    drop(shifted);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn malformed_order_overlap_bounds_and_caller_limits_refuse_without_leaked_reservations() {
    let domain = rect(0, 0, 4, 2);
    let budget = SharedBudget::new(8192);
    for runs in [
        vec![Run {
            x0: -1,
            x1: 1,
            coverage: 1,
        }],
        vec![Run {
            x0: 1,
            x1: 1,
            coverage: 1,
        }],
        vec![
            Run {
                x0: 0,
                x1: 3,
                coverage: 127,
            },
            Run {
                x0: 2,
                x1: 4,
                coverage: 128,
            },
        ],
    ] {
        assert_eq!(
            A8Mask::from_bands(
                domain,
                &[Band {
                    y0: 0,
                    y1: 1,
                    runs: &runs
                }],
                &budget,
                limits(),
                &ok
            )
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let runs = [Run {
        x0: 0,
        x1: 1,
        coverage: 1,
    }];
    assert!(
        A8Mask::from_bands(
            domain,
            &[
                Band {
                    y0: 1,
                    y1: 2,
                    runs: &runs
                },
                Band {
                    y0: 0,
                    y1: 1,
                    runs: &runs
                }
            ],
            &budget,
            limits(),
            &ok
        )
        .is_err()
    );
    for bound in [
        Limits {
            work: 0,
            ..limits()
        },
        Limits {
            bands: 0,
            ..limits()
        },
        Limits {
            runs: 0,
            ..limits()
        },
    ] {
        assert_eq!(
            A8Mask::uniform(domain, 255, &budget, bound, &ok)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    assert_eq!(
        A8Mask::uniform(domain, 255, &SharedBudget::new(1), limits(), &ok)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn each_observed_constructor_checkpoint_propagates_one_shot_cancel_and_releases_private_work() {
    let budget = SharedBudget::new(8192);
    let domain = rect(0, 0, 4, 2);
    let runs = [
        Run {
            x0: 0,
            x1: 1,
            coverage: 0,
        },
        Run {
            x0: 1,
            x1: 2,
            coverage: 1,
        },
        Run {
            x0: 2,
            x1: 4,
            coverage: 127,
        },
    ];
    let bands = [
        Band {
            y0: 0,
            y1: 1,
            runs: &runs,
        },
        Band {
            y0: 1,
            y1: 2,
            runs: &runs,
        },
    ];
    let calls = Cell::new(0);
    drop(
        A8Mask::from_bands(domain, &bands, &budget, limits(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    for fail in 1..=calls.get() {
        let n = Cell::new(0);
        let error = A8Mask::from_bands(domain, &bands, &budget, limits(), &|| {
            n.set(n.get() + 1);
            if n.get() == fail {
                Err(abort())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "A8 cancellation sentinel");
        assert_eq!(n.get(), fail);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn invisible_record_scans_and_canonical_merge_work_remain_cancellable_and_bounded() {
    let budget = SharedBudget::new(1 << 20);
    let domain = rect(0, 0, 100, 1000);
    let rows: Vec<_> = (0..1000)
        .map(|y| {
            [Run {
                x0: 0,
                x1: 1,
                coverage: if y % 2 == 0 { 127 } else { 128 },
            }]
        })
        .collect();
    let bands: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(y, r)| Band {
            y0: y as i64,
            y1: y as i64 + 1,
            runs: r,
        })
        .collect();
    let mask = A8Mask::from_bands(domain, &bands, &budget, limits(), &ok).unwrap();
    let before = budget.usage().reserved;
    let calls = Cell::new(0);
    let error = mask
        .clipped_checked(rect(50, 0, 1, 1000), 100_000, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 500 {
                Err(abort())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(calls.get(), 500);
    assert_eq!(
        mask.clipped_checked(rect(50, 0, 1, 1000), 10, &ok)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, before);
    assert_eq!(mask.coverage(0, 999), 128);
}
#[test]
fn checked_semantic_comparison_and_rectangle_visits_preserve_fractional_samples() {
    let budget = SharedBudget::new(8192);
    let domain = rect(0, 0, 4, 2);
    let mask = A8Mask::uniform(domain, 1, &budget, limits(), &ok).unwrap();
    let mut rectangles = Vec::new();
    mask.visit_rectangles_checked(100, &ok, |y0, y1, run| {
        rectangles.push((y0, y1, run));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        rectangles,
        vec![(
            0,
            2,
            Run {
                x0: 0,
                x1: 4,
                coverage: 1
            }
        )]
    );
    assert_eq!(
        mask.same_coverage_checked(&mask, 0, &ok).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        mask.same_coverage_checked(&mask, 100, &|| Err(abort()))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    let mut emitted = 0;
    assert_eq!(
        mask.visit_rectangles_checked(100, &ok, |_, _, _| {
            emitted += 1;
            Err(abort())
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(emitted, 1);
}

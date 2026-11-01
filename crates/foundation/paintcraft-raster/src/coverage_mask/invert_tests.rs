//! Finite scalar expectations and ownership/rollback controls; no dense kernel.
use super::*;
fn ok() -> Result<()> {
    Ok(())
}
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn abort() -> Error {
    Error::new(ErrorCode::OperationCancelled, "invert cancelled")
}
fn sample(budget: &SharedBudget) -> A8Mask {
    let a = [
        Run {
            x0: -2,
            x1: 0,
            coverage: 1,
        },
        Run {
            x0: 0,
            x1: 2,
            coverage: 127,
        },
        Run {
            x0: 3,
            x1: 5,
            coverage: 255,
        },
    ];
    let b = [
        Run {
            x0: -3,
            x1: -1,
            coverage: 128,
        },
        Run {
            x0: -1,
            x1: 1,
            coverage: 254,
        },
    ];
    A8Mask::from_bands(
        rect(-3, -2, 8, 6),
        &[
            Band {
                y0: -1,
                y1: 1,
                runs: &a,
            },
            Band {
                y0: 2,
                y1: 3,
                runs: &b,
            },
        ],
        budget,
        Limits::default(),
        &ok,
    )
    .unwrap()
}
fn input(x: i64, y: i64) -> u8 {
    match (x, y) {
        (-2..=-1, -1..=0) => 1,
        (0..=1, -1..=0) => 127,
        (3..=4, -1..=0) => 255,
        (-3..=-2, 2) => 128,
        (-1..=0, 2) => 254,
        _ => 0,
    }
}
#[test]
fn sparse_gaps_fractional_values_and_negative_finite_domain_have_literal_complement() {
    let budget = SharedBudget::new(1 << 20);
    let source = sample(&budget);
    let before = source.clone();
    let inverse = source
        .inverted_checked(&budget, Limits::default(), &ok)
        .unwrap();
    assert_eq!(inverse.domain(), rect(-3, -2, 8, 6));
    assert_eq!(inverse.default_coverage(), 0);
    assert_eq!(inverse.stats(), Stats { bands: 5, runs: 10 });
    for y in -3..5 {
        for x in -4..6 {
            assert_eq!(source.coverage(x, y), input(x, y));
            let expected = if (-3..5).contains(&x) && (-2..4).contains(&y) {
                255 - input(x, y)
            } else {
                0
            };
            assert_eq!(inverse.coverage(x, y), expected, "{x},{y}");
        }
    }
    assert_eq!(source, before);
    assert_eq!(
        inverse
            .inverted_checked(&budget, Limits::default(), &ok)
            .unwrap(),
        source
    );
}
#[test]
fn every_uniform_byte_complements_and_double_inverts_with_zero_outside() {
    let budget = SharedBudget::new(1 << 20);
    let domain = rect(-2, 3, 4, 2);
    for coverage in 0..=255 {
        let source = A8Mask::uniform(domain, coverage, &budget, Limits::default(), &ok).unwrap();
        let inverse = source
            .inverted_checked(&budget, Limits::default(), &ok)
            .unwrap();
        assert_eq!(inverse.coverage(-2, 3), 255 - coverage);
        assert_eq!(inverse.coverage(1, 4), 255 - coverage);
        for (x, y) in [(-3, 3), (2, 3), (-2, 2), (-2, 5)] {
            assert_eq!(inverse.coverage(x, y), 0);
        }
        let again = inverse
            .inverted_checked(&budget, Limits::default(), &ok)
            .unwrap();
        assert_eq!(again, source);
        assert_eq!(again.digest(), source.digest());
    }
}
#[test]
fn huge_empty_field_becomes_one_rectangle_and_full_field_needs_no_output_bytes() {
    let budget = SharedBudget::new(4096);
    let m = COORD_ABS_MAX;
    let domain = Rect {
        x0: -m,
        y0: -m,
        x1: m,
        y1: m,
    };
    let empty = A8Mask::from_bands(domain, &[], &budget, Limits::default(), &ok).unwrap();
    let limits = Limits {
        work: 500,
        bands: 1,
        runs: 1,
    };
    let full = empty.inverted_checked(&budget, limits, &ok).unwrap();
    assert_eq!(full.stats(), Stats { bands: 1, runs: 1 });
    assert_eq!(full.coverage(-m, -m), 255);
    assert_eq!(full.coverage(m - 1, m - 1), 255);
    assert_eq!(full.coverage(m, 0), 0);
    let no_bytes = SharedBudget::new(0);
    let zero = full
        .inverted_checked(
            &no_bytes,
            Limits {
                bands: 0,
                runs: 0,
                ..limits
            },
            &ok,
        )
        .unwrap();
    assert_eq!(zero.domain(), domain);
    assert_eq!(zero.stats(), Stats::default());
    assert_eq!(zero.storage_usage(), None);
    assert_eq!(zero, empty);
    assert_eq!(no_bytes.usage().reserved, 0);
}
#[test]
fn clipped_reframed_and_translated_views_do_not_reveal_hidden_source_storage() {
    let budget = SharedBudget::new(1 << 20);
    let source = sample(&budget);
    let kept = rect(-1, -1, 2, 2);
    let view = source
        .clipped_checked(kept, 10000, &ok)
        .unwrap()
        .unwrap()
        .reframed_checked(source.domain(), 10000, &ok)
        .unwrap()
        .translated_checked(11, -7, 10000, &ok)
        .unwrap();
    let inverse = view
        .inverted_checked(&budget, Limits::default(), &ok)
        .unwrap();
    assert_eq!(
        inverse.domain(),
        source.domain().translated(11, -7).unwrap()
    );
    for y in -3..5 {
        for x in -4..6 {
            let expected = if source.domain().contains(x, y) {
                255 - if kept.contains(x, y) { input(x, y) } else { 0 }
            } else {
                0
            };
            assert_eq!(inverse.coverage(x + 11, y - 7), expected);
        }
    }
    assert_eq!(
        inverse
            .inverted_checked(&budget, Limits::default(), &ok)
            .unwrap(),
        view
    );
}
#[test]
fn canonical_counts_are_aggregate_and_limits_cannot_reset_between_passes_or_hash() {
    let budget = SharedBudget::new(1 << 20);
    let source = sample(&budget);
    let before = budget.usage().reserved;
    for (bands, runs) in [(4, 10), (5, 9)] {
        let error = source
            .inverted_checked(
                &budget,
                Limits {
                    bands,
                    runs,
                    ..Limits::default()
                },
                &ok,
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(budget.usage().reserved, before);
    }
    let calls = Cell::new(0u64);
    let limits = Limits {
        bands: 5,
        runs: 10,
        ..Limits::default()
    };
    drop(
        source
            .inverted_checked(&budget, limits, &|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    let total = calls.get();
    assert!(total > 1);
    assert_eq!(
        source
            .inverted_checked(
                &budget,
                Limits {
                    work: total - 1,
                    ..limits
                },
                &ok
            )
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, before);
    drop(
        source
            .inverted_checked(
                &budget,
                Limits {
                    work: total,
                    ..limits
                },
                &ok,
            )
            .unwrap(),
    );
    assert_eq!(budget.usage().reserved, before);
}
#[test]
fn old_and_new_payloads_compete_in_the_same_realm_and_clones_are_not_double_charged() {
    let old_quote = retained_quote(2, 5).unwrap();
    let new_quote = retained_quote(5, 10).unwrap();
    let denied = SharedBudget::new(old_quote + new_quote - 1);
    let source = sample(&denied);
    let before = denied.usage().reserved;
    assert_eq!(before, old_quote);
    assert_eq!(
        source
            .inverted_checked(&denied, Limits::default(), &ok)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(denied.usage().reserved, before);
    let budget = SharedBudget::new(1 << 20);
    let source = sample(&budget);
    let before = budget.usage().reserved;
    let inverse = source
        .inverted_checked(&budget, Limits::default(), &ok)
        .unwrap();
    let payload = inverse.storage_usage().unwrap().1;
    assert_eq!(budget.usage().reserved, before + payload);
    let clone = inverse.clone();
    drop(inverse);
    assert_eq!(budget.usage().reserved, before + payload);
    drop(clone);
    assert_eq!(budget.usage().reserved, before);
}
#[test]
fn every_observed_callback_can_cancel_and_release_all_new_payloads_without_source_change() {
    let budget = SharedBudget::new(1 << 20);
    let source = sample(&budget);
    let digest = source.digest();
    let identity = source.storage_usage();
    let before = budget.usage().reserved;
    let calls = Cell::new(0);
    drop(
        source
            .inverted_checked(&budget, Limits::default(), &|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    let after_allocation = Cell::new(false);
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = source
            .inverted_checked(&budget, Limits::default(), &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    after_allocation
                        .set(after_allocation.get() || budget.usage().reserved > before);
                    Err(abort())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(seen.get(), stop);
        assert_eq!(budget.usage().reserved, before);
        assert_eq!(source.digest(), digest);
        assert_eq!(source.storage_usage(), identity);
        assert_eq!(source.coverage(-2, -1), 1);
    }
    assert!(
        after_allocation.get(),
        "must exercise cancellation after final allocation"
    );
}
#[test]
fn callback_unwind_after_allocation_releases_new_storage() {
    let budget = SharedBudget::new(1 << 20);
    let source = sample(&budget);
    let before = budget.usage().reserved;
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        source.inverted_checked(&budget, Limits::default(), &|| {
            assert!(
                budget.usage().reserved <= before,
                "injected callback panic after allocation"
            );
            Ok(())
        })
    }));
    assert!(failed.is_err());
    assert_eq!(budget.usage().reserved, before);
    assert_eq!(source.coverage(0, 0), 127);
}

#[test]
fn canonical_visible_rows_merge_after_clip_before_result_count_admission() {
    let budget = SharedBudget::new(1 << 20);
    let a = [
        Run {
            x0: 0,
            x1: 2,
            coverage: 127,
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
            coverage: 127,
        },
        Run {
            x0: 3,
            x1: 4,
            coverage: 254,
        },
    ];
    let source = A8Mask::from_bands(
        rect(0, 0, 4, 2),
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
        Limits::default(),
        &ok,
    )
    .unwrap();
    assert_eq!(source.stats().bands, 2);
    let view = source
        .clipped_checked(rect(0, 0, 2, 2), 10000, &ok)
        .unwrap()
        .unwrap()
        .reframed_checked(source.domain(), 10000, &ok)
        .unwrap();
    let inverse = view
        .inverted_checked(
            &budget,
            Limits {
                bands: 1,
                runs: 2,
                ..Limits::default()
            },
            &ok,
        )
        .unwrap();
    assert_eq!(inverse.stats(), Stats { bands: 1, runs: 2 });
    for y in 0..2 {
        assert_eq!(inverse.coverage(0, y), 128);
        assert_eq!(inverse.coverage(1, y), 128);
        assert_eq!(inverse.coverage(2, y), 255);
        assert_eq!(inverse.coverage(3, y), 255);
    }
    assert_eq!(
        inverse
            .inverted_checked(&budget, Limits::default(), &ok)
            .unwrap(),
        view
    );
    assert_eq!(source.coverage(3, 0), 1);
    assert_eq!(source.coverage(3, 1), 254);
}

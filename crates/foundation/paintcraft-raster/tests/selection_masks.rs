use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use paintcraft_core::types::{Error, ErrorCode, Rect, policy::COORD_ABS_MAX, text::MaskSpan};
use paintcraft_raster::selection::{BinaryMask, MaskLimits};
use std::cell::Cell;

fn ok() -> paintcraft_core::types::Result<()> {
    Ok(())
}
fn run(y: i64, x0: i64, x1: i64, coverage: u8) -> MaskSpan {
    MaskSpan {
        y,
        x0,
        x1,
        coverage,
    }
}
fn mask(domain: Rect, spans: &[MaskSpan], budget: &SharedBudget) -> BinaryMask {
    BinaryMask::from_spans(domain, false, spans, budget, MaskLimits::default(), &ok).unwrap()
}
// Independent per-pixel ray casting, with no sorted intersections or quotient rounding.
fn inside(vertices: &[(i64, i64)], x: i64, y: i64) -> bool {
    let (cx, cy) = (i128::from(x) * 2 + 1, i128::from(y) * 2 + 1);
    let mut parity = false;
    for i in 0..vertices.len() {
        let (mut a, mut b) = (vertices[i], vertices[(i + 1) % vertices.len()]);
        if a.1 > b.1 {
            std::mem::swap(&mut a, &mut b);
        }
        if cy >= i128::from(a.1) * 2 && cy < i128::from(b.1) * 2 {
            let cross = (i128::from(b.0) - i128::from(a.0)) * (cy - 2 * i128::from(a.1))
                - (i128::from(b.1) - i128::from(a.1)) * (cx - 2 * i128::from(a.0));
            if cross > 0 {
                parity = !parity;
            }
        }
    }
    parity
}
fn assert_rows(mask: &BinaryMask, area: Rect) {
    for y in area.y0 - 1..=area.y1 {
        let mut selected = Vec::new();
        mask.visit_row(y, area.x0 - 1, area.x1 + 1, u64::MAX, &ok, |a, b| {
            assert!(a < b);
            if let Some(&(_, previous)) = selected.last() {
                assert!(a > previous, "seams must merge");
            }
            selected.push((a, b));
            Ok(())
        })
        .unwrap();
        for x in area.x0 - 1..=area.x1 {
            assert_eq!(
                selected.iter().any(|&(a, b)| a <= x && x < b),
                mask.coverage(x, y) == 255,
                "{x},{y}"
            );
        }
    }
}
#[test]
fn million_square_full_empty_and_inverted_fields_have_finite_zero_storage() {
    let domain = Rect::new(0, 0, 1_048_576, 1_048_576).unwrap();
    for selected in [false, true] {
        let mask = BinaryMask::uniform(domain, selected).unwrap();
        assert_eq!(mask.stored_tiles(), 0);
        assert_eq!(
            mask.uniform_coverage(),
            Some(if selected { 255 } else { 0 })
        );
        let inverse = mask.inverted();
        for &(x, y) in &[
            (0, 0),
            (999_999, 999_999),
            (1_048_575, 1_048_575),
            (-1, 0),
            (1_048_576, 0),
        ] {
            let active = domain.contains(x, y);
            assert_eq!(
                mask.coverage(x, y),
                if active && selected { 255 } else { 0 }
            );
            assert_eq!(
                inverse.coverage(x, y),
                if active && !selected { 255 } else { 0 }
            );
        }
        let mut spans = Vec::new();
        let work = inverse
            .visit_row(1, -1, 1_048_577, 1, &ok, |a, b| {
                spans.push((a, b));
                Ok(())
            })
            .unwrap();
        assert_eq!(work, 1);
        assert_eq!(
            spans,
            if selected {
                vec![]
            } else {
                vec![(0, 1_048_576)]
            }
        );
    }
}
#[test]
fn sparse_inversion_clipping_and_translation_share_storage_and_lifetime_charge() {
    let budget = SharedBudget::new(4 << 20);
    let domain = Rect::new(-20, -20, 1_048_576, 1_048_576).unwrap();
    let original = mask(domain, &[run(-1, -2, 2, 255), run(0, -2, 2, 255)], &budget);
    assert_eq!(
        original.stored_tiles(),
        4,
        "negative tile boundaries use Euclidean division"
    );
    let bytes = budget.usage().reserved;
    assert!(bytes > 0 && bytes < 1 << 20);
    let inverse = original.inverted();
    let clipped = inverse
        .clipped(Rect::new(-4, -4, 8, 8).unwrap())
        .unwrap()
        .unwrap();
    let moved = clipped.translated(517, -273).unwrap();
    for other in [&inverse, &clipped, &moved] {
        assert!(original.shares_storage(other));
    }
    assert_eq!(budget.usage().reserved, bytes);
    for y in -4..4 {
        for x in -4..4 {
            assert_eq!(inverse.coverage(x, y), 255 - original.coverage(x, y));
            assert_eq!(moved.coverage(x + 517, y - 273), clipped.coverage(x, y));
        }
    }
    assert_eq!(clipped.coverage(4, 0), 0);
    assert_eq!(inverse.coverage(-21, 0), 0);
    assert_rows(&clipped, Rect::new(-4, -4, 8, 8).unwrap());
    assert_rows(&moved, Rect::new(513, -277, 8, 8).unwrap());
    drop(original);
    drop(inverse);
    drop(clipped);
    assert_eq!(budget.usage().reserved, bytes);
    drop(moved);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn rows_merge_tile_seams_and_full_domain_runs_canonicalize_without_retained_tiles() {
    let budget = SharedBudget::new(4 << 20);
    let domain = Rect::new(-2, -2, 260, 6).unwrap();
    let spans: Vec<_> = (domain.y0..domain.y1)
        .map(|y| run(y, domain.x0, domain.x1 - 1, 255))
        .collect();
    let full = mask(domain, &spans, &budget);
    assert_eq!(full.uniform_coverage(), Some(255));
    assert_eq!(full.stored_tiles(), 0);
    assert_eq!(budget.usage().reserved, 0);
    let cut = mask(
        domain,
        &[run(0, -1, 256, 255), run(1, -2, 257, 255)],
        &budget,
    );
    assert_rows(&cut, domain);
    assert_rows(&cut.inverted(), domain);
    let before = budget.usage().reserved;
    let copy = cut.clone();
    assert_eq!(budget.usage().reserved, before);
    drop(cut);
    assert!(copy.coverage(0, 0) > 0);
    drop(copy);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn polygons_match_independent_point_tests_for_concavity_crossings_degenerate_edges_and_limits() {
    let domain = Rect::new(-8, -8, 20, 20).unwrap();
    let budget = SharedBudget::new(8 << 20);
    let cases = vec![
        vec![(-4, -4), (8, -4), (8, 8), (-4, 8)],
        vec![(-7, -7), (9, 9), (-7, 9), (9, -7)],
        vec![(-6, -6), (9, -6), (9, 0), (0, 0), (0, 9), (-6, 9)],
        vec![(-4, -4), (-4, -4), (8, 8), (-4, 8), (8, -4), (-4, -4)],
        vec![(-1000, 0), (1000, 1), (0, 1000)],
        vec![(0, 0), (1, 1), (2, 2)],
        vec![(0, 0), (0, 0), (0, 0)],
    ];
    for vertices in cases {
        let field =
            BinaryMask::polygon(domain, &vertices, &budget, MaskLimits::default(), &ok).unwrap();
        for y in domain.y0..domain.y1 {
            for x in domain.x0..domain.x1 {
                assert_eq!(
                    field.coverage(x, y) == 255,
                    inside(&vertices, x, y),
                    "{vertices:?} at {x},{y}"
                );
            }
        }
        assert_rows(&field, domain);
        assert_rows(&field.inverted(), domain);
    }
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn deterministic_generated_polygons_and_large_coordinate_translations_keep_exact_samples() {
    let budget = SharedBudget::new(8 << 20);
    let domain = Rect::new(-9, -9, 19, 19).unwrap();
    let mut seed = 0x8765_4321_u64;
    for count in 3..20 {
        for _ in 0..12 {
            let vertices: Vec<_> = (0..count)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let x = (seed >> 32) as i64 % 31 - 15;
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (x, (seed >> 32) as i64 % 31 - 15)
                })
                .collect();
            let field = BinaryMask::polygon(domain, &vertices, &budget, MaskLimits::default(), &ok)
                .unwrap();
            for y in -9..10 {
                for x in -9..10 {
                    assert_eq!(
                        field.coverage(x, y) == 255,
                        inside(&vertices, x, y),
                        "{vertices:?} at {x},{y}"
                    );
                }
            }
        }
    }
    let delta = COORD_ABS_MAX - 32;
    let far_domain = domain.translated(delta, -delta).unwrap();
    let vertices = vec![(-8, -7), (9, 8), (-7, 9), (9, -8)];
    let far: Vec<_> = vertices
        .iter()
        .map(|&(x, y)| (x + delta, y - delta))
        .collect();
    let field = BinaryMask::polygon(far_domain, &far, &budget, MaskLimits::default(), &ok).unwrap();
    for y in -9..10 {
        for x in -9..10 {
            assert_eq!(
                field.coverage(x + delta, y - delta) == 255,
                inside(&vertices, x, y)
            );
        }
    }
}
#[test]
fn malformed_runs_budget_work_tile_limits_and_cancellation_release_private_construction() {
    let domain = Rect::new(0, 0, 1024, 1024).unwrap();
    let budget = SharedBudget::new(4 << 20);
    for spans in [
        vec![run(0, 0, 3, 128)],
        vec![run(0, 5, 4, 255)],
        vec![run(-1, 0, 3, 255)],
        vec![run(0, 0, 3, 255), run(0, 3, 5, 255)],
        vec![run(1, 0, 3, 255), run(0, 5, 6, 255)],
    ] {
        assert_eq!(
            BinaryMask::from_spans(domain, false, &spans, &budget, MaskLimits::default(), &ok)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    for limits in [
        MaskLimits {
            work: 0,
            ..Default::default()
        },
        MaskLimits {
            tiles: 0,
            ..Default::default()
        },
        MaskLimits {
            spans: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            BinaryMask::from_spans(domain, false, &[run(0, 0, 3, 255)], &budget, limits, &ok)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let denied = SharedBudget::new(1000);
    let error = BinaryMask::from_spans(
        domain,
        false,
        &[run(0, 0, 3, 255)],
        &denied,
        MaskLimits::default(),
        &ok,
    )
    .unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::SelectionMasks
    );
    assert_eq!(denied.usage().reserved, 0);
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        if calls.get() > 7 {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        BinaryMask::polygon(
            domain,
            &[(0, 0), (500, 0), (500, 500), (0, 500)],
            &budget,
            MaskLimits::default(),
            &cancelled
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(budget.usage().reserved, 0);
    let invalid = Rect {
        x0: i64::MIN,
        y0: 0,
        x1: i64::MAX,
        y1: 1,
    };
    assert_eq!(
        BinaryMask::uniform(invalid, true).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        BinaryMask::uniform(Rect::new(COORD_ABS_MAX - 2, 0, 2, 1).unwrap(), true)
            .unwrap()
            .translated(1, 0)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn nonzero_defaults_and_failed_replacements_keep_existing_snapshot_ownership() {
    let budget = SharedBudget::new(2 << 20);
    let domain = Rect::new(-3, -3, 10, 10).unwrap();
    let original = BinaryMask::from_spans(
        domain,
        true,
        &[run(0, -2, 2, 0)],
        &budget,
        MaskLimits::default(),
        &ok,
    )
    .unwrap();
    assert_eq!(original.default_coverage(), 255);
    assert_eq!(original.coverage(0, 0), 0);
    assert_eq!(original.coverage(0, 1), 255);
    assert_rows(&original, domain);
    assert_rows(&original.inverted(), domain);
    let held = budget.usage().reserved;
    let blocker = budget.reserve(budget.usage().limit - held).unwrap();
    assert_eq!(
        BinaryMask::from_spans(
            domain,
            false,
            &[run(1, 0, 3, 255)],
            &budget,
            MaskLimits::default(),
            &ok
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(original.coverage(0, 1), 255);
    assert_eq!(budget.usage().reserved, budget.usage().limit);
    drop(blocker);
    assert_eq!(budget.usage().reserved, held);
    drop(original);
    assert_eq!(budget.usage().reserved, 0);
    let spans: Vec<_> = (domain.y0..domain.y1)
        .map(|y| run(y, domain.x0, domain.x1 - 1, 0))
        .collect();
    let empty =
        BinaryMask::from_spans(domain, true, &spans, &budget, MaskLimits::default(), &ok).unwrap();
    assert_eq!(empty.uniform_coverage(), Some(0));
    assert_eq!(empty.stored_tiles(), 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn row_work_denial_and_callback_failure_do_not_mutate_or_release_the_source() {
    let domain = Rect::new(-2, -2, 264, 6).unwrap();
    let budget = SharedBudget::new(1 << 20);
    let field = mask(
        domain,
        &[run(0, -1, 2, 255), run(0, 255, 257, 255)],
        &budget,
    );
    let before = budget.usage().reserved;
    assert_eq!(
        field
            .visit_row(0, -2, 262, 0, &ok, |_, _| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut emitted = 0;
    assert_eq!(
        field
            .visit_row(0, -2, 262, 1024, &ok, |_, _| {
                emitted += 1;
                Err(Error::new(ErrorCode::OperationCancelled, "stop output"))
            })
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(emitted, 1);
    assert_eq!(budget.usage().reserved, before);
    assert_rows(&field, domain);
    let inverse = field.inverted();
    assert_eq!(inverse.coverage(1, 0), 0);
    drop(field);
    assert_eq!(budget.usage().reserved, before);
    drop(inverse);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn huge_edge_rationals_and_cancellation_during_sort_stay_bounded() {
    let budget = SharedBudget::new(8 << 20);
    let m = COORD_ABS_MAX;
    let domain = Rect::new(m - 16, m - 16, 16, 16).unwrap();
    let vertices = vec![(-m, -m), (m, m), (m, -m), (-m, m)];
    let field =
        BinaryMask::polygon(domain, &vertices, &budget, MaskLimits::default(), &ok).unwrap();
    for y in domain.y0..domain.y1 {
        for x in domain.x0..domain.x1 {
            assert_eq!(field.coverage(x, y) == 255, inside(&vertices, x, y));
        }
    }
    drop(field);
    assert_eq!(budget.usage().reserved, 0);
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() > 30 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel sorted intersections",
            ))
        } else {
            Ok(())
        }
    };
    let zigzag: Vec<_> = (0..128)
        .map(|i| (i, if i % 2 == 0 { -10 } else { 10 }))
        .collect();
    assert_eq!(
        BinaryMask::polygon(
            Rect::new(0, 0, 128, 10).unwrap(),
            &zigzag,
            &budget,
            MaskLimits::default(),
            &check
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert!(budget.usage().peak > 0);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn empty_and_outside_polygons_need_no_retained_or_scratch_capacity() {
    let domain = Rect::new(0, 0, 8, 8).unwrap();
    let budget = SharedBudget::new(0);
    for vertices in [
        vec![],
        vec![(1, 1)],
        vec![(0, 0), (2, 0), (8, 0)],
        vec![(-8, -8), (-4, 2), (-2, 7)],
        vec![(8, 0), (10, 2), (11, 8)],
    ] {
        let field =
            BinaryMask::polygon(domain, &vertices, &budget, MaskLimits::default(), &ok).unwrap();
        assert_eq!(field.uniform_coverage(), Some(0));
        assert_eq!(budget.usage().peak, 0);
    }
}

use super::*;
use paintcraft_commands::Mode;
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};

fn batch_of(operations: Vec<Op>) -> Batch {
    Batch {
        version: 1,
        policy: POLICY.into(),
        base_state: uuid::Uuid::nil(),
        base_epoch: 0,
        seed: None,
        operations,
        digest: "0".repeat(64),
    }
}

fn add(alias: &str, w: u32, h: u32) -> Op {
    Op::LayerAdd {
        alias: alias.into(),
        name: alias.into(),
        width: w,
        height: h,
        origin: (0, 0),
        default: Rgba::TRANSPARENT,
    }
}

fn pencil(alias: &str, points: Vec<(i64, i64)>) -> Op {
    Op::PencilPath {
        layer: LayerRef::Alias(alias.into()),
        points,
        color: Rgba([1, 2, 3, 255]),
        mode: Mode::Replace,
    }
}

#[test]
fn pencil_segment_budget_counts_zero_length_segments() {
    let doc = new_document(2, 2, None, 0, POLICY).unwrap();
    // 17 × 65,535 segments exceeds 1,048,576 while emitting a single pixel each time.
    let mut ops = vec![add("a", 2, 2)];
    ops.extend((0..17).map(|_| pencil("a", vec![(0, 0); 65_536])));
    let e = execute(&doc, Selection::Inactive, &batch_of(ops.clone())).unwrap_err();
    assert_eq!(e.code, ErrorCode::ResourceLimit);
    assert_eq!(
        e.operation_index,
        Some(17),
        "the first operation that crosses the limit"
    );
    // Sixteen of them stay within budget.
    ops.truncate(17);
    assert!(execute(&doc, Selection::Inactive, &batch_of(ops)).is_ok());
}

#[test]
fn pencil_pixel_budget_is_checked_before_expansion() {
    let doc = new_document(2, 2, None, 0, POLICY).unwrap();
    let far = vec![(0, 0), (PENCIL_PIXELS_MAX as i64, 0)];
    let e = execute(
        &doc,
        Selection::Inactive,
        &batch_of(vec![add("a", 2, 2), pencil("a", far)]),
    )
    .unwrap_err();
    assert_eq!(
        (e.code, e.operation_index),
        (ErrorCode::ResourceLimit, Some(1))
    );
    // Exactly the limit is admitted: 1 + (limit - 1) pixels.
    let edge = vec![(0, 0), (PENCIL_PIXELS_MAX as i64 - 1, 0)];
    assert!(
        execute(
            &doc,
            Selection::Inactive,
            &batch_of(vec![add("a", 2, 2), pencil("a", edge)])
        )
        .is_ok()
    );
}

#[test]
fn exhausted_staging_aborts_with_a_resource_error() {
    let doc = new_document(512, 512, None, 0, POLICY).unwrap();
    let fill = Op::FillRect {
        layer: LayerRef::Alias("a".into()),
        rect: Rect::new(0, 0, 512, 512).unwrap(),
        color: Rgba([9, 9, 9, 255]),
        mode: Mode::Replace,
    };
    let batch = batch_of(vec![add("a", 512, 512), fill]);
    let e = execute_with_budget(&doc, Selection::Inactive, &batch, 65_536 * 3).unwrap_err();
    assert_eq!(
        (e.code, e.operation_index),
        (ErrorCode::ResourceLimit, Some(1))
    );
    assert!(execute_with_budget(&doc, Selection::Inactive, &batch, 65_536 * 4).is_ok());
}

fn full_walk(a: (i64, i64), b: (i64, i64)) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    walk_clipped(
        a,
        b,
        Rect::new(-1000, -1000, 2000, 2000),
        false,
        &|| Ok(()),
        |x, y| {
            out.push((x, y));
            Ok(())
        },
    )
    .unwrap();
    out
}

#[test]
fn bresenham_covers_endpoints_in_every_octant() {
    for end in [
        (6, 4),
        (5, 6),
        (2, 6),
        (0, 5),
        (0, 2),
        (1, 0),
        (4, 0),
        (6, 1),
        (3, 3),
    ] {
        let w = full_walk((3, 3), end);
        assert_eq!(w.first(), Some(&(3, 3)));
        assert_eq!(w.last(), Some(&end));
        assert_eq!(w.len() as i64, 1 + (end.0 - 3).abs().max((end.1 - 3).abs()));
    }
    assert_eq!(full_walk((2, 2), (2, 2)), [(2, 2)]);
}

#[test]
fn clipped_walk_equals_the_filtered_full_walk() {
    // Deterministic LCG; avoids a dev-dependency for randomness.
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    let mut next = |m: i64| {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((s >> 33) as i64 % (2 * m + 1)) - m
    };
    for _ in 0..5000 {
        let (a, b) = ((next(15), next(15)), (next(15), next(15)));
        let c0 = (next(8), next(8));
        let clip = Rect::new(c0.0, c0.1, 1 + next(5).abs() + 3, 1 + next(5).abs() + 3).unwrap();
        for skip in [false, true] {
            let want: Vec<_> = full_walk(a, b)
                .into_iter()
                .skip(usize::from(skip))
                .filter(|&(x, y)| clip.contains(x, y))
                .collect();
            let mut got = Vec::new();
            walk_clipped(a, b, Some(clip), skip, &|| Ok(()), |x, y| {
                got.push((x, y));
                Ok(())
            })
            .unwrap();
            assert_eq!(got, want, "{a:?}->{b:?} clip {clip:?} skip {skip}");
        }
    }
}

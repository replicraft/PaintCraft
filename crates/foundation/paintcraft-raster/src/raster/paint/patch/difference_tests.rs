//! Literal rectangle subtraction order and finite exact union; no renderer.
use super::*;
fn rect(v: [i64; 4]) -> Rect {
    Rect {
        x0: v[0],
        y0: v[1],
        x1: v[2],
        y1: v[3],
    }
}
#[test]
fn fixed_difference_preserves_literal_order_and_exact_disjoint_union() {
    type Case = ([i64; 4], [i64; 4], &'static [[i64; 4]]);
    let cases: &[Case] = &[
        ([0, 0, 8, 6], [10, 2, 12, 4], &[[0, 0, 8, 6]]), // disjoint
        (
            [0, 0, 8, 6],
            [2, 1, 5, 4],
            &[[0, 0, 8, 1], [0, 4, 8, 6], [0, 1, 2, 4], [5, 1, 8, 4]],
        ), // contained
        ([0, 0, 8, 6], [8, 1, 11, 3], &[[0, 0, 8, 6]]),  // touching_edge
        ([0, 0, 8, 6], [-2, -1, 10, 7], &[]),            // covering
        ([0, 0, 8, 6], [0, 0, 8, 6], &[]),               // equal
        (
            [-3, -2, 5, 4],
            [-1, 0, 2, 3],
            &[[-3, -2, 5, 0], [-3, 3, 5, 4], [-3, 0, -1, 3], [2, 0, 5, 3]],
        ), // negative_coordinates
        ([0, 0, 8, 6], [-2, -1, 3, 2], &[[0, 2, 8, 6], [3, 0, 8, 2]]), // partial_left_top
        ([0, 0, 8, 6], [8, 6, 10, 8], &[[0, 0, 8, 6]]),  // touching_corner
    ];
    for &(outer, inner, expected) in cases {
        let outer = rect(outer);
        let inner = rect(inner);
        let actual: Vec<Rect> = difference(outer, inner).collect();
        let expected: Vec<Rect> = expected.iter().copied().map(rect).collect();
        assert_eq!(actual, expected, "outer={outer:?},inner={inner:?}");
        assert!(actual.len() <= 4);
        for piece in &actual {
            assert!(piece.width() > 0 && piece.height() > 0);
        }
        // Finite points cover every literal case plus an exterior margin.
        // Each outer-minus-inner point is covered ONCE, all others zero.
        for y in -4..10 {
            for x in -5..14 {
                let count = actual.iter().filter(|r| r.contains(x, y)).count();
                assert_eq!(
                    count,
                    usize::from(outer.contains(x, y) && !inner.contains(x, y)),
                    "point=({x},{y}),outer={outer:?},inner={inner:?}"
                );
            }
        }
    }
}

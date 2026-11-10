//! Independent Fraction model literals frozen before implementation.
use super::*;
#[test]
fn exact_inverse_centers_and_expanded_bounds() {
    // identity
    {
        let g = StretchSkew::new(Rect::new(0, 0, 2, 2).unwrap(), (2, 2), (0, 0)).unwrap();
        assert_eq!(
            g.bounds(Rect::new(0, 0, 2, 2).unwrap()).unwrap(),
            Rect {
                x0: 0,
                y0: 0,
                x1: 2,
                y1: 2
            }
        );
        assert_eq!(g.source_pixel(0, 0).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(1, 0).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(0, 1).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(1, 1).unwrap(), (1, 1));
    }
    // double
    {
        let g = StretchSkew::new(Rect::new(0, 0, 2, 2).unwrap(), (4, 4), (0, 0)).unwrap();
        assert_eq!(
            g.bounds(Rect::new(0, 0, 2, 2).unwrap()).unwrap(),
            Rect {
                x0: 0,
                y0: 0,
                x1: 4,
                y1: 4
            }
        );
        assert_eq!(g.source_pixel(0, 0).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(1, 0).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(2, 0).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(3, 0).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(0, 1).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(1, 1).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(2, 1).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(3, 1).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(0, 2).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(1, 2).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(2, 2).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(3, 2).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(0, 3).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(1, 3).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(2, 3).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(3, 3).unwrap(), (1, 1));
    }
    // halve
    {
        let g = StretchSkew::new(Rect::new(0, 0, 4, 4).unwrap(), (2, 2), (0, 0)).unwrap();
        assert_eq!(
            g.bounds(Rect::new(0, 0, 4, 4).unwrap()).unwrap(),
            Rect {
                x0: 0,
                y0: 0,
                x1: 2,
                y1: 2
            }
        );
        assert_eq!(g.source_pixel(0, 0).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(1, 0).unwrap(), (3, 1));
        assert_eq!(g.source_pixel(0, 1).unwrap(), (1, 3));
        assert_eq!(g.source_pixel(1, 1).unwrap(), (3, 3));
    }
    // horizontal45
    {
        let g = StretchSkew::new(Rect::new(0, 0, 2, 2).unwrap(), (2, 2), (65536, 0)).unwrap();
        assert_eq!(
            g.bounds(Rect::new(0, 0, 2, 2).unwrap()).unwrap(),
            Rect {
                x0: 0,
                y0: 0,
                x1: 4,
                y1: 2
            }
        );
        assert_eq!(g.source_pixel(0, 0).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(1, 0).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(2, 0).unwrap(), (2, 0));
        assert_eq!(g.source_pixel(3, 0).unwrap(), (3, 0));
        assert_eq!(g.source_pixel(0, 1).unwrap(), (-1, 1));
        assert_eq!(g.source_pixel(1, 1).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(2, 1).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(3, 1).unwrap(), (2, 1));
    }
    // negative45
    {
        let g = StretchSkew::new(Rect::new(0, 0, 2, 2).unwrap(), (2, 2), (-65536, 0)).unwrap();
        assert_eq!(
            g.bounds(Rect::new(0, 0, 2, 2).unwrap()).unwrap(),
            Rect {
                x0: 0,
                y0: 0,
                x1: 4,
                y1: 2
            }
        );
        assert_eq!(g.source_pixel(0, 0).unwrap(), (-1, 0));
        assert_eq!(g.source_pixel(1, 0).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(2, 0).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(3, 0).unwrap(), (2, 0));
        assert_eq!(g.source_pixel(0, 1).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(1, 1).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(2, 1).unwrap(), (2, 1));
        assert_eq!(g.source_pixel(3, 1).unwrap(), (3, 1));
    }
    // both45
    {
        let g = StretchSkew::new(Rect::new(0, 0, 2, 2).unwrap(), (2, 2), (65536, 65536)).unwrap();
        assert_eq!(
            g.bounds(Rect::new(0, 0, 2, 2).unwrap()).unwrap(),
            Rect {
                x0: 0,
                y0: 0,
                x1: 4,
                y1: 6
            }
        );
        assert_eq!(g.source_pixel(0, 0).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(1, 0).unwrap(), (2, -1));
        assert_eq!(g.source_pixel(2, 0).unwrap(), (4, -2));
        assert_eq!(g.source_pixel(3, 0).unwrap(), (6, -3));
        assert_eq!(g.source_pixel(0, 1).unwrap(), (-1, 1));
        assert_eq!(g.source_pixel(1, 1).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(2, 1).unwrap(), (3, -1));
        assert_eq!(g.source_pixel(3, 1).unwrap(), (5, -2));
        assert_eq!(g.source_pixel(0, 2).unwrap(), (-2, 2));
        assert_eq!(g.source_pixel(1, 2).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(2, 2).unwrap(), (2, 0));
        assert_eq!(g.source_pixel(3, 2).unwrap(), (4, -1));
        assert_eq!(g.source_pixel(0, 3).unwrap(), (-3, 3));
        assert_eq!(g.source_pixel(1, 3).unwrap(), (-1, 2));
        assert_eq!(g.source_pixel(2, 3).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(3, 3).unwrap(), (3, 0));
        assert_eq!(g.source_pixel(0, 4).unwrap(), (-4, 4));
        assert_eq!(g.source_pixel(1, 4).unwrap(), (-2, 3));
        assert_eq!(g.source_pixel(2, 4).unwrap(), (0, 2));
        assert_eq!(g.source_pixel(3, 4).unwrap(), (2, 1));
        assert_eq!(g.source_pixel(0, 5).unwrap(), (-5, 5));
        assert_eq!(g.source_pixel(1, 5).unwrap(), (-3, 4));
        assert_eq!(g.source_pixel(2, 5).unwrap(), (-1, 3));
        assert_eq!(g.source_pixel(3, 5).unwrap(), (1, 2));
    }
    // offcanvas
    {
        let g = StretchSkew::new(Rect::new(0, 0, 2, 2).unwrap(), (4, 4), (0, 0)).unwrap();
        assert_eq!(
            g.bounds(Rect::new(-1, -1, 4, 4).unwrap()).unwrap(),
            Rect {
                x0: -2,
                y0: -2,
                x1: 6,
                y1: 6
            }
        );
        assert_eq!(g.source_pixel(-2, -2).unwrap(), (-1, -1));
        assert_eq!(g.source_pixel(-1, -2).unwrap(), (-1, -1));
        assert_eq!(g.source_pixel(0, -2).unwrap(), (0, -1));
        assert_eq!(g.source_pixel(1, -2).unwrap(), (0, -1));
        assert_eq!(g.source_pixel(2, -2).unwrap(), (1, -1));
        assert_eq!(g.source_pixel(3, -2).unwrap(), (1, -1));
        assert_eq!(g.source_pixel(4, -2).unwrap(), (2, -1));
        assert_eq!(g.source_pixel(5, -2).unwrap(), (2, -1));
        assert_eq!(g.source_pixel(-2, -1).unwrap(), (-1, -1));
        assert_eq!(g.source_pixel(-1, -1).unwrap(), (-1, -1));
        assert_eq!(g.source_pixel(0, -1).unwrap(), (0, -1));
        assert_eq!(g.source_pixel(1, -1).unwrap(), (0, -1));
        assert_eq!(g.source_pixel(2, -1).unwrap(), (1, -1));
        assert_eq!(g.source_pixel(3, -1).unwrap(), (1, -1));
        assert_eq!(g.source_pixel(4, -1).unwrap(), (2, -1));
        assert_eq!(g.source_pixel(5, -1).unwrap(), (2, -1));
        assert_eq!(g.source_pixel(-2, 0).unwrap(), (-1, 0));
        assert_eq!(g.source_pixel(-1, 0).unwrap(), (-1, 0));
        assert_eq!(g.source_pixel(0, 0).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(1, 0).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(2, 0).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(3, 0).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(4, 0).unwrap(), (2, 0));
        assert_eq!(g.source_pixel(5, 0).unwrap(), (2, 0));
        assert_eq!(g.source_pixel(-2, 1).unwrap(), (-1, 0));
        assert_eq!(g.source_pixel(-1, 1).unwrap(), (-1, 0));
        assert_eq!(g.source_pixel(0, 1).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(1, 1).unwrap(), (0, 0));
        assert_eq!(g.source_pixel(2, 1).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(3, 1).unwrap(), (1, 0));
        assert_eq!(g.source_pixel(4, 1).unwrap(), (2, 0));
        assert_eq!(g.source_pixel(5, 1).unwrap(), (2, 0));
        assert_eq!(g.source_pixel(-2, 2).unwrap(), (-1, 1));
        assert_eq!(g.source_pixel(-1, 2).unwrap(), (-1, 1));
        assert_eq!(g.source_pixel(0, 2).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(1, 2).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(2, 2).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(3, 2).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(4, 2).unwrap(), (2, 1));
        assert_eq!(g.source_pixel(5, 2).unwrap(), (2, 1));
        assert_eq!(g.source_pixel(-2, 3).unwrap(), (-1, 1));
        assert_eq!(g.source_pixel(-1, 3).unwrap(), (-1, 1));
        assert_eq!(g.source_pixel(0, 3).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(1, 3).unwrap(), (0, 1));
        assert_eq!(g.source_pixel(2, 3).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(3, 3).unwrap(), (1, 1));
        assert_eq!(g.source_pixel(4, 3).unwrap(), (2, 1));
        assert_eq!(g.source_pixel(5, 3).unwrap(), (2, 1));
        assert_eq!(g.source_pixel(-2, 4).unwrap(), (-1, 2));
        assert_eq!(g.source_pixel(-1, 4).unwrap(), (-1, 2));
        assert_eq!(g.source_pixel(0, 4).unwrap(), (0, 2));
        assert_eq!(g.source_pixel(1, 4).unwrap(), (0, 2));
        assert_eq!(g.source_pixel(2, 4).unwrap(), (1, 2));
        assert_eq!(g.source_pixel(3, 4).unwrap(), (1, 2));
        assert_eq!(g.source_pixel(4, 4).unwrap(), (2, 2));
        assert_eq!(g.source_pixel(5, 4).unwrap(), (2, 2));
        assert_eq!(g.source_pixel(-2, 5).unwrap(), (-1, 2));
        assert_eq!(g.source_pixel(-1, 5).unwrap(), (-1, 2));
        assert_eq!(g.source_pixel(0, 5).unwrap(), (0, 2));
        assert_eq!(g.source_pixel(1, 5).unwrap(), (0, 2));
        assert_eq!(g.source_pixel(2, 5).unwrap(), (1, 2));
        assert_eq!(g.source_pixel(3, 5).unwrap(), (1, 2));
        assert_eq!(g.source_pixel(4, 5).unwrap(), (2, 2));
        assert_eq!(g.source_pixel(5, 5).unwrap(), (2, 2));
    }
    // selection-anchor
    {
        let g = StretchSkew::new(Rect::new(3, 4, 2, 2).unwrap(), (4, 2), (0, 0)).unwrap();
        assert_eq!(
            g.bounds(Rect::new(3, 4, 2, 2).unwrap()).unwrap(),
            Rect {
                x0: 3,
                y0: 4,
                x1: 7,
                y1: 6
            }
        );
        assert_eq!(g.source_pixel(3, 4).unwrap(), (3, 4));
        assert_eq!(g.source_pixel(4, 4).unwrap(), (3, 4));
        assert_eq!(g.source_pixel(5, 4).unwrap(), (4, 4));
        assert_eq!(g.source_pixel(6, 4).unwrap(), (4, 4));
        assert_eq!(g.source_pixel(3, 5).unwrap(), (3, 5));
        assert_eq!(g.source_pixel(4, 5).unwrap(), (3, 5));
        assert_eq!(g.source_pixel(5, 5).unwrap(), (4, 5));
        assert_eq!(g.source_pixel(6, 5).unwrap(), (4, 5));
    }
}
#[test]
fn invalid_extents_and_shear_refuse_without_overflow() {
    let r = Rect::new(0, 0, 2, 2).unwrap();
    assert!(StretchSkew::new(r, (0, 2), (0, 0)).is_err());
    assert!(StretchSkew::new(r, (2, 2), (SHEAR_MAX + 1, 0)).is_err());
    assert!(
        StretchSkew::new(
            Rect {
                x0: i64::MIN,
                x1: i64::MAX,
                y0: 0,
                y1: 1
            },
            (1, 1),
            (0, 0)
        )
        .is_err()
    );
    assert!(
        StretchSkew::new(
            r,
            (CANVAS_SIDE_MAX, CANVAS_SIDE_MAX),
            (SHEAR_MAX, SHEAR_MAX)
        )
        .is_err()
    );
}

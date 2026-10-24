use paintcraft_core::types::{Error, ErrorCode, Rect, policy::COORD_ABS_MAX, shapes::ShapePreset};
use paintcraft_raster::shape::{presets::contour, visit_polygon};
use std::cell::Cell;

// Independent per-pixel ray crossing, with no scanline rounding or span pairing.
fn inside(points: &[(i64, i64)], x: i64, y: i64) -> bool {
    let (px, py) = (i128::from(2 * x + 1), i128::from(2 * y + 1));
    let mut odd = false;
    for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
        let (a, b) = if a.1 <= b.1 { (a, b) } else { (b, a) };
        let (ax, ay, bx, by) = (
            i128::from(2 * a.0),
            i128::from(2 * a.1),
            i128::from(2 * b.0),
            i128::from(2 * b.1),
        );
        if ay <= py && py < by && (bx - ax) * (py - ay) > (px - ax) * (by - ay) {
            odd = !odd;
        }
    }
    odd
}

#[test]
fn every_preset_matches_independent_pixel_membership_and_never_blends_twice() {
    let clip = Rect::new(-8, -8, 48, 48).unwrap();
    for shape in ShapePreset::ALL {
        for (w, h) in [(3, 3), (31, 27), (11, 34), (34, 11)] {
            let bbox = Rect::new(-3, -5, w, h).unwrap();
            let vertices = contour(shape, &bbox).unwrap();
            assert!(vertices.len() <= 257 && vertices.first() == vertices.last());
            assert!(vertices.iter().all(|p| bbox.contains(p.0, p.1)));
            let mut pixels = vec![0_u8; 48 * 48];
            visit_polygon(&vertices, &clip, &|| Ok(()), |span| {
                assert!(clip.contains(span.x0, span.y) && clip.contains(span.x1, span.y));
                for x in span.x0..=span.x1 {
                    pixels[((span.y + 8) * 48 + x + 8) as usize] += 1;
                }
                Ok(())
            })
            .unwrap();
            for y in clip.y0..clip.y1 {
                for x in clip.x0..clip.x1 {
                    assert_eq!(
                        pixels[((y + 8) * 48 + x + 8) as usize],
                        u8::from(inside(&vertices, x, y)),
                        "{shape:?} {w}x{h} at {x},{y}"
                    );
                }
            }
        }
    }
}

#[test]
fn far_clipped_polygons_stream_only_visible_rows_and_support_cancellation() {
    let vertices = [
        (-COORD_ABS_MAX, -COORD_ABS_MAX),
        (COORD_ABS_MAX, -COORD_ABS_MAX),
        (COORD_ABS_MAX, COORD_ABS_MAX),
        (-COORD_ABS_MAX, COORD_ABS_MAX),
    ];
    let clip = Rect::new(0, 0, 8, 8).unwrap();
    let mut rows = 0;
    visit_polygon(&vertices, &clip, &|| Ok(()), |span| {
        assert_eq!((span.x0, span.x1), (0, 7));
        rows += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(rows, 8);
    let calls = Cell::new(0);
    let error = visit_polygon(
        &vertices,
        &clip,
        &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 5 {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel polygon"))
            } else {
                Ok(())
            }
        },
        |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    let too_many_edges: Vec<_> = [(0, 0), (1, 0), (1, 1_048_576), (0, 1_048_576)]
        .into_iter()
        .cycle()
        .take(1024)
        .collect();
    let error = visit_polygon(
        &too_many_edges,
        &Rect::new(0, 0, 1, 1_048_576).unwrap(),
        &|| Ok(()),
        |_| panic!("unadmitted polygon must not emit pixels"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
}

//! Bounded, deterministic contours for drag-to-size closed shapes.
//! Curved contours are fixed-point cubic flattenings, not native Paint oracles.
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, shapes::ShapePreset};

type Point = (i64, i64);

/// Closed pixel-space contour, at most 257 vertices. Reversing drag direction
/// changes the bounding box only; directional arrows retain their named direction.
pub fn contour(preset: ShapePreset, bbox: &Rect) -> Result<Vec<Point>> {
    let w = bbox.x1.checked_sub(bbox.x0).filter(|n| *n > 0);
    let h = bbox.y1.checked_sub(bbox.y0).filter(|n| *n > 0);
    let (w, h) = w
        .zip(h)
        .ok_or_else(|| Error::new(ErrorCode::InputSchema, "invalid shape box"))?;
    let mut points = Vec::new();
    points
        .try_reserve_exact(257)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot reserve shape contour"))?;
    normalized(preset, &mut points);
    for (x, y) in &mut points {
        *x = bbox.x0 + (i128::from(*x) * i128::from(w - 1) / 1024) as i64;
        *y = bbox.y0 + (i128::from(*y) * i128::from(h - 1) / 1024) as i64;
    }
    points.dedup();
    if points.last() != points.first() {
        points.push(points[0]);
    }
    Ok(points)
}

fn cubic(out: &mut Vec<Point>, a: Point, b: Point, c: Point, d: Point) {
    for i in 1..=32_i64 {
        let j = 32 - i;
        let coordinate = |a, b, c, d| {
            (j * j * j * a + 3 * j * j * i * b + 3 * j * i * i * c + i * i * i * d + 16384) / 32768
        };
        out.push((
            coordinate(a.0, b.0, c.0, d.0),
            coordinate(a.1, b.1, c.1, d.1),
        ));
    }
}

fn normalized(shape: ShapePreset, out: &mut Vec<Point>) {
    use ShapePreset::*;
    let polygon: &[Point] = match shape {
        Triangle => &[(512, 0), (1024, 1024), (0, 1024)],
        RightTriangle => &[(0, 0), (1024, 1024), (0, 1024)],
        Diamond => &[(512, 0), (1024, 512), (512, 1024), (0, 512)],
        Pentagon => &[(512, 0), (1024, 391), (828, 1024), (196, 1024), (0, 391)],
        Hexagon => &[
            (256, 0),
            (768, 0),
            (1024, 512),
            (768, 1024),
            (256, 1024),
            (0, 512),
        ],
        RightArrow | LeftArrow | UpArrow | DownArrow => &[
            (0, 307),
            (614, 307),
            (614, 0),
            (1024, 512),
            (614, 1024),
            (614, 717),
            (0, 717),
        ],
        FourPointStar => &[
            (512, 0),
            (640, 384),
            (1024, 512),
            (640, 640),
            (512, 1024),
            (384, 640),
            (0, 512),
            (384, 384),
        ],
        FivePointStar => &[
            (512, 0),
            (633, 375),
            (1024, 375),
            (708, 607),
            (829, 1024),
            (512, 768),
            (195, 1024),
            (316, 607),
            (0, 375),
            (391, 375),
        ],
        SixPointStar => &[
            (512, 0),
            (660, 256),
            (956, 256),
            (808, 512),
            (956, 768),
            (660, 768),
            (512, 1024),
            (364, 768),
            (68, 768),
            (216, 512),
            (68, 256),
            (364, 256),
        ],
        Lightning => &[
            (512, 0),
            (1024, 0),
            (666, 410),
            (922, 410),
            (205, 1024),
            (410, 563),
            (102, 563),
        ],
        RectangularCallout => &[
            (0, 0),
            (1024, 0),
            (1024, 768),
            (563, 768),
            (205, 1024),
            (307, 768),
            (0, 768),
        ],
        _ => &[],
    };
    out.extend_from_slice(polygon);
    if matches!(shape, LeftArrow | UpArrow | DownArrow) {
        for p in out.iter_mut() {
            *p = match shape {
                LeftArrow => (1024 - p.0, p.1),
                UpArrow => (p.1, 1024 - p.0),
                _ => (p.1, p.0),
            };
        }
    }
    match shape {
        RoundedRectangle => {
            out.push((256, 0));
            out.push((768, 0));
            cubic(out, (768, 0), (909, 0), (1024, 115), (1024, 256));
            out.push((1024, 768));
            cubic(out, (1024, 768), (1024, 909), (909, 1024), (768, 1024));
            out.push((256, 1024));
            cubic(out, (256, 1024), (115, 1024), (0, 909), (0, 768));
            out.push((0, 256));
            cubic(out, (0, 256), (0, 115), (115, 0), (256, 0));
        }
        Heart => {
            out.push((512, 256));
            cubic(out, (512, 256), (154, -256), (-256, 256), (205, 666));
            cubic(out, (205, 666), (307, 768), (410, 922), (512, 1024));
            cubic(out, (512, 1024), (614, 922), (717, 768), (819, 666));
            cubic(out, (819, 666), (1280, 256), (870, -256), (512, 256));
        }
        OvalCallout => {
            out.push((512, 0));
            cubic(out, (512, 0), (795, 0), (1024, 172), (1024, 384));
            cubic(out, (1024, 384), (1024, 596), (795, 768), (563, 768));
            out.extend_from_slice(&[(205, 1024), (307, 737)]);
            cubic(out, (307, 737), (-154, 666), (-154, 0), (512, 0));
        }
        CloudCallout => {
            out.push((205, 154));
            cubic(out, (205, 154), (205, -51), (563, -51), (614, 102));
            cubic(out, (614, 102), (870, -51), (1126, 256), (922, 410));
            cubic(out, (922, 410), (1178, 717), (768, 922), (614, 717));
            cubic(out, (614, 717), (410, 922), (205, 922), (205, 717));
            out.extend_from_slice(&[(102, 1024), (307, 819), (205, 717)]);
            cubic(out, (205, 717), (-102, 768), (-51, 461), (102, 410));
            cubic(out, (102, 410), (-102, 307), (0, 102), (205, 154));
        }
        _ => {}
    }
    // Cubic control points can lie outside the template; samples are bounded to
    // the drag box, including tiny rounded/heart shapes.
    for p in out {
        p.0 = p.0.clamp(0, 1024);
        p.1 = p.1.clamp(0, 1024);
    }
}

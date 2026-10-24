//! Checked composition of exact destination-to-source coordinate maps.
use super::*;
use paintcraft_core::types::orthogonal::OrthogonalTransform;
#[derive(Clone, Copy, Debug)]
pub(super) struct Coordinates {
    matrix: [i64; 4],
    offset: (i64, i64),
}
impl Coordinates {
    pub(super) const IDENTITY: Self = Self {
        matrix: [1, 0, 0, 1],
        offset: (0, 0),
    };
    pub(super) fn pixel(self, x: i64, y: i64) -> Result<(i64, i64)> {
        if self.matrix == [1, 0, 0, 1] {
            return Ok((
                x.checked_add(self.offset.0)
                    .ok_or_else(metadata::exhausted)?,
                y.checked_add(self.offset.1)
                    .ok_or_else(metadata::exhausted)?,
            ));
        }
        let [a, b, c, d] = self.matrix;
        let row = |a: i64, b: i64, offset: i64| {
            x.checked_mul(a)
                .and_then(|n| y.checked_mul(b).and_then(|m| n.checked_add(m)))
                .and_then(|n| n.checked_add(offset))
                .ok_or_else(metadata::exhausted)
        };
        Ok((row(a, b, self.offset.0)?, row(c, d, self.offset.1)?))
    }
    /// Apply `next` after this mapping.
    pub(super) fn then(self, next: Self) -> Result<Self> {
        let [a, b, c, d] = next.matrix;
        let [e, f, g, h] = self.matrix;
        // Both operands are signed permutation matrices; products stay in -1..=1.
        Ok(Self {
            matrix: [a * e + b * g, a * f + b * h, c * e + d * g, c * f + d * h],
            offset: next.pixel(self.offset.0, self.offset.1)?,
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Sampling {
    pub area: Rect,
    pub transform: Option<OrthogonalTransform>,
}
impl Sampling {
    pub fn dimensions(self) -> (u32, u32) {
        let (w, h) = (self.area.width() as u32, self.area.height() as u32);
        self.transform.map_or((w, h), |t| t.dimensions(w, h))
    }
    pub fn bounds(self) -> Rect {
        let (w, h) = self.dimensions();
        Rect::from_origin_size(0, 0, w, h).unwrap()
    }
    pub fn target_pixel(self, x: i64, y: i64) -> (i64, i64) {
        let (x, y) = (x - self.area.x0, y - self.area.y0);
        self.transform.map_or((x, y), |t| {
            t.pixel_checked(x, y, self.area.width() as u32, self.area.height() as u32)
                .expect("admitted source coordinate")
        })
    }
    pub fn coordinates(self) -> Coordinates {
        let (w, h) = self.dimensions();
        let point = |x, y| {
            self.transform.map_or((x, y), |t| {
                t.inverse()
                    .pixel_checked(x, y, w, h)
                    .expect("bounded raster basis")
            })
        };
        let p = point(0, 0);
        let x = point(1, 0);
        let y = point(0, 1);
        Coordinates {
            matrix: [x.0 - p.0, y.0 - p.0, x.1 - p.1, y.1 - p.1],
            offset: (p.0 + self.area.x0, p.1 + self.area.y0),
        }
    }
}

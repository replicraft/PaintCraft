//! Deterministic nearest-neighbor geometry: stretch, horizontal then vertical shear.
//! Edge bounds round outward; destination pixel centers map back with floor.
use super::{
    Error, ErrorCode, Rect, Result,
    policy::{CANVAS_SIDE_MAX, COORD_ABS_MAX},
};
const Q: i128 = 65_536;
/// Largest supported Q16 tangent, conservatively including 89 degrees.
pub const SHEAR_MAX: i32 = 3_754_555;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StretchSkew {
    source: Rect,
    target: (u32, u32),
    shear: (i32, i32),
    offset: (i64, i64),
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Stretch and skew exceeds geometry policy",
    )
}
fn valid(rect: Rect) -> Result<()> {
    if [rect.x0, rect.y0, rect.x1, rect.y1]
        .into_iter()
        .any(|n| n.unsigned_abs() > COORD_ABS_MAX as u64)
        || rect.x1 <= rect.x0
        || rect.y1 <= rect.y0
        || rect.width() > i64::from(CANVAS_SIDE_MAX)
        || rect.height() > i64::from(CANVAS_SIDE_MAX)
    {
        return Err(invalid());
    }
    Ok(())
}
fn product(values: &[i128]) -> Result<i128> {
    values
        .iter()
        .try_fold(1_i128, |a, &b| a.checked_mul(b).ok_or_else(invalid))
}
fn add(a: i128, b: i128) -> Result<i128> {
    a.checked_add(b).ok_or_else(invalid)
}
impl StretchSkew {
    pub fn new(source: Rect, target: (u32, u32), shear: (i32, i32)) -> Result<Self> {
        valid(source)?;
        if target.0 == 0
            || target.1 == 0
            || target.0 > CANVAS_SIDE_MAX
            || target.1 > CANVAS_SIDE_MAX
            || shear.0.unsigned_abs() > SHEAR_MAX as u32
            || shear.1.unsigned_abs() > SHEAR_MAX as u32
        {
            return Err(invalid());
        }
        let mut value = Self {
            source,
            target,
            shear,
            offset: (0, 0),
        };
        let raw = value.raw_bounds(source)?;
        value.offset = (raw.x0, raw.y0);
        value.bounds(source)?;
        Ok(value)
    }
    pub fn source(self) -> Rect {
        self.source
    }
    pub fn is_identity(self) -> bool {
        self.target == (self.source.width() as u32, self.source.height() as u32)
            && self.shear == (0, 0)
    }
    fn raw_bounds(self, rect: Rect) -> Result<Rect> {
        valid(rect)?;
        let (w, h) = (
            i128::from(self.source.width()),
            i128::from(self.source.height()),
        );
        let (tw, th) = (i128::from(self.target.0), i128::from(self.target.1));
        let (kx, ky) = (i128::from(self.shear.0), i128::from(self.shear.1));
        let denominator = w * h * Q * Q;
        let mut lo = (i128::MAX, i128::MAX);
        let mut hi = (i128::MIN, i128::MIN);
        for x in [rect.x0, rect.x1] {
            for y in [rect.y0, rect.y1] {
                // Distant offcanvas extents can overflow even when the canvas fits.
                // Refuse every intermediate product before rounding or allocation.
                let (x, y) = (
                    i128::from(x) - i128::from(self.source.x0),
                    i128::from(y) - i128::from(self.source.y0),
                );
                let xn = add(product(&[x, tw, h, Q])?, product(&[kx, y, th, w])?)?;
                let yn = add(product(&[y, th, w, Q, Q])?, product(&[ky, xn])?)?;
                let (xn, yn) = (product(&[xn, Q])?, yn);
                lo.0 = lo.0.min(xn.div_euclid(denominator));
                lo.1 = lo.1.min(yn.div_euclid(denominator));
                let ceil = |n: i128| {
                    n.div_euclid(denominator) + i128::from(n.rem_euclid(denominator) != 0)
                };
                hi.0 = hi.0.max(ceil(xn));
                hi.1 = hi.1.max(ceil(yn));
            }
        }
        let c = |n| i64::try_from(n).map_err(|_| invalid());
        Ok(Rect {
            x0: c(lo.0)?,
            y0: c(lo.1)?,
            x1: c(hi.0)?,
            y1: c(hi.1)?,
        })
    }
    /// Transform an arbitrary finite source extent, retaining offcanvas content.
    pub fn bounds(self, rect: Rect) -> Result<Rect> {
        let raw = self.raw_bounds(rect)?;
        let dx = self
            .source
            .x0
            .checked_sub(self.offset.0)
            .ok_or_else(invalid)?;
        let dy = self
            .source
            .y0
            .checked_sub(self.offset.1)
            .ok_or_else(invalid)?;
        let out = raw.translated(dx, dy).ok_or_else(invalid)?;
        valid(out)?;
        Ok(out)
    }
    /// Inverse-map one destination center. Outside the finite source is padding.
    pub fn source_pixel(self, x: i64, y: i64) -> Result<(i64, i64)> {
        if [x, y]
            .into_iter()
            .any(|v| v.unsigned_abs() > COORD_ABS_MAX as u64)
        {
            return Err(invalid());
        }
        let x = 2 * (i128::from(x) - i128::from(self.source.x0) + i128::from(self.offset.0)) + 1;
        let y = 2 * (i128::from(y) - i128::from(self.source.y0) + i128::from(self.offset.1)) + 1;
        let v = product(&[y, Q])?
            .checked_sub(product(&[i128::from(self.shear.1), x])?)
            .ok_or_else(invalid)?;
        let u = product(&[x, Q, Q])?
            .checked_sub(product(&[i128::from(self.shear.0), v])?)
            .ok_or_else(invalid)?;
        let sx = product(&[u, i128::from(self.source.width())])?
            .div_euclid(2 * Q * Q * i128::from(self.target.0))
            + i128::from(self.source.x0);
        let sy = product(&[v, i128::from(self.source.height())])?
            .div_euclid(2 * Q * i128::from(self.target.1))
            + i128::from(self.source.y0);
        Ok((
            i64::try_from(sx).map_err(|_| invalid())?,
            i64::try_from(sy).map_err(|_| invalid())?,
        ))
    }
}

#[cfg(test)]
mod tests;

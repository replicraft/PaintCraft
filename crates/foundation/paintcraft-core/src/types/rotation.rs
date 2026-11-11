//! Pure stored-coefficient rotation. No pixel allocation or execution authority.
//! Normalize exact rational source minimum before integer bounds, retaining the
//! existing selected-transform top-left anchor and exact cardinal orientations.
use super::{
    Error, ErrorCode, Rect, Result,
    orthogonal::OrthogonalTransform,
    policy::{CANVAS_SIDE_MAX, COORD_ABS_MAX},
};
const Q: i128 = 1 << 30;
const D: i128 = 2 * Q;
/// Fixed denominator for persisted command coefficients, never document fields.
pub const COEFFICIENT_ONE: i32 = 1 << 30;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rotation {
    source: Rect,
    coefficients: (i32, i32),
    min_relative: (i64, i64),
}
fn resource() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Rotation exceeds geometry policy")
}
fn coefficients() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "Rotation coefficients are not a supported near-unit Q30 pair",
    )
}
fn add(a: i128, b: i128) -> Result<i128> {
    a.checked_add(b).ok_or_else(resource)
}
fn sub(a: i128, b: i128) -> Result<i128> {
    a.checked_sub(b).ok_or_else(resource)
}
fn mul(a: i128, b: i128) -> Result<i128> {
    a.checked_mul(b).ok_or_else(resource)
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
        return Err(resource());
    }
    Ok(())
}
impl Rotation {
    /// Coefficients are (cosine,sine) in Q30; positive sine rotates clockwise in
    /// document coordinates. Nearest rounding of true unit components fits the
    /// closed tolerance |c²+s²−Q²|<=2Q. The actual determinant is inverted below.
    /// Consuming callers retain ordinary geometry/decoded/metadata admission.
    /// # Errors
    /// Rejects malformed coefficients and out-of-policy source/output extents.
    pub fn new(source: Rect, coefficients_q30: (i32, i32)) -> Result<Self> {
        valid(source)?;
        let (c, s) = (
            i128::from(coefficients_q30.0),
            i128::from(coefficients_q30.1),
        );
        if c.abs() > Q || s.abs() > Q || (c * c + s * s - Q * Q).abs() > 2 * Q {
            return Err(coefficients());
        }
        let mut rotation = Self {
            source,
            coefficients: coefficients_q30,
            min_relative: (0, 0),
        };
        let (lo, _) = rotation.relative_bounds(source)?;
        // Source-relative corners are bounded by Q*(width+height)<=2^51.
        // Narrow only retained state; all subsequent arithmetic stays i128.
        rotation.min_relative = (
            i64::try_from(lo.0).map_err(|_| resource())?,
            i64::try_from(lo.1).map_err(|_| resource())?,
        );
        rotation.bounds(source)?;
        Ok(rotation)
    }
    pub fn source(self) -> Rect {
        self.source
    }
    pub fn coefficients(self) -> (i32, i32) {
        self.coefficients
    }
    pub fn is_identity(self) -> bool {
        self.coefficients == (COEFFICIENT_ONE, 0)
    }
    /// Exact existing orientation when representable; identity is is_identity().
    /// Consumers can keep the old lossless operation rather than resample.
    pub fn exact_orientation(self) -> Option<OrthogonalTransform> {
        match self.coefficients {
            (0, COEFFICIENT_ONE) => Some(OrthogonalTransform::RotateRight),
            (n, 0) if n == -COEFFICIENT_ONE => Some(OrthogonalTransform::Rotate180),
            (0, n) if n == -COEFFICIENT_ONE => Some(OrthogonalTransform::RotateLeft),
            _ => None,
        }
    }
    fn relative(self, x: i64, y: i64) -> Result<(i128, i128)> {
        let cx = add(i128::from(self.source.x0), i128::from(self.source.x1))?;
        let cy = add(i128::from(self.source.y0), i128::from(self.source.y1))?;
        let x = sub(mul(2, i128::from(x))?, cx)?;
        let y = sub(mul(2, i128::from(y))?, cy)?;
        let (c, s) = (
            i128::from(self.coefficients.0),
            i128::from(self.coefficients.1),
        );
        Ok((sub(mul(c, x)?, mul(s, y)?)?, add(mul(s, x)?, mul(c, y)?)?))
    }
    fn relative_bounds(self, rect: Rect) -> Result<((i128, i128), (i128, i128))> {
        let mut lo = (i128::MAX, i128::MAX);
        let mut hi = (i128::MIN, i128::MIN);
        for x in [rect.x0, rect.x1] {
            for y in [rect.y0, rect.y1] {
                let p = self.relative(x, y)?;
                lo.0 = lo.0.min(p.0);
                lo.1 = lo.1.min(p.1);
                hi.0 = hi.0.max(p.0);
                hi.1 = hi.1.max(p.1);
            }
        }
        Ok((lo, hi))
    }
    /// Map arbitrary finite layer extent using this SAME source pivot/offset.
    /// Exact rational minimum is translated before outward floor/ceil; no float.
    /// # Errors
    /// Refuses checked overflow or finite coordinate/canvas-side policy excess.
    pub fn bounds(self, rect: Rect) -> Result<Rect> {
        valid(rect)?;
        let (lo, hi) = self.relative_bounds(rect)?;
        let dx = sub(
            mul(D, i128::from(self.source.x0))?,
            i128::from(self.min_relative.0),
        )?;
        let dy = sub(
            mul(D, i128::from(self.source.y0))?,
            i128::from(self.min_relative.1),
        )?;
        let ceil =
            |n: i128| -> Result<i128> { add(n.div_euclid(D), i128::from(n.rem_euclid(D) != 0)) };
        let convert = |n: i128| i64::try_from(n).map_err(|_| resource());
        let result = Rect {
            x0: convert(add(lo.0, dx)?.div_euclid(D))?,
            y0: convert(add(lo.1, dy)?.div_euclid(D))?,
            x1: convert(ceil(add(hi.0, dx)?)?)?,
            y1: convert(ceil(add(hi.1, dy)?)?)?,
        };
        valid(result)?;
        Ok(result)
    }
    /// Inverse-map one destination pixel center with exact floor. Coordinates
    /// outside the finite source are padding; callers must not read them as data.
    /// # Errors
    /// Refuses out-of-policy destination coordinates or checked arithmetic overflow.
    pub fn source_pixel(self, x: i64, y: i64) -> Result<(i64, i64)> {
        if [x, y]
            .into_iter()
            .any(|v| v.unsigned_abs() > COORD_ABS_MAX as u64)
        {
            return Err(resource());
        }
        let (c, s) = (
            i128::from(self.coefficients.0),
            i128::from(self.coefficients.1),
        );
        let determinant = add(mul(c, c)?, mul(s, s)?)?;
        let u = sub(
            add(
                mul(add(mul(2, i128::from(x))?, 1)?, Q)?,
                i128::from(self.min_relative.0),
            )?,
            mul(D, i128::from(self.source.x0))?,
        )?;
        let v = sub(
            add(
                mul(add(mul(2, i128::from(y))?, 1)?, Q)?,
                i128::from(self.min_relative.1),
            )?,
            mul(D, i128::from(self.source.y0))?,
        )?;
        let cx = add(i128::from(self.source.x0), i128::from(self.source.x1))?;
        let cy = add(i128::from(self.source.y0), i128::from(self.source.y1))?;
        let xn = add(add(mul(c, u)?, mul(s, v)?)?, mul(cx, determinant)?)?;
        let yn = add(sub(mul(c, v)?, mul(s, u)?)?, mul(cy, determinant)?)?;
        let denominator = mul(2, determinant)?;
        Ok((
            i64::try_from(xn.div_euclid(denominator)).map_err(|_| resource())?,
            i64::try_from(yn.div_euclid(denominator)).map_err(|_| resource())?,
        ))
    }
}
#[cfg(test)]
mod tests;

/// Convert a finite clockwise angle in degrees to the closed stored Q30 pair.
/// Exact cardinal angles bypass trigonometry. Other angles use the pinned libm
/// implementation once; commands persist the resulting integers, not an angle.
/// # Errors
/// Returns InputSchema for non-finite/out-of-range input or invalid coefficients.
pub fn coefficients_from_degrees(degrees: f64) -> Result<(i32, i32)> {
    if !degrees.is_finite() || !(-360.0..=360.0).contains(&degrees) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Rotation angle must be finite and between -360 and 360 degrees",
        ));
    }
    let q = COEFFICIENT_ONE;
    let pair = if degrees == 0.0 || degrees.abs() == 360.0 {
        (q, 0)
    } else if degrees == 90.0 || degrees == -270.0 {
        (0, q)
    } else if degrees.abs() == 180.0 {
        (-q, 0)
    } else if degrees == 270.0 || degrees == -90.0 {
        (0, -q)
    } else {
        let (s, c) = libm::sincos(degrees * (core::f64::consts::PI / 180.0));
        (
            libm::round(c * f64::from(q)) as i32,
            libm::round(s * f64::from(q)) as i32,
        )
    };
    Rotation::new(Rect::new(0, 0, 1, 1).expect("unit rectangle"), pair)?;
    Ok(pair)
}

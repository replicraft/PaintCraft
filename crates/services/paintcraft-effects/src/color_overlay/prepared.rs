//! Additive exact prepared arithmetic. Quotes never create execution authority.
use super::*;
use paintcraft_core::types::resources::BudgetAccount;
use std::mem::size_of;

/// Logical formation debit the consuming caller pays before asking for a quote.
pub const QUERY_WORK: u64 = 4096;
/// Maximum explicitly modeled inline control reservation for one row stage.
pub const CONTROL_BYTES: u64 = 1024;
/// Conservative complete source-action envelope per tuple; not CPU instructions.
pub const PIXEL_WORK: u64 = 256;

/// Pure finite row facts. The caller must admit them in its original realm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    pub work: u64,
    pub control_bytes: u64,
}

/// Exact affine coefficients for one closed byte policy; no LUT or heap owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedColorOverlay {
    coefficients: [(u32, u32); 3],
}
impl PreparedColorOverlay {
    /// Form one policy after the consuming caller has paid its formation debit.
    pub fn new(policy: ColorOverlayRgbV1) -> Self {
        let color = policy.color();
        let t = u32::from(policy.opacity());
        let mode = policy.blend();
        let coefficients = [
            coefficient(color[0], t, mode),
            coefficient(color[1], t, mode),
            coefficient(color[2], t, mode),
        ];
        Self { coefficients }
    }
    /// Infallible closed arithmetic; the input alpha, including zero, is copied.
    pub fn pixel(&self, incoming: Rgba) -> Rgba {
        Rgba([
            channel(incoming.0[0], self.coefficients[0]),
            channel(incoming.0[1], self.coefficients[1]),
            channel(incoming.0[2], self.coefficients[2]),
            incoming.0[3],
        ])
    }
    /// Transform at most64 private tuples. Geometry refusal precedes callbacks
    /// and writes; cancellation may leave private output partially transformed.
    ///
    /// # Errors
    /// Returns InputSchema above64 tuples or the original callback error.
    pub fn pixels_in_place(&self, row: &mut [Rgba], check: &dyn Fn() -> Result<()>) -> Result<()> {
        geometry(row.len())?;
        check()?;
        if !row.is_empty() {
            check()?;
        }
        let mut i = 0;
        while i < row.len() {
            row[i] = self.pixel(row[i]);
            i += 1;
        }
        check()
    }
    /// Byte-row counterpart, without unsafe casts, allocation or per-pixel
    /// Result carriers. The row must contain at most64 complete RGBA tuples.
    ///
    /// # Errors
    /// Returns InputSchema for incomplete/long rows or the original callback error.
    pub fn bytes_in_place(&self, row: &mut [u8], check: &dyn Fn() -> Result<()>) -> Result<()> {
        if !row.len().is_multiple_of(4) {
            return Err(invalid());
        }
        geometry(row.len() / 4)?;
        check()?;
        if !row.is_empty() {
            check()?;
        }
        let mut i = 0;
        while i < row.len() {
            row[i] = channel(row[i], self.coefficients[0]);
            row[i + 1] = channel(row[i + 1], self.coefficients[1]);
            row[i + 2] = channel(row[i + 2], self.coefficients[2]);
            i += 4;
        }
        check()
    }
}
// Three fixed <=96-action formations, rather than an iterator/index loop.
fn coefficient(c: u8, t: u32, mode: EffectBlendMode) -> (u32, u32) {
    let c = u32::from(c);
    let normal = (255 - t) * 255;
    match mode {
        EffectBlendMode::Normal => (normal, c * 255 * t),
        EffectBlendMode::Multiply => (normal + c * t, 0),
        EffectBlendMode::Screen => (normal + (255 - c) * t, c * 255 * t),
    }
}
fn channel(x: u8, (gain, bias): (u32, u32)) -> u8 {
    ((u32::from(x) * gain + bias + 32_512) / 65_025) as u8
}
fn geometry(length: usize) -> Result<()> {
    if length > 64 { Err(invalid()) } else { Ok(()) }
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "Prepared Color Overlay row exceeds64 complete tuples",
    )
}
fn unsupported() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Prepared Color Overlay target control layout",
    )
}
/// Defining named inline slots, including the consuming caller's account movers.
/// Successful supported-target queries allocate no heap or budget owner.
/// The consuming inline bound requires phase separation: finish all query
/// Results before forming prepared coefficients/pixels. Retained Quote scalar
/// values are included; arbitrary simultaneous caller allocations are not.
///
/// # Errors
/// Refuses unsupported pointer/result/error layouts or a sum above1024.
pub fn named_control_bytes() -> Result<u64> {
    if !matches!(size_of::<usize>(), 4 | 8)
        || size_of::<PreparedColorOverlay>() != 24
        || size_of::<Error>() > 96
        || size_of::<Result<()>>() > 112
        || size_of::<Result<Quote>>() > 112
        || size_of::<Result<u64>>() > 112
    {
        return Err(unsupported());
    }
    // Pure quotation completes before any Prepared/pixel formation. Successful
    // scalar Quote values remain in Common; complete query Results do not.
    let common = 4 * size_of::<&mut [u8]>()
        + 2 * size_of::<&dyn Fn() -> Result<()>>()
        + 4 * size_of::<Quote>()
        + 5 * size_of::<usize>()
        + 2 * size_of::<BudgetAccount>();
    let query = common
        + 2 * size_of::<Result<Quote>>()
        + 2 * size_of::<Result<u64>>()
        + 2 * size_of::<Result<()>>();
    let execution = common
        + 4 * size_of::<PreparedColorOverlay>()
        + 4 * size_of::<Rgba>()
        + 4 * size_of::<Result<()>>();
    let named = query.max(execution);
    if named as u64 > CONTROL_BYTES {
        return Err(unsupported());
    }
    Ok(named as u64)
}
/// Pure <=64-row quotation. Consuming callers pay QUERY_WORK before this query,
/// then the remaining work before coefficient formation or private writes.
/// The control bound requires those phases to be disjoint: no prepared/pixel
/// formation may overlap live complete query Results.
///
/// # Errors
/// Refuses invalid length or unsupported complete inline control layouts.
pub fn quote(length: usize) -> Result<Quote> {
    geometry(length)?;
    named_control_bytes()?;
    Ok(Quote {
        work: QUERY_WORK + PIXEL_WORK * length as u64,
        control_bytes: CONTROL_BYTES,
    })
}

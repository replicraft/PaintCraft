//! Frozen structural work policy. Counts are paid source facts, never capacity grants.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::{FilterInstance, Layer};
use std::mem::size_of;
pub(super) fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source8 assembly bound")
}
pub(super) fn invalid() -> Error {
    Error::new(ErrorCode::CodecFailed, "Source8 assembly identity mismatch")
}
pub(super) fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
pub(super) fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(limit)
}
pub(super) fn sum(values: &[u64]) -> Result<u64> {
    values.iter().try_fold(0, |a, &b| add(a, b))
}
pub(super) fn buckets(n: u64) -> Result<u64> {
    Ok(match n {
        0 => 0,
        1..=3 => 4,
        4..=7 => 8,
        8..=14 => 16,
        _ => mul(n, 8)?
            .checked_div(7)
            .and_then(u64::checked_next_power_of_two)
            .ok_or_else(limit)?,
    })
}
pub(super) fn bare(n: u64) -> Result<u64> {
    if n < 2 {
        return Ok(0);
    }
    add(
        mul(add(n / 2, n - 1)?, u64::from(64 - n.leading_zeros()) + 1)?,
        n - 1,
    )
}
pub(super) fn stack(n: u64, h: u64) -> Result<u64> {
    sum(&[
        64,
        mul(18, mul(n, n.saturating_sub(1))? / 2)?,
        mul(192, n)?,
        mul(16, add(h, 128)?)?,
        480,
    ])
}
pub(super) fn retire(l: u64, f: u64, o: u64) -> Result<u64> {
    let w = size_of::<usize>() as u64;
    sum(&[
        6 * w,
        mul(l, add(size_of::<Layer>() as u64, 3 * w)?)?,
        mul(f, add(size_of::<FilterInstance>() as u64, 1)?)?,
        o,
    ])
}
/// Facts gathered during the already-priced linear intention stream.
#[derive(Default)]
pub(super) struct AdoptionFacts {
    pub filters: u64,
    pub masks: u64,
    pub own: u64,
    pub filter_masks: u64,
    pub searches: u64,
    pub filter_sorts: u64,
}
impl AdoptionFacts {
    pub fn layer(&mut self, n: u64, m: u64, own: bool) -> Result<()> {
        self.filters = add(self.filters, n)?;
        self.filter_masks = add(self.filter_masks, m)?;
        self.masks = add(self.masks, add(m, u64::from(own))?)?;
        self.own = add(self.own, u64::from(own))?;
        self.searches = add(self.searches, mul(n, m)?)?;
        self.filter_sorts = add(self.filter_sorts, bare(n)?)?;
        Ok(())
    }
}
/// Conservative all-13 registry bound. Literal coefficient table frozen pre-Rust.
pub(super) fn adoption(l: u64, b: u64, f: &AdoptionFacts) -> Result<u64> {
    let w = size_of::<usize>() as u64;
    let index = if w == 8 { 24 } else { 20 };
    let req = if w == 8 { 2064 } else { 1032 };
    let req8 = if w == 8 { 2200 } else { 1120 };
    let registry = if w == 8 { 330880 } else { 255680 };
    let get = sum(&[65, b, mul(18, l)?])?;
    let headers = (b"paintcraft.worker-coverage-bounds/v1\0".len()
        + 2
        + if w == 8 { 11 } else { 9 }
        + 2
        + 4
        + b"paintcraft.worker-mask-inventory/v1\0".len()
        + 4
        + b"paintcraft.worker-layer-masks/v1\0".len()
        + 4) as u64;
    let h = sum(&[headers, mul(278, f.masks)?, mul(49, f.own)?])?;
    sum(&[
        mul(3, b)?,
        mul(add(mul(3, l)?, f.masks)?, get)?,
        mul(l, get.saturating_sub(2048))?,
        mul(
            512,
            sum(&[mul(2, bare(l)?)?, mul(2, f.filter_sorts)?, l, 1])?,
        )?,
        req,
        mul(4, req8)?,
        mul(1706, f.filters)?,
        registry,
        1280 + 64 * 13,
        40 * 13,
        mul(4 * index, add(l, f.filters)?)?,
        mul(2184, f.masks)?,
        mul(18, f.searches)?.saturating_sub(mul(2048, f.filter_masks)?),
        add(mul(16, add(h, 384)?)?, 3 * 480)?,
        2528,
        336 + 2 * req8 + 64,
    ])
}

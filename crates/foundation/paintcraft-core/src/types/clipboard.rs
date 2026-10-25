//! Immutable straight-RGBA8 clipboard pixels. Charges vector capacity, not RSS.
use super::resources::{BudgetLease, BudgetReservation, SharedBudget};
use super::{Error, ErrorCode, Result, Rgba};
use std::sync::Arc;

pub const MAX_PIXELS: u64 = 16_777_216;
pub const MAX_BYTES: u64 = MAX_PIXELS * 4;

#[derive(Debug)]
struct Payload {
    rgba: Vec<u8>,
    _lease: BudgetLease,
    _extra: Option<BudgetLease>,
}

/// Cheap clones retain one immutable allocation and its reservation.
#[derive(Clone, Debug)]
pub struct Image {
    width: u32,
    height: u32,
    payload: Arc<Payload>,
}

fn size(width: u32, height: u32) -> Result<usize> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || pixels > MAX_PIXELS {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Clipboard image dimensions exceed the limit",
        ));
    }
    usize::try_from(pixels * 4)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Clipboard image is too large"))
}

impl Image {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.payload.rgba
    }

    /// Admit before allocating; cancellation or source failure publishes no image.
    pub fn from_fn(
        width: u32,
        height: u32,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
        read: impl Fn(u32, u32) -> Result<Rgba>,
    ) -> Result<Self> {
        check()?;
        let bytes = size(width, height)?;
        let lease = budget.reserve(bytes as u64)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(bytes).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Clipboard pixel allocation failed",
            )
        })?;
        if rgba.capacity() as u64 > MAX_BYTES {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Clipboard allocation exceeds the limit",
            ));
        }
        let extra = if rgba.capacity() > bytes {
            Some(budget.reserve((rgba.capacity() - bytes) as u64)?)
        } else {
            None
        };
        check()?;
        for y in 0..height {
            for x in 0..width {
                if x % 64 == 0 {
                    check()?;
                }
                rgba.extend_from_slice(&read(x, y)?.0);
            }
        }
        check()?;
        Ok(Self {
            width,
            height,
            payload: Arc::new(Payload {
                rgba,
                _lease: lease,
                _extra: extra,
            }),
        })
    }

    /// Admit a zeroed private output before invoking a fallible compositor.
    /// No partial output escapes a failed writer or final cancellation check.
    pub fn from_writer(
        width: u32,
        height: u32,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
        write: impl FnOnce(&mut [u8]) -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let bytes = size(width, height)?;
        let lease = budget.reserve(bytes as u64)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(bytes).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Clipboard pixel allocation failed",
            )
        })?;
        if rgba.capacity() as u64 > MAX_BYTES {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Clipboard allocation exceeds the limit",
            ));
        }
        let extra = if rgba.capacity() > bytes {
            Some(budget.reserve((rgba.capacity() - bytes) as u64)?)
        } else {
            None
        };
        rgba.resize(bytes, 0);
        check()?;
        write(&mut rgba)?;
        check()?;
        Ok(Self {
            width,
            height,
            payload: Arc::new(Payload {
                rgba,
                _lease: lease,
                _extra: extra,
            }),
        })
    }

    /// Adopt a decoder output from capacity reserved before decoding. No second
    /// pixel copy or second reservation is required; unused capacity stays with
    /// the caller's reservation until it drops that attempt.
    pub fn from_rgba_reserved(
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        reservation: &BudgetReservation,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if rgba.len() != size(width, height)? {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Clipboard RGBA length does not match dimensions",
            ));
        }
        if rgba.capacity() as u64 > MAX_BYTES {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Clipboard allocation exceeds the limit",
            ));
        }
        let lease = reservation.reserve(rgba.capacity() as u64)?;
        let image = Self {
            width,
            height,
            payload: Arc::new(Payload {
                rgba,
                _lease: lease,
                _extra: None,
            }),
        };
        check()?;
        Ok(image)
    }

    /// Take already-owned, unmultiplied RGBA bytes; charge their entire capacity.
    pub fn from_rgba(
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if rgba.len() != size(width, height)? {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Clipboard RGBA length does not match dimensions",
            ));
        }
        if rgba.capacity() as u64 > MAX_BYTES {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Clipboard allocation exceeds the limit",
            ));
        }
        let lease = budget.reserve(rgba.capacity() as u64)?;
        let image = Self {
            width,
            height,
            payload: Arc::new(Payload {
                rgba,
                _lease: lease,
                _extra: None,
            }),
        };
        check()?;
        Ok(image)
    }
}

#[cfg(test)]
mod tests;

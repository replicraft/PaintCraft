//! Private precursor for a separately qualified AVIF SDR reader, not registry admission.
//! The existing identity v1 reader and code5 worker do not call this API.
use super::{
    Admitted, Result,
    admission::{self, ColorPolicy},
};

/// Explicit BT.709 YCbCr with sRGB transfer, 8-bit 4:4:4 or 4:2:0, full or
/// limited range, and optional same-size straight monochrome alpha.
///
/// AV1 section 6.4.2 color-config semantics defines CSP_VERTICAL (1) at luma
/// coordinate (0, 0.5) and CSP_COLOCATED (2) at (0, 0). UNKNOWN (0) supplies no
/// inferred position here. Admission does not decode or publicly enable this
/// candidate; independent reconstruction and isolated-worker qualification follow.
pub fn admit_ycbcr8<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Admitted<'a>> {
    admission::admit_with_policy(bytes, pixels, check, ColorPolicy::Ycbcr709)
}

#[cfg(test)]
mod tests;

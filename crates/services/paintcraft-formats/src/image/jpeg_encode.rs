//! JPEG's explicit RGB backdrop, retaining the historical white default.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JpegMatte([u8; 3]);
impl JpegMatte {
    pub const WHITE: Self = Self([255; 3]);
    pub const fn new(rgb: [u8; 3]) -> Self {
        Self(rgb)
    }
    pub const fn rgb(self) -> [u8; 3] {
        self.0
    }
}
impl Default for JpegMatte {
    fn default() -> Self {
        Self::WHITE
    }
}
impl std::str::FromStr for JpegMatte {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        let value = value.strip_prefix('#').unwrap_or(value);
        if value.len() != 6 || !value.bytes().all(|v| v.is_ascii_hexdigit()) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "JPEG matte must be six RGB hexadecimal digits",
            ));
        }
        let mut rgb = [0; 3];
        for (index, channel) in rgb.iter_mut().enumerate() {
            *channel = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                .map_err(|_| Error::new(ErrorCode::InputSchema, "invalid JPEG matte"))?;
        }
        Ok(Self(rgb))
    }
}

fn prepare(
    width: u32,
    height: u32,
    rgba: &[u8],
    matte: JpegMatte,
    scratch_limit: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    check_size(u64::from(width), u64::from(height))?;
    if width > u32::from(u16::MAX) || height > u32::from(u16::MAX) {
        return Err(unsupported("JPEG supports at most 65535 pixels per side"));
    }
    let pixels = u64::from(width) * u64::from(height);
    if rgba.len() as u64 != pixels * 4 {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "pixel buffer does not match the JPEG size",
        ));
    }
    let count = usize::try_from(
        pixels
            .checked_mul(3)
            .ok_or_else(|| codec("JPEG RGB extent overflow"))?,
    )
    .map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "JPEG RGB extent exceeds address space",
        )
    })?;
    if count > scratch_limit || count > isize::MAX as usize {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "JPEG RGB scratch exceeds the byte allowance",
        ));
    }
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(count).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "JPEG RGB scratch allocation refused",
        )
    })?;
    for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if index.is_multiple_of(4096) {
            check()?;
        }
        let alpha = u32::from(pixel[3]);
        for (source, backdrop) in pixel[..3].iter().zip(matte.0) {
            rgb.push(
                ((u32::from(*source) * alpha + u32::from(backdrop) * (255 - alpha) + 127) / 255)
                    as u8,
            );
        }
    }
    check()?;
    Ok(rgb)
}

/// Encode with a frozen RGB8 alpha backdrop. Opaque codec compression is checked
/// before and after execution; the scratch conversion also checks periodically.
pub fn encode_checked(
    width: u32,
    height: u32,
    rgba: &[u8],
    quality: u8,
    matte: JpegMatte,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let rgb = prepare(
        width,
        height,
        rgba,
        matte,
        (DECODE_PIXELS_MAX * 3) as usize,
        check,
    )?;
    check()?;
    let mut output = Vec::new();
    ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality.clamp(1, 100))
        .encode(&rgb, width, height, ::image::ExtendedColorType::Rgb8)
        .map_err(image_error)?;
    check()?;
    Ok(output)
}

#[cfg(test)]
mod tests;

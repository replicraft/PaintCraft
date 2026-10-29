//! Original qualified BMP declarations; this does not interpret calibrated RGB.
use super::raster_source_metadata::{
    self as facts, Digest, Icc, Palette, Resolution, ResolutionUnit,
};
use paintcraft_core::types::Result;
use std::io::Cursor;
pub const CALIBRATED_RGB: u32 = 0;
pub const SRGB: u32 = 0x7352_4742;
pub const WINDOWS_COLOR_SPACE: u32 = 0x5769_6e20;
pub const PROFILE_LINKED: u32 = 0x4c49_4e4b;
pub const PROFILE_EMBEDDED: u32 = 0x4d42_4544;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub source: Digest,
    pub width: u32,
    pub height: u32,
    pub top_down: bool,
    pub dib_bytes: u32,
    pub bits_per_pixel: u16,
    pub compression: u32,
    pub row_stride: u64,
    pub pixel_offset: u32,
    pub masks: [u32; 4],
    pub palette: Palette,
    /// RGBQUAD's fourth byte is reserved, not palette alpha.
    pub palette_reserved: Vec<u8>,
    pub declared_colors_used: u32,
    pub important_colors: u32,
    pub file_reserved: [u16; 2],
    pub resolution: Option<Resolution>,
    pub color_space: Option<u32>,
    pub endpoints_raw: Option<[i32; 9]>,
    pub gamma_scaled: Option<[u32; 3]>,
    pub rendering_intent: Option<u32>,
    pub icc: Option<Icc>,
    pub undeclared_bytes: u64,
}
impl Descriptor {
    pub fn losses(&self) -> Vec<&'static str> {
        let mut losses = Vec::new();
        if self.bits_per_pixel <= 8 {
            losses.push("palette-indices-expanded");
        }
        if self.icc.is_some() {
            losses.push("icc-profile-ignored");
        }
        if self.color_space.is_some() {
            losses.push("bmp-color-metadata-ignored");
        }
        if self.resolution.is_some() {
            losses.push("resolution-metadata-ignored");
        }
        if self.undeclared_bytes > 0
            || self.important_colors != 0
            || self.file_reserved != [0; 2]
            || self.bits_per_pixel > 8 && self.declared_colors_used != 0
        {
            losses.push("bmp-metadata-ignored");
        }
        losses
    }
}
fn u16_at(bytes: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + 2)
            .ok_or_else(|| facts::invalid("Truncated BMP field"))?
            .try_into()
            .unwrap(),
    ))
}
fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or_else(|| facts::invalid("Truncated BMP field"))?
            .try_into()
            .unwrap(),
    ))
}
pub fn inspect(bytes: &[u8], pixels: u64, check: &dyn Fn() -> Result<()>) -> Result<Descriptor> {
    // BMP's existing decoder does not allocate embedded profiles. Streaming
    // inspection therefore defaults to source extent, not a new profile-size cap.
    inspect_with_metadata_limit(bytes, pixels, bytes.len() as u64, check)
}
pub fn inspect_with_metadata_limit(
    bytes: &[u8],
    pixels: u64,
    metadata_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Descriptor> {
    check()?;
    facts::allowance(bytes, pixels)?;
    if bytes.get(..2) != Some(b"BM") {
        return Err(facts::invalid("BMP signature"));
    }
    let dib = u32_at(bytes, 14)?;
    if !matches!(dib, 40 | 108 | 124) {
        return Err(facts::unsupported("Unsupported BMP source header"));
    }
    let header_end = 14 + dib as usize;
    if bytes.len() < header_end {
        return Err(facts::invalid("Truncated BMP source header"));
    }
    let width = u32_at(bytes, 18)? as i32;
    let height = u32_at(bytes, 22)? as i32;
    if width <= 0 || height == 0 || height == i32::MIN || u16_at(bytes, 26)? != 1 {
        return Err(facts::invalid("BMP source dimensions/planes"));
    }
    let (width, height, top_down) = (width as u32, height.unsigned_abs(), height < 0);
    super::check_size(u64::from(width), u64::from(height))?;
    if u64::from(width) * u64::from(height) > pixels.min(super::DECODE_PIXELS_MAX) {
        return Err(facts::resource("BMP metadata pixel allowance"));
    }
    let bpp = u16_at(bytes, 28)?;
    let compression = u32_at(bytes, 30)?;
    if !matches!(bpp, 1 | 4 | 8 | 24 | 32) {
        return Err(facts::unsupported("Unsupported BMP source depth"));
    }
    let mut masks = [0; 4];
    let external_masks = match compression {
        0 => 0,
        3 if bpp == 32 => {
            for (i, value) in masks[..3].iter_mut().enumerate() {
                *value = u32_at(bytes, 54 + i * 4)?;
            }
            if dib >= 108 {
                masks[3] = u32_at(bytes, 66)?;
            }
            if masks[..3] != [0x00ff0000, 0x0000ff00, 0x000000ff]
                || !matches!(masks[3], 0 | 0xff000000)
            {
                return Err(facts::unsupported("Unsupported BMP source channel masks"));
            }
            if dib == 40 { 12 } else { 0 }
        }
        _ => return Err(facts::unsupported("Unsupported BMP source compression")),
    };
    let row_stride = (u64::from(width) * u64::from(bpp)).div_ceil(32) * 4;
    let pixel_offset = u32_at(bytes, 10)?;
    let count = if bpp <= 8 {
        let n = u32_at(bytes, 46)?;
        if n == 0 { 1u32 << bpp } else { n }
    } else {
        0
    };
    if count > 1u32 << bpp.min(8) {
        return Err(facts::invalid("BMP source palette domain"));
    }
    let metadata_end = header_end + external_masks + count as usize * 4;
    let pixels_end = u64::from(pixel_offset)
        .checked_add(row_stride * u64::from(height))
        .ok_or_else(|| facts::invalid("BMP pixel extent"))?;
    if (pixel_offset as usize) < metadata_end || pixels_end > bytes.len() as u64 {
        return Err(facts::invalid("BMP source pixel bounds"));
    }
    let mut palette = Palette::default();
    let mut palette_reserved = Vec::new();
    for entry in bytes[header_end + external_masks..metadata_end]
        .as_chunks::<4>()
        .0
    {
        check()?;
        palette.rgb.push([entry[2], entry[1], entry[0]]);
        palette_reserved.push(entry[3]);
    }
    // RGBQUAD reserved bytes are not palette alpha under this qualified profile.
    let (x, y) = (u32_at(bytes, 38)? as i32, u32_at(bytes, 42)? as i32);
    if x < 0 || y < 0 {
        return Err(facts::invalid("Negative BMP physical resolution"));
    }
    let resolution = (x != 0 || y != 0).then_some(Resolution {
        x: x as u32,
        y: y as u32,
        unit: ResolutionUnit::Meter,
    });
    let mut result = Descriptor {
        source: facts::digest(bytes, check)?,
        width,
        height,
        top_down,
        dib_bytes: dib,
        bits_per_pixel: bpp,
        compression,
        row_stride,
        pixel_offset,
        masks,
        palette,
        palette_reserved,
        declared_colors_used: u32_at(bytes, 46)?,
        important_colors: u32_at(bytes, 50)?,
        file_reserved: [u16_at(bytes, 6)?, u16_at(bytes, 8)?],
        resolution,
        color_space: None,
        endpoints_raw: None,
        gamma_scaled: None,
        rendering_intent: None,
        icc: None,
        undeclared_bytes: u64::from(pixel_offset) - metadata_end as u64 + bytes.len() as u64
            - pixels_end,
    };
    if dib >= 108 {
        let color = u32_at(bytes, 70)?;
        result.color_space = Some(color);
        result.endpoints_raw = Some(std::array::from_fn(|i| {
            u32_at(bytes, 74 + i * 4).unwrap() as i32
        }));
        result.gamma_scaled = Some(std::array::from_fn(|i| u32_at(bytes, 110 + i * 4).unwrap()));
        if !matches!(
            color,
            CALIBRATED_RGB | SRGB | WINDOWS_COLOR_SPACE | PROFILE_EMBEDDED | PROFILE_LINKED
        ) {
            return Err(facts::unsupported("Unknown BMP source color declaration"));
        }
        if matches!(color, PROFILE_EMBEDDED | PROFILE_LINKED) && dib != 124 {
            return Err(facts::invalid("BMP profile requires V5 header"));
        }
    }
    if dib == 124 {
        result.rendering_intent = Some(u32_at(bytes, 122)?);
        let offset = u64::from(u32_at(bytes, 126)?);
        let length = u64::from(u32_at(bytes, 130)?);
        if u32_at(bytes, 134)? != 0 {
            return Err(facts::invalid("BMP V5 reserved field"));
        }
        if result.color_space == Some(PROFILE_LINKED) {
            return Err(facts::unsupported(
                "Linked BMP color profiles are not opened",
            ));
        }
        if result.color_space == Some(PROFILE_EMBEDDED) {
            let start = 14u64
                .checked_add(offset)
                .ok_or_else(|| facts::invalid("BMP profile offset"))?;
            let end = start
                .checked_add(length)
                .ok_or_else(|| facts::invalid("BMP profile extent"))?;
            if length == 0
                || start < metadata_end as u64
                || end > bytes.len() as u64
                || start < pixels_end && end > u64::from(pixel_offset)
            {
                return Err(facts::invalid("BMP profile overlaps source structures"));
            }
            result.icc = Some(facts::inspect_icc(
                Cursor::new(&bytes[start as usize..end as usize]),
                metadata_limit,
                check,
            )?);
            result.undeclared_bytes = result
                .undeclared_bytes
                .checked_sub(length)
                .ok_or_else(|| facts::invalid("BMP profile accounting"))?;
        } else if offset != 0 || length != 0 {
            // The specification ignores these fields without a profile CSType.
            // Preserve their presence as omitted metadata, never dereference them.
            result.undeclared_bytes = result.undeclared_bytes.max(1);
        }
    }
    check()?;
    Ok(result)
}
#[cfg(test)]
mod tests;

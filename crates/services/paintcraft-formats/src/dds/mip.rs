//! Explicit top-mip conversion backend. Ordinary image APIs remain single-level.
use super::*;

pub const OTHER_MIPS_LOSS: &str = "dds-other-mips-not-imported";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MipLevel {
    pub width: u32,
    pub height: u32,
    /// Absolute byte offset in the admitted source, not a pixel-buffer offset.
    pub byte_offset: u64,
    pub byte_len: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MipInspection {
    pub image: Inspection,
    /// Source order, beginning at level zero; at most 17 levels.
    pub levels: Vec<MipLevel>,
}
impl MipInspection {
    pub fn losses(&self) -> &[&'static str] {
        if self.levels.len() > 1 {
            &[OTHER_MIPS_LOSS]
        } else {
            &[]
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedTopMip {
    pub info: MipInspection,
    /// Only the top level is decoded/allocated. Lower encoded levels are omitted.
    pub rgba: Vec<u8>,
}
impl DecodedTopMip {
    pub fn losses(&self) -> &[&'static str] {
        self.info.losses()
    }
}
pub(super) fn admit_count(bytes: &[u8], header: &Header, raw: u32) -> Result<()> {
    let flags = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let caps = u32::from_le_bytes(bytes[108..112].try_into().unwrap());
    if flags & 0x1007 != 0x1007
        || caps & 0x1000 == 0
        || (raw == 0 && flags & 0x20000 != 0)
        || (raw > 1 && (flags & 0x20000 == 0 || caps & 0x400008 != 0x400008))
        || (caps & 0x400000 != 0 && (raw < 2 || flags & 0x20000 == 0))
        || (raw > 1 && header.mipmap_count().get() != raw)
    {
        return Err(codec("DDS mip count/flags/capabilities disagree"));
    }
    let maximum = 32 - header.width().max(header.height()).leading_zeros();
    if raw > maximum.max(1) {
        return Err(codec("DDS mip count exceeds the geometry's chain length"));
    }
    Ok(())
}
fn level_bytes(width: u32, height: u32, encoding: Encoding) -> Result<u64> {
    let (w, h) = (u64::from(width), u64::from(height));
    let length = match encoding {
        Encoding::Rgba8 | Encoding::Bgra8 => w.checked_mul(h).and_then(|n| n.checked_mul(4)),
        Encoding::Bc1 | Encoding::Bc2 | Encoding::Bc3 => w
            .div_ceil(4)
            .checked_mul(h.div_ceil(4))
            .and_then(|n| n.checked_mul(if encoding == Encoding::Bc1 { 8 } else { 16 })),
    };
    length.ok_or_else(|| resource("DDS mip extent overflow"))
}
pub(super) fn layout(
    layout: ::dds::DataLayout,
    info: Inspection,
    body_offset: usize,
    body_bytes: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<MipInspection> {
    let texture = layout
        .texture()
        .ok_or_else(|| unsupported("DDS source is not one 2D texture"))?;
    let mut levels = Vec::new();
    levels
        .try_reserve_exact(usize::from(texture.mipmaps()))
        .map_err(|_| resource("DDS mip inventory allocation"))?;
    let mut offset = 0u64;
    let (mut width, mut height) = (info.width, info.height);
    for surface in texture.iter_mips() {
        check()?;
        let length = level_bytes(width, height, info.encoding)?;
        if surface.width() != width
            || surface.height() != height
            || surface.data_offset() != offset
            || surface.data_len() != length
        {
            return Err(codec(
                "DDS decoder layout differs from checked mip geometry",
            ));
        }
        levels.push(MipLevel {
            width,
            height,
            byte_offset: (body_offset as u64)
                .checked_add(offset)
                .ok_or_else(|| resource("DDS mip offset overflow"))?,
            byte_len: length,
        });
        offset = offset
            .checked_add(length)
            .ok_or_else(|| resource("DDS mip chain overflow"))?;
        if offset > BYTES_MAX as u64 {
            return Err(resource("DDS mip chain exceeds byte allowance"));
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    if levels.is_empty() || offset != body_bytes as u64 || layout.data_len() != offset {
        return Err(codec(
            "DDS data length does not exactly match its checked mip chain",
        ));
    }
    Ok(MipInspection {
        image: info,
        levels,
    })
}
pub fn inspect_top_mip_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<MipInspection> {
    let control = Control::new(check);
    let (_, plan) = prepare_profile(bytes, pixels, &control, true)?;
    Ok(plan)
}
/// No transfer or alpha interpretation is invented. Frontends must explicitly
/// admit this conversion and require its losses before publishing a document.
/// Exact full-tail layout is checked before the sole top-level pixel allocation.
pub fn decode_top_mip_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<DecodedTopMip> {
    let control = Control::new(check);
    let (mut decoder, plan) = prepare_profile(bytes, pixels, &control, true)?;
    let mut rgba = read_rgba(&mut decoder, plan.image, &control)?;
    let top = &plan.levels[0];
    let expected_tail = (bytes.len() as u64)
        .checked_sub(
            top.byte_offset
                .checked_add(top.byte_len)
                .ok_or_else(|| resource("DDS top mip extent overflow"))?,
        )
        .ok_or_else(|| codec("DDS top mip exceeds input"))?;
    // The unconsumed tail proves the decoder did not decode/read lower levels.
    if decoder.into_reader().remaining.len() as u64 != expected_tail {
        return Err(codec("DDS decoder consumed beyond the selected top mip"));
    }
    finish_alpha(&mut rgba, plan.image, check)?;
    Ok(DecodedTopMip { info: plan, rgba })
}
#[cfg(test)]
mod tests;

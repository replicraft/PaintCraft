//! Scoped single-surface DDS decoding; no frontend capability promotion.
//!
//! Raw channel precision and transfer/alpha hints remain explicit. Texture
//! arrays, mipmaps, cubemaps, volumes and unsupported precision are refused.
mod encode;
mod input;
pub use encode::{encode_rgba8, encode_rgba8_checked};
#[cfg(test)]
mod tests;
use ::dds::{
    ColorFormat, DataRegion, Decoder, Format, ImageViewMut,
    header::{AlphaMode, DxgiFormat, Header, ParseOptions, ResourceDimension},
};
use input::{Control, Input};
use paintcraft_core::types::{Error, ErrorCode, Result};

pub const BYTES_MAX: usize = 64 << 20;
pub const PIXELS_MAX: u64 = 4_194_304;
const WORKSPACE_BYTES_MAX: usize = 8 << 20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Rgba8,
    Bgra8,
    Bc1,
    Bc2,
    Bc3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    Unspecified,
    Linear,
    Srgb,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceAlpha {
    Unspecified,
    Straight,
    Opaque,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub width: u32,
    pub height: u32,
    pub encoding: Encoding,
    pub transfer: Transfer,
    pub alpha: SourceAlpha,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub info: Inspection,
    pub rgba: Vec<u8>,
}
pub fn sniff(bytes: &[u8]) -> bool {
    bytes.starts_with(b"DDS ")
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn codec(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn admit(header: &Header, pixels: u64) -> Result<Inspection> {
    let width = header.width();
    let height = header.height();
    if width == 0 || height == 0 {
        return Err(codec("DDS dimensions must be positive"));
    }
    if width > 65_536
        || height > 65_536
        || u64::from(width) * u64::from(height) > pixels.min(PIXELS_MAX)
    {
        return Err(resource("DDS exceeds the pixel limit"));
    }
    if header.depth().is_some()
        || header.is_volume()
        || header.is_cube_map()
        || header.array_size() != 1
        || header.mipmap_count().get() != 1
    {
        return Err(unsupported(
            "DDS mipmaps, arrays, cubemaps and volumes are not qualified",
        ));
    }
    if header
        .dx10()
        .is_some_and(|h| h.resource_dimension != ResourceDimension::Texture2D)
    {
        return Err(unsupported("Only 2D DDS textures are qualified"));
    }
    if header.dx10().is_some_and(|dx10| {
        !matches!(
            dx10.dxgi_format,
            DxgiFormat::R8G8B8A8_UNORM
                | DxgiFormat::R8G8B8A8_UNORM_SRGB
                | DxgiFormat::B8G8R8A8_UNORM
                | DxgiFormat::B8G8R8A8_UNORM_SRGB
                | DxgiFormat::BC1_UNORM
                | DxgiFormat::BC1_UNORM_SRGB
                | DxgiFormat::BC2_UNORM
                | DxgiFormat::BC2_UNORM_SRGB
                | DxgiFormat::BC3_UNORM
                | DxgiFormat::BC3_UNORM_SRGB
        )
    }) {
        return Err(unsupported(
            "DDS typeless, deep or unsupported DXGI profile",
        ));
    }
    let alpha = match header.alpha_mode() {
        AlphaMode::Unknown => SourceAlpha::Unspecified,
        AlphaMode::Straight => SourceAlpha::Straight,
        AlphaMode::Opaque => SourceAlpha::Opaque,
        _ => {
            return Err(unsupported(
                "DDS premultiplied or custom alpha requires explicit conversion",
            ));
        }
    };
    let format = Format::from_header(header)
        .map_err(|e| unsupported(&format!("DDS format is not qualified: {e}")))?;
    let encoding = match format {
        Format::R8G8B8A8_UNORM => Encoding::Rgba8,
        Format::B8G8R8A8_UNORM => Encoding::Bgra8,
        Format::BC1_UNORM => Encoding::Bc1,
        Format::BC2_UNORM => Encoding::Bc2,
        Format::BC3_UNORM => Encoding::Bc3,
        _ => {
            return Err(unsupported(
                "DDS precision or compression profile is not qualified",
            ));
        }
    };
    let transfer = if header.dx10().is_none() {
        Transfer::Unspecified
    } else if header.is_srgb() {
        Transfer::Srgb
    } else {
        Transfer::Linear
    };
    Ok(Inspection {
        width,
        height,
        encoding,
        transfer,
        alpha,
    })
}
fn prepare<'a, 'b>(
    bytes: &'a [u8],
    pixels: u64,
    control: &'b Control<'b>,
) -> Result<(Decoder<Input<'a, 'b>>, Inspection)> {
    (control.check)()?;
    if bytes.len() > BYTES_MAX {
        return Err(resource("DDS exceeds the input byte limit"));
    }
    let mut input = Input {
        remaining: bytes,
        control,
    };
    let mut options = ParseOptions::default();
    options.permissive = false;
    options.skip_magic_bytes = false;
    let header = Header::read(&mut input, &options)
        .map_err(|e| control.error(codec(format!("DDS header is invalid: {e}"))))?;
    // Preserve strict refusal even when an older writer forgets the flag;
    // the library otherwise normalizes an unflagged mip count to one.
    if u32::from_le_bytes(bytes[28..32].try_into().unwrap()) > 1 {
        return Err(unsupported("DDS mipmaps require a texture import workflow"));
    }
    if u32::from_le_bytes(bytes[112..116].try_into().unwrap()) != 0 {
        return Err(unsupported(
            "DDS nontrivial surface capabilities are not qualified",
        ));
    }
    if header.dx10().is_some()
        && (u32::from_le_bytes(bytes[136..140].try_into().unwrap()) & !4 != 0
            || u32::from_le_bytes(bytes[144..148].try_into().unwrap()) & !7 != 0)
    {
        return Err(unsupported(
            "DDS reserved resource or alpha flags are not qualified",
        ));
    }
    let info = admit(&header, pixels)?;
    let mut decoder = Decoder::from_header(input, header).map_err(|e| control.map(e))?;
    decoder.options.memory_limit = WORKSPACE_BYTES_MAX;
    if !decoder.layout().is_texture() {
        return Err(unsupported("DDS source is not a single texture"));
    }
    let body_offset = 4usize
        .checked_add(decoder.header().byte_len())
        .ok_or_else(|| resource("DDS header size overflow"))?;
    let body_bytes = bytes
        .len()
        .checked_sub(body_offset)
        .ok_or_else(|| codec("Truncated DDS header"))?;
    if decoder.layout().data_len() != body_bytes as u64 {
        return Err(codec(
            "DDS data length does not exactly match its single surface",
        ));
    }
    let flags = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let pitch = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    match info.encoding {
        Encoding::Rgba8 | Encoding::Bgra8
            if flags & 8 != 0 && u64::from(pitch) != u64::from(info.width) * 4 =>
        {
            return Err(unsupported("Padded DDS row pitch is not qualified"));
        }
        Encoding::Bc1 | Encoding::Bc2 | Encoding::Bc3
            if flags & 0x80000 != 0 && u64::from(pitch) != body_bytes as u64 =>
        {
            return Err(codec(
                "DDS compressed linear size does not match its surface",
            ));
        }
        _ => {}
    }
    (control.check)()?;
    Ok((decoder, info))
}
pub fn inspect_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection> {
    let control = Control::new(check);
    let (_, info) = prepare(bytes, pixels, &control)?;
    Ok(info)
}
pub fn decode_with_pixel_limit(bytes: &[u8], pixels: u64) -> Result<Decoded> {
    decode_checked(bytes, pixels, &|| Ok(()))
}
pub fn decode_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    let control = Control::new(check);
    let (mut decoder, info) = prepare(bytes, pixels, &control)?;
    let count = usize::try_from(u64::from(info.width) * u64::from(info.height) * 4)
        .map_err(|_| resource("DDS output length exceeds target address space"))?;
    if count > BYTES_MAX {
        return Err(resource("DDS exceeds the output byte limit"));
    }
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(count)
        .map_err(|_| resource("DDS output allocation failed"))?;
    rgba.resize(count, 0);
    let view = ImageViewMut::new(&mut rgba, decoder.main_size(), ColorFormat::RGBA_U8)
        .ok_or_else(|| codec("DDS output geometry is invalid"))?;
    decoder.read_surface(view).map_err(|e| control.map(e))?;
    if !decoder.is_done() {
        return Err(codec("DDS decoder did not consume exactly one surface"));
    }
    if !decoder.into_reader().remaining.is_empty() {
        return Err(codec("DDS decoder left trailing surface bytes"));
    }
    if info.alpha == SourceAlpha::Opaque {
        for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if index.is_multiple_of(1024) {
                check()?;
            }
            pixel[3] = 255;
        }
    }
    check()?;
    Ok(Decoded { info, rgba })
}

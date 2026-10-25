//! Bounded extended raster adapters; pixel coding delegates to selected codecs.
use super::*;
mod encode;
pub use encode::PIXELS_MAX as ENCODE_PIXELS_MAX;
pub(super) use encode::encode;
mod pcx;
mod pnm;
pub(super) use pcx::decode as decode_pcx;
pub(crate) use pcx::inspect as inspect_pcx;
pub(super) use pnm::admit as admit_pnm;

pub(super) fn buffer(bytes: u64) -> Result<Vec<u8>> {
    admit_buffer_bytes(bytes, isize::MAX as u64)?;
    let length = usize::try_from(bytes).map_err(|_| codec("image buffer length overflow"))?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot allocate decoded image"))?;
    result.resize(length, 0);
    Ok(result)
}
fn admit_dimensions(width: u32, height: u32, pixels: u64) -> Result<u64> {
    check_size(u64::from(width), u64::from(height))?;
    let count = u64::from(width) * u64::from(height);
    if count > pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "extended raster exceeds pixel budget",
        ));
    }
    Ok(count)
}
pub(super) fn admit_farbfeld(bytes: &[u8], pixels: u64) -> Result<Vec<String>> {
    let h = bytes
        .get(..16)
        .ok_or_else(|| codec("truncated farbfeld header"))?;
    if &h[..8] != b"farbfeld" {
        return Err(codec("invalid farbfeld signature"));
    }
    let count = admit_dimensions(
        u32::from_be_bytes(h[8..12].try_into().unwrap()),
        u32::from_be_bytes(h[12..16].try_into().unwrap()),
        pixels,
    )?;
    if 16 + count * 8 != bytes.len() as u64 {
        return Err(codec("farbfeld requires exactly one complete raster"));
    }
    Ok(Vec::new()) // decode_pixels reports the explicit 16-to-8 reduction.
}

pub(super) fn decode_qoi(bytes: &[u8], pixels: u64) -> Result<Decoded> {
    let header = bytes
        .get(..14)
        .ok_or_else(|| codec("truncated QOI header"))?;
    if &header[..4] != b"qoif" || !matches!(header[12], 3 | 4) {
        return Err(codec("invalid QOI header"));
    }
    if header[13] != 0 {
        return Err(unsupported(
            "linear or unknown QOI color space requires color conversion",
        ));
    }
    let width = u32::from_be_bytes(header[4..8].try_into().unwrap());
    let height = u32::from_be_bytes(header[8..12].try_into().unwrap());
    let count = admit_dimensions(width, height, pixels)?;
    // Validate lengths before decoding. The codec clamps an overlong final RUN;
    // such malformed streams must be refused rather than silently repaired.
    let mut at = 14usize;
    let mut decoded = 0u64;
    while decoded < count {
        let tag = *bytes.get(at).ok_or_else(|| codec("truncated QOI opcode"))?;
        at += 1;
        let (extra, run) = match tag {
            0xfe => (3, 1),
            0xff => (4, 1),
            0xc0..=0xfd => (0, u64::from(tag & 63) + 1),
            0x80..=0xbf => (1, 1),
            _ => (0, 1),
        };
        if decoded + run > count {
            return Err(codec("QOI run exceeds the image extent"));
        }
        at = at
            .checked_add(extra)
            .ok_or_else(|| codec("QOI offset overflow"))?;
        if at > bytes.len() {
            return Err(codec("truncated QOI opcode payload"));
        }
        decoded += run;
    }
    if bytes.get(at..) != Some(&[0, 0, 0, 0, 0, 0, 0, 1][..]) {
        return Err(codec("invalid QOI end marker or trailing data"));
    }
    // QOI's channel count is informative, not an opcode restriction. qoi 0.4
    // chooses an RGB-only opcode path for a 3-channel header, so normalize that
    // single field without copying the encoded body or changing pixel semantics.
    let mut normalized: [u8; 14] = header.try_into().unwrap();
    normalized[12] = 4;
    use std::io::Read as _;
    let reader = Cursor::new(normalized).chain(Cursor::new(&bytes[14..]));
    let mut decoder = qoi::Decoder::from_stream(reader)
        .map_err(|e| codec(e.to_string()))?
        .with_channels(qoi::Channels::Rgba);
    let mut rgba = buffer(count * 4)?;
    decoder
        .decode_to_buf(&mut rgba)
        .map_err(|e| codec(e.to_string()))?;
    Ok(Decoded {
        format: ImageFormat::Qoi,
        width,
        height,
        rgba,
        losses: Vec::new(),
    })
}

use super::{Encoding, malformed, u16_at, u32_at, unsupported};
use paintcraft_core::types::Result;

pub(super) fn inspect(data: &[u8], width: u32, height: u32) -> Result<(Encoding, u16, u32)> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(data, width, height)
    } else {
        bitmap(data, width, height)
    }
}

fn png(data: &[u8], width: u32, height: u32) -> Result<(Encoding, u16, u32)> {
    png_options(data, width, height, false)
}
pub(super) fn png_with_metadata(
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<(Encoding, u16, u32)> {
    png_options(data, width, height, true)
}
fn png_options(
    data: &[u8],
    width: u32,
    height: u32,
    metadata: bool,
) -> Result<(Encoding, u16, u32)> {
    let mut offset = 8usize;
    let mut depth = None;
    let mut colors = 0;
    let mut image_data = false;
    let mut data_ended = false;
    let mut transparency = false;
    let (mut srgb, mut exif, mut chunks) = (false, false, 0usize);
    while offset < data.len() {
        chunks += 1;
        if metadata && chunks > 8192 {
            return Err(unsupported("ICNS PNG chunk allowance exceeded"));
        }
        let header = data
            .get(offset..offset + 8)
            .ok_or_else(|| malformed("truncated icon PNG chunk"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as u64;
        let end = offset as u64 + 12 + length;
        if end > data.len() as u64 {
            return Err(malformed("icon PNG chunk exceeds payload"));
        }
        let end = end as usize;
        let body = &data[offset + 8..end - 4];
        let crc = u32::from_be_bytes(data[end - 4..end].try_into().unwrap());
        if crc32fast::hash(&data[offset + 4..end - 4]) != crc {
            return Err(malformed("icon PNG CRC mismatch"));
        }
        let tag = &header[4..8];
        if offset == 8 && tag != b"IHDR" {
            return Err(malformed("icon PNG must start with IHDR"));
        }
        match tag {
            b"IHDR" => {
                if depth.is_some() || body.len() != 13 {
                    return Err(malformed("invalid icon PNG IHDR"));
                }
                if u32::from_be_bytes(body[..4].try_into().unwrap()) != width
                    || u32::from_be_bytes(body[4..8].try_into().unwrap()) != height
                {
                    return Err(malformed("icon PNG dimensions contradict directory"));
                }
                let channels = match body[9] {
                    0 => 1,
                    2 => 3,
                    3 => 1,
                    4 => 2,
                    6 => 4,
                    _ => return Err(malformed("invalid icon PNG color type")),
                };
                if body[8] != 8 || body[10..] != [0, 0, 0] {
                    return Err(unsupported(
                        "icon PNG requires noninterlaced eight-bit samples",
                    ));
                }
                depth = Some((u16::from(body[8]) * channels, body[9]));
            }
            b"PLTE" => {
                if image_data
                    || colors != 0
                    || body.is_empty()
                    || !body.len().is_multiple_of(3)
                    || body.len() > 768
                {
                    return Err(malformed("invalid icon PNG palette"));
                }
                colors = (body.len() / 3) as u32;
            }
            b"tRNS" => {
                if image_data || transparency {
                    return Err(malformed("invalid icon PNG transparency order"));
                }
                transparency = true;
            }
            b"sRGB" if metadata => {
                if srgb || image_data || !matches!(body, [0..=3]) {
                    return Err(malformed("invalid ICNS PNG sRGB declaration"));
                }
                srgb = true;
            }
            b"eXIf" if metadata => {
                if exif || image_data || body.len() > 65536 {
                    return Err(unsupported("ICNS PNG EXIF profile or allowance"));
                }
                exif = true;
            }
            b"IDAT" => {
                if data_ended {
                    return Err(malformed("noncontiguous icon PNG image data"));
                }
                image_data = true;
            }
            b"IEND" => {
                if !body.is_empty() || !image_data || end != data.len() {
                    return Err(malformed("invalid icon PNG end"));
                }
                let (bits, color_type) =
                    depth.ok_or_else(|| malformed("missing icon PNG header"))?;
                if color_type == 3 && colors == 0 {
                    return Err(malformed("missing icon PNG palette"));
                }
                return Ok((Encoding::Png, bits, colors));
            }
            _ => {
                return Err(unsupported(
                    "icon PNG ancillary or extension chunk is not qualified",
                ));
            }
        }
        if image_data && tag != b"IDAT" {
            data_ended = true;
        }
        offset = end;
    }
    Err(malformed("missing icon PNG IEND"))
}

fn bitmap(data: &[u8], width: u32, height: u32) -> Result<(Encoding, u16, u32)> {
    let h = data
        .get(..40)
        .ok_or_else(|| malformed("truncated icon bitmap header"))?;
    if u32_at(h, 0) != 40 {
        return Err(unsupported("icon bitmap header variant not qualified"));
    }
    if u32_at(h, 4) != width || u32_at(h, 8) != height * 2 || u16_at(h, 12) != 1 {
        return Err(malformed(
            "icon bitmap geometry or planes contradict directory",
        ));
    }
    let bits = u16_at(h, 14);
    if ![1, 4, 8, 24, 32].contains(&bits) || u32_at(h, 16) != 0 {
        return Err(unsupported(
            "icon bitmap depth or compression not qualified",
        ));
    }
    if h[24..32] != [0; 8] || u32_at(h, 36) != 0 {
        return Err(unsupported(
            "icon bitmap resolution or important-color metadata not qualified",
        ));
    }
    let declared = u32_at(h, 32);
    let colors = if bits <= 8 {
        if declared > 1 << bits {
            return Err(malformed("icon palette exceeds bit depth"));
        }
        if declared == 0 { 1 << bits } else { declared }
    } else {
        if declared != 0 {
            return Err(unsupported("true-color icon palette not qualified"));
        }
        0
    };
    let xor = (width * u32::from(bits)).div_ceil(32) * 4 * height;
    let mask = width.div_ceil(32) * 4 * height;
    let image_size = u32_at(h, 20);
    if image_size != 0 && image_size != xor && image_size != xor + mask {
        return Err(malformed("icon bitmap declared image size mismatch"));
    }
    if data.len() as u64 != 40 + u64::from(colors) * 4 + u64::from(xor + mask) {
        return Err(malformed("icon bitmap pixel or mask length mismatch"));
    }
    if bits <= 8 {
        // The selected decoder indexes its declared palette without a bounds
        // check. Validate XOR indices, including pixels hidden by the AND mask.
        // Unused packed bits and physical row padding are not samples.
        let start = 40 + colors as usize * 4;
        let stride = (width * u32::from(bits)).div_ceil(32) as usize * 4;
        for row in data[start..start + xor as usize].chunks_exact(stride) {
            for x in 0..width as usize {
                let index = match bits {
                    8 => row[x],
                    4 => (row[x / 2] >> if x % 2 == 0 { 4 } else { 0 }) & 15,
                    _ => (row[x / 8] >> (7 - x % 8)) & 1,
                };
                if u32::from(index) >= colors {
                    return Err(malformed("icon bitmap index exceeds declared palette"));
                }
            }
        }
    }
    Ok((Encoding::Bitmap, bits, colors))
}

//! Strict RLE source admission around the selected image BMP RGB decoder.
use super::bmp_metadata::Descriptor;
use super::*;
#[derive(Clone, Copy)]
pub(super) struct Spec {
    pub width: u32,
    pub height: u32,
    pub depth: u16,
    pub palette_entries: u32,
    pub pixel_offset: u32,
    pub declared_bytes: u32,
    pub limit: u64,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Span {
    pub stream_bytes: u64,
    pub storage_bytes: u64,
}
struct Input<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Input<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8]> {
        let end = self
            .at
            .checked_add(count)
            .ok_or_else(|| codec("BMP RLE command extent"))?;
        let result = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| codec("Truncated BMP RLE command or missing EOI"))?;
        self.at = end;
        Ok(result)
    }
}
pub(super) fn is_rle(bytes: &[u8]) -> bool {
    bytes
        .get(30..34)
        .is_some_and(|v| matches!(u32::from_le_bytes(v.try_into().unwrap()), 1 | 2))
}
/// The callback sees only actual authored source indices, never skipped holes.
/// Cursor motion is monotone and source bytes advance even for a zero delta.
pub(super) fn scan(
    bytes: &[u8],
    spec: Spec,
    check: &dyn Fn() -> Result<()>,
    mut put: impl FnMut(u32, u32, u8) -> Result<()>,
) -> Result<Span> {
    check()?;
    if !matches!(spec.depth, 4 | 8)
        || spec.width == 0
        || spec.height == 0
        || spec.palette_entries == 0
        || spec.palette_entries > 1u32 << spec.depth
    {
        return Err(codec("Invalid BMP RLE source profile"));
    }
    let start = u64::from(spec.pixel_offset);
    let end = if spec.declared_bytes == 0 {
        spec.limit
    } else {
        start
            .checked_add(u64::from(spec.declared_bytes))
            .ok_or_else(|| codec("BMP RLE storage extent"))?
    };
    if end > spec.limit || end > bytes.len() as u64 || start > end {
        return Err(codec("BMP RLE storage overlaps source bounds or profile"));
    }
    let mut input = Input {
        bytes: &bytes[start as usize..end as usize],
        at: 0,
    };
    let (mut x, mut y, mut samples) = (0u32, 0u32, 0u64);
    let maximum = u64::from(spec.width) * u64::from(spec.height);
    loop {
        check()?;
        let pair = input.take(2)?;
        let (count, value) = (pair[0], pair[1]);
        if y == spec.height && (count, value) != (0, 1) {
            return Err(codec("BMP RLE data after final row"));
        }
        if count == 0 {
            match value {
                0 => {
                    y = y
                        .checked_add(1)
                        .ok_or_else(|| codec("BMP RLE row extent"))?;
                    x = 0;
                    continue;
                }
                1 => {
                    let used = input.at as u64;
                    for padding in input.bytes[input.at..].chunks(65536) {
                        // With no declared length these are following structures,
                        // not pixel padding. The exact EOI-delimited span wins.
                        if spec.declared_bytes == 0 {
                            break;
                        }
                        check()?;
                        if padding.iter().any(|&v| v != 0) {
                            return Err(codec("Nonzero BMP RLE trailing storage padding"));
                        }
                    }
                    check()?;
                    return Ok(Span {
                        stream_bytes: used,
                        storage_bytes: if spec.declared_bytes == 0 {
                            used
                        } else {
                            u64::from(spec.declared_bytes)
                        },
                    });
                }
                2 => {
                    let delta = input.take(2)?;
                    x = x
                        .checked_add(u32::from(delta[0]))
                        .ok_or_else(|| codec("BMP RLE horizontal delta extent"))?;
                    y = y
                        .checked_add(u32::from(delta[1]))
                        .ok_or_else(|| codec("BMP RLE vertical delta extent"))?;
                    if x > spec.width || y >= spec.height {
                        return Err(codec("BMP RLE delta outside the bitmap"));
                    }
                    continue;
                }
                _ => {}
            }
        }
        let n = u32::from(if count == 0 { value } else { count });
        let end_x = x
            .checked_add(n)
            .ok_or_else(|| codec("BMP RLE run extent"))?;
        if y >= spec.height || end_x > spec.width {
            return Err(codec("BMP RLE run crosses a row boundary"));
        }
        samples = samples
            .checked_add(u64::from(n))
            .ok_or_else(|| codec("BMP RLE sample extent"))?;
        if samples > maximum {
            return Err(codec("BMP RLE produced sample allowance"));
        }
        if count == 0 {
            let encoded = if spec.depth == 8 {
                n as usize
            } else {
                (n as usize).div_ceil(2)
            };
            let data = input.take(encoded)?;
            for i in 0..n as usize {
                let index = if spec.depth == 8 {
                    data[i]
                } else if i % 2 == 0 {
                    data[i / 2] >> 4
                } else {
                    data[i / 2] & 15
                };
                if u32::from(index) >= spec.palette_entries {
                    return Err(codec("BMP RLE absolute sample exceeds source palette"));
                }
                put(x + i as u32, y, index)?;
            }
            if encoded % 2 != 0 {
                let padding = input.take(1)?[0];
                if spec.depth == 8 && padding != 0 {
                    return Err(codec("Nonzero BMP RLE8 absolute padding"));
                }
            }
        } else {
            for i in 0..n {
                let index = if spec.depth == 8 {
                    value
                } else if i % 2 == 0 {
                    value >> 4
                } else {
                    value & 15
                };
                if u32::from(index) >= spec.palette_entries {
                    return Err(codec("BMP RLE run sample exceeds source palette"));
                }
                put(x + i, y, index)?;
            }
        }
        x = end_x;
    }
}
fn buffer(bytes: u64) -> Result<Vec<u8>> {
    admit_buffer_bytes(bytes, isize::MAX as u64)?;
    let size = usize::try_from(bytes)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "BMP RLE address extent"))?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(size)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "BMP RLE allocation allowance"))?;
    result.resize(size, 0);
    Ok(result)
}
pub(super) fn decode(bytes: &[u8], pixels: u64, check: &dyn Fn() -> Result<()>) -> Result<Decoded> {
    let source = super::bmp_metadata::inspect(bytes, pixels, check)?;
    decode_source(bytes, &source, check)
}
fn decode_source(
    bytes: &[u8],
    source: &Descriptor,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    if !matches!((source.compression, source.bits_per_pixel), (1, 8) | (2, 4)) || source.top_down {
        return Err(unsupported(
            "BMP RLE requires a qualified bottom-up indexed source",
        ));
    }
    let count = u64::from(source.width) * u64::from(source.height);
    let mut decoder =
        ::image::codecs::bmp::BmpDecoder::new(Cursor::new(bytes)).map_err(image_error)?;
    decoder
        .set_limits(decode_limits(count))
        .map_err(image_error)?;
    let palette = decoder
        .get_palette()
        .ok_or_else(|| codec("BMP backend palette missing"))?;
    if decoder.dimensions() != (source.width, source.height)
        || decoder.color_type() != ::image::ColorType::Rgb8
        || palette.get(..source.palette.rgb.len()) != Some(source.palette.rgb.as_slice())
    {
        return Err(codec("BMP RLE backend descriptor/palette mismatch"));
    }
    let mut rgb = buffer(count * 3)?;
    check()?;
    decoder.read_image(&mut rgb).map_err(image_error)?;
    check()?;
    let mut rgba = buffer(count * 4)?;
    let background = source
        .palette
        .rgb
        .first()
        .ok_or_else(|| codec("BMP RLE source palette missing"))?;
    for chunk in rgba.chunks_mut(64 * 1024) {
        check()?;
        for pixel in chunk.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&[background[0], background[1], background[2], 255]);
        }
    }
    let spec = Spec {
        width: source.width,
        height: source.height,
        depth: source.bits_per_pixel,
        palette_entries: source.palette.rgb.len() as u32,
        pixel_offset: source.pixel_offset,
        declared_bytes: source.declared_image_bytes,
        limit: u64::from(source.pixel_offset) + source.pixel_data_bytes,
    };
    let span = scan(bytes, spec, check, |x, bottom_y, index| {
        let pixel =
            u64::from(source.height - 1 - bottom_y) * u64::from(source.width) + u64::from(x);
        let from = (pixel * 3) as usize;
        let to = (pixel * 4) as usize;
        let expected = source.palette.rgb[usize::from(index)];
        if rgb[from..from + 3] != expected {
            return Err(codec("BMP RLE backend authored sample mismatch"));
        }
        rgba[to..to + 3].copy_from_slice(&rgb[from..from + 3]);
        Ok(())
    })?;
    if Some(span.stream_bytes) != source.rle_stream_bytes
        || span.storage_bytes != source.pixel_data_bytes
    {
        return Err(codec("BMP RLE encoded storage changed"));
    }
    check()?;
    Ok(Decoded {
        format: ImageFormat::Bmp,
        width: source.width,
        height: source.height,
        rgba,
        losses: source.losses().into_iter().map(str::to_owned).collect(),
    })
}
#[cfg(test)]
mod tests;

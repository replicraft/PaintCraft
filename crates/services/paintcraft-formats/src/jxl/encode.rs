use super::*;
use std::io::Write;
use zerocopy::IntoBytes;
use zune_core::{bit_depth::BitDepth, colorspace::ColorSpace, options::EncoderOptions};

struct Sink<'a> {
    bytes: Vec<u8>,
    check: &'a dyn Fn() -> Result<()>,
    failure: Option<Error>,
}
impl Write for Sink<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let result = (self.check)().and_then(|()| {
            if self.bytes.len().saturating_add(bytes.len()) > OUTPUT_BYTES_MAX {
                return Err(resource("JPEG XL exceeds the encoded byte limit"));
            }
            self.bytes
                .try_reserve(bytes.len())
                .map_err(|_| resource("JPEG XL output allocation failed"))?;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        });
        result.map_err(|error| {
            self.failure = Some(error);
            std::io::Error::other("JPEG XL output rejected")
        })
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode(
    width: u32,
    height: u32,
    samples: &[u8],
    depth: BitDepth,
    colorspace: ColorSpace,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let count = admit_size(width as usize, height as usize, PIXELS_MAX)?;
    let expected = (count / 4)
        .checked_mul(colorspace.num_components())
        .and_then(|n| n.checked_mul(depth.size_of()))
        .ok_or_else(|| resource("JPEG XL sample size overflow"))?;
    if samples.len() != expected {
        return Err(codec(
            "JPEG XL source sample length does not match dimensions",
        ));
    }
    // This is an upstream 0.5.2 restriction, not a format restriction. Do not
    // pad dimensions or advertise the single-row/column profile as qualified.
    if width <= 1 || height <= 1 {
        return Err(unsupported(
            "JPEG XL encoder does not support single-row or single-column images",
        ));
    }
    let options =
        EncoderOptions::new(width as usize, height as usize, colorspace, depth).set_num_threads(1);
    let mut sink = Sink {
        bytes: Vec::new(),
        check,
        failure: None,
    };
    let result = zune_jpegxl::JxlSimpleEncoder::new(samples, options).encode(&mut sink);
    if let Some(error) = sink.failure {
        return Err(error);
    }
    result.map_err(|e| codec(format!("JPEG XL encode failed: {e:?}")))?;
    check()?;
    Ok(sink.bytes)
}
pub fn encode_rgba8(width: u32, height: u32, samples: &[u8]) -> Result<Vec<u8>> {
    encode_rgba8_checked(width, height, samples, &|| Ok(()))
}
/// Source is immutable straight-alpha sRGB. Encoding is single-threaded on all
/// targets. Cancellation occurs before/after compression and at output writes;
/// the upstream compression loop itself does not expose a cancellation hook.
pub fn encode_rgba8_checked(
    width: u32,
    height: u32,
    samples: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    encode(
        width,
        height,
        samples,
        BitDepth::Eight,
        ColorSpace::RGBA,
        check,
    )
}
pub fn encode_rgba16(width: u32, height: u32, samples: &[u16]) -> Result<Vec<u8>> {
    encode_rgba16_checked(width, height, samples, &|| Ok(()))
}
/// Opaque u16 RGBA is staged as RGB16 in native endian, as required by the
/// upstream encoder. Non-opaque 16-bit alpha is refused because version 0.5.2
/// incorrectly declares its alpha plane as 8-bit.
pub fn encode_rgba16_checked(
    width: u32,
    height: u32,
    samples: &[u16],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let count = admit_size(width as usize, height as usize, PIXELS_MAX)?;
    if samples.len() != count {
        return Err(codec(
            "JPEG XL source sample length does not match dimensions",
        ));
    }
    if samples
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[3] != u16::MAX)
    {
        return Err(unsupported(
            "JPEG XL encoder cannot preserve non-opaque 16-bit alpha",
        ));
    }
    // zune-jpegxl 0.5.2 declares alpha as 8-bit even for RGBA16. The
    // qualified opaque profile encodes RGB16 and reconstructs opaque alpha.
    let mut rgb = allocated::<u16>(count / 4 * 3)?;
    for (source, destination) in samples
        .as_chunks::<4>()
        .0
        .iter()
        .zip(rgb.as_chunks_mut::<3>().0.iter_mut())
    {
        destination.copy_from_slice(&source[..3]);
    }
    encode(
        width,
        height,
        rgb.as_bytes(),
        BitDepth::Sixteen,
        ColorSpace::RGB,
        check,
    )
}

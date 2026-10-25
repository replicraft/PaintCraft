//! Version-5 RGB24 and indexed-8 copies through the selected `pcx` writer.
//! Alpha conversion belongs to the flat-export policy; indexed output retains
//! supplied indices and palette ordering without implicit quantization.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::io::{self, Write};
mod palette;
pub use palette::{Mode, Prepared, Preview, prepare_rgba};

pub const PIXELS_MAX: u64 = 4_194_304;
pub const BYTES_MAX: usize = 64 << 20;

fn geometry(width: u32, height: u32, dpi: (u16, u16)) -> Result<(usize, usize)> {
    // The codec stores the padded bytes-per-plane-row in u16. Width 65535
    // rounds to 65536 and overflows that field inside the upstream writer.
    if width == 0 || width > 65534 || height == 0 || height > 65535 || dpi.0 == 0 || dpi.1 == 0 {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "PCX requires width 1..65534, height 1..65535 and positive DPI",
        ));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > PIXELS_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PCX export exceeds pixel allowance",
        ));
    }
    Ok((width as usize, height as usize))
}

struct Output<'a> {
    bytes: Vec<u8>,
    failure: Option<Error>,
    next_check: usize,
    check: &'a dyn Fn() -> Result<()>,
}
impl<'a> Output<'a> {
    fn new(capacity: usize, check: &'a dyn Fn() -> Result<()>) -> Result<Self> {
        if capacity > BYTES_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "PCX encoded size exceeds allowance",
            ));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Cannot allocate PCX output"))?;
        Ok(Self {
            bytes,
            failure: None,
            next_check: 0,
            check,
        })
    }
    fn finish(self, result: io::Result<()>) -> Result<Vec<u8>> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        result.map_err(|_| Error::new(ErrorCode::CodecFailed, "PCX writer failed"))?;
        (self.check)()?;
        Ok(self.bytes)
    }
}
impl Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            if self.bytes.len() >= self.next_check {
                (self.check)()?;
                self.next_check = self.bytes.len().saturating_add(65536);
            }
            let end = self
                .bytes
                .len()
                .checked_add(bytes.len())
                .filter(|end| *end <= self.bytes.capacity() && *end <= BYTES_MAX)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "PCX writer exceeded reserved output",
                    )
                })?;
            self.bytes.extend_from_slice(bytes);
            debug_assert_eq!(self.bytes.len(), end);
            Ok(bytes.len())
        })();
        result.map_err(|error| {
            self.failure.get_or_insert(error);
            io::Error::other("PCX output admission failed")
        })
    }
    fn flush(&mut self) -> io::Result<()> {
        (self.check)().map_err(|error| {
            self.failure.get_or_insert(error);
            io::Error::other("PCX output cancelled")
        })
    }
}

/// Encode exact RGB samples. The caller supplies its explicit alpha matte policy.
pub fn encode_rgb(
    width: u32,
    height: u32,
    rgb: &[u8],
    dpi: (u16, u16),
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let (w, h) = geometry(width, height, dpi)?;
    if rgb.len() != w * h * 3 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "PCX RGB input length does not match dimensions",
        ));
    }
    let mut output = Output::new(128 + (w + (w & 1)) * h * 3 * 2, check)?;
    let result = (|| {
        let mut writer = ::pcx::WriterRgb::new(&mut output, (width as u16, height as u16), dpi)?;
        for row in rgb.chunks_exact(w * 3) {
            if let Err(error) = check() {
                return Err(io::Error::other(error));
            }
            writer.write_row(row)?;
        }
        writer.finish()
    })();
    // Row cancellation is carried by io::Error's typed source, rather than
    // reclassifying it as a codec failure or retrying the write.
    if output.failure.is_none()
        && let Err(error) = &result
        && let Some(source) = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<Error>())
    {
        output.failure = Some(source.clone());
    }
    output.finish(result)
}

/// Encode exact index bytes and palette order, including duplicate RGB entries.
/// An index must reference a supplied entry. The unused palette suffix is black.
pub fn encode_indexed(
    width: u32,
    height: u32,
    indices: &[u8],
    palette: &[[u8; 3]],
    dpi: (u16, u16),
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let (w, h) = geometry(width, height, dpi)?;
    if indices.len() != w * h
        || palette.is_empty()
        || palette.len() > 256
        || indices
            .iter()
            .any(|index| usize::from(*index) >= palette.len())
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "PCX indices or palette do not match the declared image",
        ));
    }
    let mut output = Output::new(128 + (w + (w & 1)) * h * 2 + 769, check)?;
    let result = (|| {
        let mut writer =
            ::pcx::WriterPaletted::new(&mut output, (width as u16, height as u16), dpi)?;
        for row in indices.chunks_exact(w) {
            if let Err(error) = check() {
                return Err(io::Error::other(error));
            }
            writer.write_row(row)?;
        }
        writer.write_palette(palette.as_flattened())
    })();
    if output.failure.is_none()
        && let Err(error) = &result
        && let Some(source) = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<Error>())
    {
        output.failure = Some(source.clone());
    }
    output.finish(result)
}

/// Flat UI export profile: RGB24 with transparency composited against white.
pub(crate) fn encode_rgba(
    width: u32,
    height: u32,
    rgba: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let (w, h) = geometry(width, height, (72, 72))?;
    if rgba.len() != w * h * 4 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "PCX RGBA input length does not match dimensions",
        ));
    }
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(w * h * 3)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Cannot allocate PCX matte pixels"))?;
    for row in rgba.chunks_exact(w * 4) {
        check()?;
        for px in row.as_chunks::<4>().0 {
            let alpha = u32::from(px[3]);
            rgb.extend(px[..3].iter().map(|sample| {
                ((u32::from(*sample) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
            }));
        }
    }
    encode_rgb(width, height, &rgb, (72, 72), check)
}

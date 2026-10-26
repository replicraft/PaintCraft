//! Historical Windows Paint monochrome rasters, distinct from modern `.paint`.
//! Internal qualified profile; no public registry or frontend activation here.
mod decode;
mod encode;
#[cfg(test)]
mod tests;
pub use decode::{decode, inspect};
pub use encode::{encode_v1, encode_v1_rgba_exact};
use paintcraft_core::types::{Error, ErrorCode, Result};
pub const BYTES_MAX: usize = 16 << 20;
pub const PIXELS_MAX: u64 = 4_194_304;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    UncompressedV1,
    RowRleV2,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    version: Version,
    width: u16,
    height: u16,
    words: [u16; 16],
}
impl Header {
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn width(&self) -> u32 {
        u32::from(self.width)
    }
    pub fn height(&self) -> u32 {
        u32::from(self.height)
    }
    /// Exact original words, including checksum/device/reserved metadata.
    /// Geometry uses words2/3 only; other fields have no resource effects.
    pub fn original_words(&self) -> &[u16; 16] {
        &self.words
    }
    pub fn row_bytes(&self) -> usize {
        usize::from(self.width).div_ceil(8)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    header: Header,
    packed: Vec<u8>,
}
impl Bitmap {
    pub fn header(&self) -> &Header {
        &self.header
    }
    /// Original row order and MSB-first bits; unused final-byte bits retained.
    pub fn packed(&self) -> &[u8] {
        &self.packed
    }
    /// Explicit expansion only: bit0 black, bit1 white, always opaque. No
    /// scaling, palette, color management, threshold or alpha inference.
    pub fn rgba(&self, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
        check()?;
        let pixels = usize::from(self.header.width) * usize::from(self.header.height);
        let mut rgba = buffer(pixels * 4)?;
        for y in 0..usize::from(self.header.height) {
            check()?;
            let row = &self.packed[y * self.header.row_bytes()..(y + 1) * self.header.row_bytes()];
            for x in 0..usize::from(self.header.width) {
                let value = if row[x / 8] & (0x80 >> (x % 8)) == 0 {
                    0
                } else {
                    255
                };
                rgba.extend([value, value, value, 255]);
            }
        }
        check()?;
        Ok(rgba)
    }
}
fn geometry(width: u32, height: u32) -> Result<(usize, usize)> {
    if width == 0 || width > 65535 || height == 0 || height > 65535 {
        return Err(bad("MSP dimensions must be 1..65535"));
    }
    if u64::from(width) * u64::from(height) > PIXELS_MAX {
        return Err(limit("MSP pixel allowance exceeded"));
    }
    Ok((width as usize, (width as usize).div_ceil(8)))
}
fn buffer(capacity: usize) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| limit("cannot allocate MSP buffer"))?;
    Ok(result)
}
fn bad(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn limit(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}

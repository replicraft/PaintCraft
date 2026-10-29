//! Read-only original-source facts. This does not decode or authorize adoption.
use crate::image::{self, bmp_metadata, jpeg, png_metadata};
use paintcraft_core::types::{Error, ErrorCode, Result};
pub const INPUT_BYTES_MAX: u64 = 1 << 30;
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub input_bytes: u64,
    pub pixels: u64,
    /// Optional lower adapter-specific metadata/ICC allowance. This is not an
    /// aggregate report allocation or RSS budget; BMP's fixed palette is separate.
    pub metadata_bytes: Option<u64>,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            input_bytes: INPUT_BYTES_MAX,
            pixels: image::DECODE_PIXELS_MAX,
            metadata_bytes: None,
        }
    }
}
impl Limits {
    pub fn input_limit(self) -> u64 {
        self.input_bytes.min(INPUT_BYTES_MAX)
    }
    pub fn pixel_limit(self) -> u64 {
        self.pixels.min(image::DECODE_PIXELS_MAX)
    }
    pub fn effective_metadata_limit(self, source: &Source) -> u64 {
        self.metadata_limit(match source {
            Source::Png(_) => self.pixel_limit().saturating_mul(8).min(isize::MAX as u64),
            Source::Bmp(s) => s.source.bytes,
            Source::Jpeg(_) => INPUT_BYTES_MAX,
        })
    }
    fn metadata_limit(self, default: u64) -> u64 {
        self.metadata_bytes.map_or(default, |n| n.min(default))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Png(png_metadata::Descriptor),
    Bmp(bmp_metadata::Descriptor),
    Jpeg(jpeg::Source),
}
impl Source {
    pub fn format(&self) -> &'static str {
        match self {
            Self::Png(_) => "png",
            Self::Bmp(_) => "bmp",
            Self::Jpeg(_) => "jpeg",
        }
    }
    pub fn source_sha256(&self) -> [u8; 32] {
        match self {
            Self::Png(s) => s.source.sha256,
            Self::Bmp(s) => s.source.sha256,
            Self::Jpeg(s) => s.source_sha256(),
        }
    }
    pub fn source_bytes(&self) -> u64 {
        match self {
            Self::Png(s) => s.source.bytes,
            Self::Bmp(s) => s.source.bytes,
            Self::Jpeg(s) => s.source_bytes(),
        }
    }
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Png(s) => (s.width, s.height),
            Self::Bmp(s) => (s.width, s.height),
            Self::Jpeg(s) => s.dimensions(),
        }
    }
    pub fn proposed_dimensions(&self) -> (u32, u32) {
        match self {
            Self::Jpeg(s) => s.oriented_dimensions(),
            _ => self.dimensions(),
        }
    }
    /// Header-derived proposed conversion losses, not a final decoder receipt.
    /// Invalid entropy/samples may still be rejected by the actual import.
    pub fn proposed_losses(&self) -> Vec<String> {
        match self {
            Self::Jpeg(s) => s.losses(),
            Self::Bmp(s) => s.losses().into_iter().map(str::to_owned).collect(),
            Self::Png(s) => {
                let mut result = Vec::new();
                if s.bit_depth == 16 {
                    result.push("depth-16-reduced-to-8".into());
                }
                result.extend(s.losses().into_iter().map(str::to_owned));
                result
            }
        }
    }
}
/// Inspect only PNG, BMP and JPEG using their existing bounded source readers.
/// No fallback decoder, document mutation, import ticket or CMS is implied.
pub fn inspect(bytes: &[u8], limits: Limits, check: &dyn Fn() -> Result<()>) -> Result<Source> {
    check()?;
    if bytes.len() as u64 > limits.input_limit()
        || bytes.len() > isize::MAX as usize
        || limits.pixel_limit() == 0
    {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Source inspection input/pixel allowance",
        ));
    }
    let source = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let default = limits
            .pixel_limit()
            .saturating_mul(8)
            .min(isize::MAX as u64);
        Source::Png(png_metadata::inspect_with_metadata_limit(
            bytes,
            limits.pixel_limit(),
            limits.metadata_limit(default),
            check,
        )?)
    } else if bytes.starts_with(b"BM") {
        Source::Bmp(bmp_metadata::inspect_with_metadata_limit(
            bytes,
            limits.pixel_limit(),
            limits.metadata_limit(bytes.len() as u64),
            check,
        )?)
    } else if bytes.starts_with(b"\xff\xd8") {
        Source::Jpeg(jpeg::inspect_with_limits(
            bytes,
            jpeg::Limits {
                input_bytes: limits.input_limit(),
                pixels: limits.pixel_limit(),
                metadata_bytes: limits.metadata_limit(INPUT_BYTES_MAX),
            },
            check,
        )?)
    } else {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source inspection currently supports PNG, BMP and JPEG only",
        ));
    };
    check()?;
    Ok(source)
}
#[cfg(test)]
mod tests;

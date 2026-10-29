//! Explicit selected-page TIFF conversion. Other images remain in the source.
//! This is not a multipage document model or a foreign preserving Save route.
use paintcraft_core::types::{Error, ErrorCode, Result};
use sha2::{Digest, Sha256};

pub const BYTES_MAX: usize = 64 << 20;
pub const PIXELS_MAX: u64 = 4_194_304;
pub const PAGES_MAX: usize = 1024;
pub const TAGS_PER_PAGE_MAX: usize = 4096;
pub const TAGS_TOTAL_MAX: usize = 65_536;
pub const METADATA_PER_PAGE_MAX: usize = 1 << 20;
pub const METADATA_BYTES_MAX: usize = 4 << 20;
pub const IMPORT_VARIANT: &str = "tiff-selected-page-rgba8-v1";
pub const LOSS_OTHER_PAGES: &str = "tiff-other-pages-not-imported";
pub const CONVERSION_POLICY: &str = "tiff-uint16-high-byte-rgba8-v1";
pub const ASSESSMENT_BYTES: usize = 256;
mod assessment;
pub mod browser_wire;
mod gray_alpha;
mod inspect;
mod view;
pub use assessment::Assessment;
pub use inspect::inspect;

/// Unsupported arrays retain their declared count and first four values; the
/// complete immutable source fingerprint covers the remaining values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Components {
    pub count: u32,
    pub values: [u32; 4],
}
impl Components {
    const fn single(value: u32) -> Self {
        Self {
            count: 1,
            values: [value, 0, 0, 0],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub ifd_offset: u64,
    pub little_endian: bool,
    /// 1 = strips, 2 = tiles; other values describe unqualified inventories.
    pub storage: u8,
    pub bits_per_sample: Components,
    pub sample_format: Components,
    pub extra_samples: Components,
    pub photometric: u32,
    pub samples_per_pixel: u32,
    pub compression: u32,
    pub orientation: u32,
    pub planar_configuration: u32,
    pub icc_bytes: u32,
    pub icc_sha256: [u8; 32],
    pub metadata_bytes: u32,
    pub omitted_metadata: bool,
    pub resolution_x: Option<(u32, u32)>,
    pub resolution_y: Option<(u32, u32)>,
    pub resolution_unit: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub index: u16,
    pub width: u32,
    pub height: u32,
    pub descriptor: Descriptor,
    pub unsupported_reason: Option<&'static str>,
}
#[derive(Debug)]
pub struct Inventory {
    pages: Vec<Page>,
    source_sha256: [u8; 32],
    source_bytes: u64,
}
impl Inventory {
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }
    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
}
#[derive(Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub losses: Vec<String>,
    pub assessment: Assessment,
}
fn malformed(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn unsupported(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn hash_checked(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(65_536) {
        check()?;
        hash.update(chunk);
    }
    check()?;
    Ok(hash.finalize().into())
}
/// Re-inventory the same full source before resolving a page. No caller-created
/// descriptor, stale page offset or changed byte buffer grants decode authority.
pub fn decode_page(
    bytes: &[u8],
    index: u16,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    let inventory = inspect(bytes, check)?;
    let selected = inventory.pages.get(usize::from(index)).ok_or_else(|| {
        Error::new(
            ErrorCode::InputSchema,
            "TIFF selected page is outside the directory",
        )
    })?;
    if let Some(reason) = selected.unsupported_reason {
        return Err(unsupported(reason));
    }
    let pixels = pixels.min(PIXELS_MAX);
    if pixels == 0 || u64::from(selected.width) * u64::from(selected.height) > pixels {
        return Err(resource("TIFF selected page exceeds the pixel allowance"));
    }
    let losses = crate::image::tiff::admit_page(
        bytes,
        selected.descriptor.ifd_offset as usize,
        pixels,
        check,
    )?;
    let result = if selected.descriptor.samples_per_pixel == 2
        && matches!(selected.descriptor.photometric, 0 | 1)
    {
        gray_alpha::decode(bytes, selected, losses, check)
    } else {
        let reader = view::Selected::new(bytes, selected.descriptor, check)?;
        // The output is already checked against the caller's limit. A small
        // image may still require an admitted padded tile up to this profile's
        // working-buffer ceiling, independent of its final cropped dimensions.
        crate::image::decode_tiff_selected(reader, PIXELS_MAX, losses)
    };
    check()?;
    let mut image = result?;
    if (image.width, image.height) != (selected.width, selected.height) {
        return Err(malformed("TIFF selected decoder geometry changed"));
    }
    if inventory.pages.len() > 1 {
        image.losses.push(LOSS_OTHER_PAGES.into());
    }
    let assessment = Assessment::new(&inventory, selected, &image.rgba, &image.losses, check)?;
    Ok(Decoded {
        width: image.width,
        height: image.height,
        rgba: image.rgba,
        losses: image.losses,
        assessment,
    })
}

#[cfg(test)]
mod hostile_tests;
#[cfg(test)]
mod independent_tests;
#[cfg(test)]
mod tests;

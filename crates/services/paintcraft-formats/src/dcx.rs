//! Bounded DCX directory admission and explicit selected-page conversion.
//! DCX contains flat PCX pages; it is not a layered project or an animation.
use crate::image::{self, Decoded};
use paintcraft_core::types::{Error, ErrorCode, Result};

pub const DIRECTORY_BYTES: usize = 4 + 1024 * 4;
pub const MAGIC: u32 = 987_654_321;
pub const PAGES_MAX: usize = 1023;
pub const EXPORT_VARIANT: &str = "dcx-fresh-single-pcx-page";
mod prepare;
mod write;
pub use prepare::{DocumentLimits, PreparedDocument, prepare_document, prepare_document_with};
pub use write::{
    PagePixels, WriteLimits, WritePage, WritePlan, encode_pages, preflight_write, write_pages_to,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    /// Zero-based page index.
    pub index: u16,
    pub offset: u64,
    pub length: u64,
    pub width: u32,
    pub height: u32,
}
fn malformed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}

/// Inspect every page's selected PCX profile without allocating decoded pixels.
/// Offsets must be ascending, exactly terminated, and cover complete PCX payloads.
pub fn inspect(bytes: &[u8]) -> Result<Vec<Page>> {
    if bytes.len() as u64 > 1 << 30 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "DCX exceeds encoded input budget",
        ));
    }
    let header = bytes
        .get(..DIRECTORY_BYTES)
        .ok_or_else(|| malformed("truncated DCX directory"))?;
    if u32::from_le_bytes(header[..4].try_into().unwrap()) != MAGIC {
        return Err(malformed("invalid DCX signature"));
    }
    let mut offsets = [0usize; 1024];
    let mut count = 0usize;
    let mut ended = false;
    for entry in header[4..].as_chunks::<4>().0 {
        let offset = u32::from_le_bytes(*entry) as usize;
        if offset == 0 {
            ended = true;
            continue;
        }
        if ended {
            return Err(malformed(
                "DCX has nonzero entries after directory terminator",
            ));
        }
        if offset < DIRECTORY_BYTES
            || offset >= bytes.len()
            || count > 0 && offset <= offsets[count - 1]
        {
            return Err(malformed(
                "DCX page offsets must be ascending and inside the file",
            ));
        }
        offsets[count] = offset;
        count += 1;
    }
    if !ended || count > PAGES_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "DCX page count or directory terminator exceeds supported profile",
        ));
    }
    if count == 0 {
        return Err(malformed("DCX contains no pages"));
    }
    if offsets[0] != DIRECTORY_BYTES {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "DCX pre-page extensions are not qualified",
        ));
    }
    let mut pages = Vec::new();
    pages.try_reserve_exact(count).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "cannot allocate DCX page directory",
        )
    })?;
    for index in 0..count {
        let start = offsets[index];
        let end = if index + 1 < count {
            offsets[index + 1]
        } else {
            bytes.len()
        };
        let (width, height) = image::inspect_pcx(&bytes[start..end], image::DECODE_PIXELS_MAX)
            .map_err(|error| {
                Error::new(error.code, format!("DCX page {index}: {}", error.message))
            })?;
        pages.push(Page {
            index: index as u16,
            offset: start as u64,
            length: (end - start) as u64,
            width,
            height,
        });
    }
    Ok(pages)
}

/// Decode one explicitly selected zero-based page. Never select page zero implicitly.
/// The caller must acknowledge reported conversion losses before publishing pixels.
pub fn decode_page(bytes: &[u8], page: u16, pixels: u64) -> Result<Decoded> {
    let pages = inspect(bytes)?;
    let selected = pages.get(usize::from(page)).ok_or_else(|| {
        Error::new(
            ErrorCode::InputSchema,
            "DCX selected page is outside the directory",
        )
    })?;
    let start = selected.offset as usize;
    let end = start + selected.length as usize;
    let mut decoded = image::decode_with_pixel_limit(&bytes[start..end], pixels)?;
    if pages.len() > 1 {
        decoded.losses.push("dcx-other-pages-not-imported".into());
    }
    Ok(decoded)
}

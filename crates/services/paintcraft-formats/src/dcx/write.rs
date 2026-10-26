//! Ordered independent flat PCX pages, never a document-page or animation model.
use super::*;
use std::io::Write;
#[derive(Clone, Copy, Debug)]
pub enum PagePixels<'a> {
    Rgb(&'a [u8]),
    Indexed {
        indices: &'a [u8],
        palette: &'a [[u8; 3]],
    },
}
#[derive(Clone, Copy, Debug)]
pub struct WritePage<'a> {
    pub width: u32,
    pub height: u32,
    pub dpi: (u16, u16),
    pub pixels: PagePixels<'a>,
}
#[derive(Clone, Copy, Debug)]
pub struct WriteLimits {
    pub pages: usize,
    pub pixels: u64,
    pub encoded_bytes: usize,
    pub transient_bytes: usize,
}
impl Default for WriteLimits {
    fn default() -> Self {
        Self {
            pages: PAGES_MAX,
            pixels: crate::pcx::PIXELS_MAX,
            encoded_bytes: 64 << 20,
            transient_bytes: 128 << 20,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WritePlan {
    pub pages: usize,
    pub pixels: u64,
    pub source_bytes: usize,
    pub output_capacity: usize,
    pub largest_page_capacity: usize,
    pub transient_bytes: usize,
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource("DCX write size overflow"))
}
/// Validate every page, including the last, before any encoded/output allocation.
pub fn preflight_write(
    pages: &[WritePage<'_>],
    limits: &WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WritePlan> {
    check()?;
    if pages.is_empty() || pages.len() > limits.pages.min(PAGES_MAX) {
        return Err(resource("DCX writer requires 1..1023 admitted pages"));
    }
    let mut pixels = 0u64;
    let mut source_bytes = 0usize;
    let mut capacity = DIRECTORY_BYTES;
    let mut largest = 0usize;
    for page in pages {
        check()?;
        let indexed = matches!(page.pixels, PagePixels::Indexed { .. });
        let bound = crate::pcx::encoded_capacity(page.width, page.height, page.dpi, indexed)?;
        let count = u64::from(page.width) * u64::from(page.height);
        pixels = pixels
            .checked_add(count)
            .ok_or_else(|| resource("DCX aggregate pixel overflow"))?;
        if pixels > limits.pixels.min(crate::pcx::PIXELS_MAX) {
            return Err(resource("DCX aggregate page pixel allowance"));
        }
        let count =
            usize::try_from(count).map_err(|_| resource("DCX pixels exceed address space"))?;
        match page.pixels {
            PagePixels::Rgb(rgb) => {
                if rgb.len() != count * 3 {
                    return Err(invalid("DCX RGB page length disagrees with geometry"));
                }
                source_bytes = add(source_bytes, rgb.len())?;
            }
            PagePixels::Indexed { indices, palette } => {
                if indices.len() != count || palette.is_empty() || palette.len() > 256 {
                    return Err(invalid(
                        "DCX indexed page or palette disagrees with geometry",
                    ));
                }
                for chunk in indices.chunks(65536) {
                    check()?;
                    if chunk
                        .iter()
                        .any(|index| usize::from(*index) >= palette.len())
                    {
                        return Err(invalid("DCX index references an absent page palette entry"));
                    }
                }
                source_bytes = add(source_bytes, add(indices.len(), palette.len() * 3)?)?;
            }
        }
        capacity = add(capacity, bound)?;
        largest = largest.max(bound);
        if capacity > limits.encoded_bytes.min(64 << 20) || u32::try_from(capacity).is_err() {
            return Err(resource("DCX aggregate encoded/offset allowance"));
        }
    }
    // Selected pcx0.2.5 streams each plane through a fixed scalar RLE state;
    // no pixel/row clone or heap codec cache. Reserve metadata/stack headroom.
    let metadata = add(DIRECTORY_BYTES, std::mem::size_of_val(pages))?;
    let transient = add(
        add(source_bytes, capacity)?,
        add(largest, add(metadata, 4096)?)?,
    )?;
    if transient > limits.transient_bytes {
        return Err(resource("DCX transient page/output allowance"));
    }
    Ok(WritePlan {
        pages: pages.len(),
        pixels,
        source_bytes,
        output_capacity: capacity,
        largest_page_capacity: largest,
        transient_bytes: transient,
    })
}
/// Encode in source order with one page buffer alive at a time. Palette order,
/// duplicate entries and exact indices are retained; unused palette suffix is black.
pub fn encode_pages(
    pages: &[WritePage<'_>],
    limits: &WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let plan = preflight_write(pages, limits, check)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(plan.output_capacity)
        .map_err(|_| resource("Cannot allocate DCX private output"))?;
    output.resize(DIRECTORY_BYTES, 0);
    output[..4].copy_from_slice(&MAGIC.to_le_bytes());
    for (index, page) in pages.iter().enumerate() {
        check()?;
        let offset =
            u32::try_from(output.len()).map_err(|_| resource("DCX page offset exceeds u32"))?;
        let encoded = match page.pixels {
            PagePixels::Rgb(rgb) => {
                crate::pcx::encode_rgb(page.width, page.height, rgb, page.dpi, check)?
            }
            PagePixels::Indexed { indices, palette } => crate::pcx::encode_indexed(
                page.width,
                page.height,
                indices,
                palette,
                page.dpi,
                check,
            )?,
        };
        if add(output.len(), encoded.len())? > plan.output_capacity {
            return Err(resource("DCX page exceeded admitted aggregate output"));
        }
        output[4 + index * 4..8 + index * 4].copy_from_slice(&offset.to_le_bytes());
        for chunk in encoded.chunks(65536) {
            check()?;
            output.extend_from_slice(chunk);
        }
    }
    check()?;
    Ok(output)
}
/// Private staged sink only; callers retain source and atomic-publication authority.
pub fn write_pages_to(
    pages: &[WritePage<'_>],
    limits: &WriteLimits,
    sink: &mut impl Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let bytes = encode_pages(pages, limits, check)?;
    for chunk in bytes.chunks(65536) {
        check()?;
        sink.write_all(chunk)
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "Cannot write staged DCX copy"))?;
    }
    check()
}

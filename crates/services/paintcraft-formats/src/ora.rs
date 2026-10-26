//! Bounded conversion of flat OpenRaster PNG layer stacks.
//! Unknown group/blend/transform semantics are refused; foreign preserving Save
//! is a separate capability and is not supplied by this adapter.
mod archive;
mod stack;
mod write;
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::borrow::Cow;
pub use write::{TITLE_LOSS, encode_document, preflight, write_document_to};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub archive_bytes: usize,
    pub entries: usize,
    pub inflated_bytes: usize,
    pub entry_bytes: usize,
    pub metadata_bytes: usize,
    pub xml_bytes: usize,
    pub attribute_bytes: usize,
    pub attributes: usize,
    pub depth: usize,
    pub path_bytes: usize,
    pub layers: usize,
    pub dimension: u32,
    pub pixels: u64,
    pub decoded_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            archive_bytes: 256 << 20,
            entries: 4096,
            inflated_bytes: 512 << 20,
            entry_bytes: 128 << 20,
            metadata_bytes: 8 << 20,
            xml_bytes: 4 << 20,
            attribute_bytes: 16_384,
            attributes: 16,
            depth: 8,
            path_bytes: 1024,
            layers: 1024,
            dimension: 1_048_576,
            pixels: 67_108_864,
            decoded_bytes: 256 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedLayer {
    pub name: String,
    pub origin: (i32, i32),
    pub visible: bool,
    pub opacity: u8,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub title: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub layers: Vec<DecodedLayer>,
    pub losses: Vec<String>,
}
/// Backend-only group conversion loss; not admitted by generic frontend imports.
pub const GROUP_STRUCTURE_LOSS: &str = "ora-group-structure-not-retained";
pub(crate) struct GroupCopy {
    pub decoded: Decoded,
    pub merged: Option<Vec<u8>>,
}
pub const LOSSES: [&str; 2] = ["ora-opacity-quantized", "ora-resolution-ignored"];

/// A dispatch hint only. Full ZIP, XML and PNG admission remains mandatory.
pub(crate) fn has_signature(bytes: &[u8]) -> bool {
    if bytes.get(..4) != Some(b"PK\x03\x04") || bytes.get(8..10) != Some(&[0, 0]) {
        return false;
    }
    let Some(header) = bytes.get(..30) else {
        return false;
    };
    let name_len = usize::from(u16::from_le_bytes([header[26], header[27]]));
    let extra_len = usize::from(u16::from_le_bytes([header[28], header[29]]));
    let start = 30 + name_len + extra_len;
    name_len == 8
        && bytes.get(30..38) == Some(b"mimetype")
        && bytes.get(start..start + 16) == Some(b"image/openraster")
}
fn malformed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
struct Budget {
    used: std::cell::Cell<usize>,
    limit: usize,
}
impl Budget {
    fn new(limit: usize) -> Self {
        Self {
            used: std::cell::Cell::new(0),
            limit,
        }
    }
    fn charge<T>(&self, n: usize) -> Result<()> {
        let total = n
            .checked_mul(std::mem::size_of::<T>())
            .and_then(|bytes| self.used.get().checked_add(bytes))
            .ok_or_else(|| resource("ORA metadata size overflow"))?;
        if total > self.limit {
            return Err(resource("ORA metadata capacity allowance exceeded"));
        }
        self.used.set(total);
        Ok(())
    }
    fn vector<T>(&self, n: usize) -> Result<Vec<T>> {
        self.charge::<T>(n)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(n)
            .map_err(|_| resource("Cannot allocate ORA metadata"))?;
        Ok(values)
    }
    fn text(&self, value: &str) -> Result<String> {
        self.charge::<u8>(value.len())?;
        let mut text = String::new();
        text.try_reserve_exact(value.len())
            .map_err(|_| resource("Cannot allocate ORA text"))?;
        text.push_str(value);
        Ok(text)
    }
}
fn dimensions(width: u32, height: u32, limits: &Limits) -> Result<u64> {
    if width == 0 || height == 0 {
        return Err(malformed("ORA dimensions must be positive"));
    }
    let pixels = u64::from(width) * u64::from(height);
    if width > limits.dimension
        || height > limits.dimension
        || pixels > limits.pixels
        || pixels
            .checked_mul(4)
            .is_none_or(|bytes| bytes > limits.decoded_bytes as u64)
    {
        return Err(resource("ORA dimensions exceed the pixel/byte allowance"));
    }
    Ok(pixels)
}
fn png_header(bytes: &[u8], limits: &Limits) -> Result<(u32, u32, u8, u8)> {
    let header = bytes
        .get(..33)
        .ok_or_else(|| malformed("Truncated ORA layer PNG header"))?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[8..16] != b"\0\0\0\rIHDR" {
        return Err(malformed("ORA layers must be PNG images"));
    }
    if crc32fast::hash(&header[12..29]) != u32::from_be_bytes(header[29..33].try_into().unwrap()) {
        return Err(malformed("ORA PNG header CRC mismatch"));
    }
    let width = u32::from_be_bytes(header[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(header[20..24].try_into().unwrap());
    dimensions(width, height, limits)?;
    Ok((width, height, header[24], header[28]))
}
fn loss(losses: &mut Vec<String>, code: &str) {
    if !losses.iter().any(|old| old == code) {
        losses.push(code.into());
    }
}

pub fn decode_layers(
    bytes: &[u8],
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    Ok(decode_policy(bytes, limits, false, check)?.decoded)
}
pub(crate) fn decode_neutral_groups(
    bytes: &[u8],
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<GroupCopy> {
    decode_policy(bytes, limits, true, check)
}
fn decode_policy(
    bytes: &[u8],
    limits: &Limits,
    allow_groups: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<GroupCopy> {
    check()?;
    let budget = Budget::new(limits.metadata_bytes);
    let archive = archive::Archive::parse(bytes, limits, &budget, check)?;
    if archive.entries[0].method != 0 || archive.entries[0].uncompressed != 16 {
        return Err(malformed(
            "ORA mimetype must be stored with its exact content size",
        ));
    }
    let mime = archive.read(0, limits, check)?;
    if archive.entries[0].name != "mimetype"
        || archive.entries[0].method != 0
        || mime.as_ref() != b"image/openraster"
    {
        return Err(malformed("ORA requires first stored mimetype entry"));
    }
    let stack_index = archive.index("stack.xml")?;
    if archive.entries[stack_index].uncompressed > limits.xml_bytes {
        return Err(resource("ORA XML exceeds byte allowance"));
    }
    let xml = archive.read(stack_index, limits, check)?;
    let mut stack = if allow_groups {
        stack::parse_neutral_groups(&xml, limits, &budget, check)?
    } else {
        stack::parse(&xml, limits, &budget, check)?
    };
    let merged = archive.index("mergedimage.png")?;
    let thumbnail = archive.index("Thumbnails/thumbnail.png")?;
    let mut used = budget.vector(archive.entries.len())?;
    used.resize(archive.entries.len(), false);
    for index in [0, stack_index, merged, thumbnail] {
        used[index] = true;
    }
    let mut sources = budget.vector(stack.layers.len())?;
    for layer in &stack.layers {
        check()?;
        archive::safe_path(&layer.src, limits, false)?;
        if [
            "mimetype",
            "stack.xml",
            "mergedimage.png",
            "Thumbnails/thumbnail.png",
        ]
        .contains(&layer.src.as_str())
        {
            return Err(unsupported(
                "ORA previews cannot be used as editable layer sources",
            ));
        }
        let index = archive.index(&layer.src)?;
        if archive.entries[index].directory {
            return Err(malformed("ORA layer source is a directory"));
        }
        if !sources.contains(&index) {
            sources.push(index);
        }
        used[index] = true;
    }
    for (index, entry) in archive.entries.iter().enumerate() {
        check()?;
        if !used[index] && !entry.directory {
            return Err(unsupported(
                "ORA contains unqualified unreferenced resources",
            ));
        }
        if entry.directory {
            archive.read(index, limits, check)?;
        }
    }
    let merged_bytes = archive.read(merged, limits, check)?;
    let (w, h, depth, _) = png_header(&merged_bytes, limits)?;
    if (w, h) != (stack.width, stack.height) || !matches!(depth, 8 | 16) {
        return Err(malformed("ORA merged preview disagrees with the canvas"));
    }

    let thumb = archive.read(thumbnail, limits, check)?;
    let (w, h, depth, interlace) = png_header(&thumb, limits)?;
    if w > 256 || h > 256 || depth != 8 || interlace != 0 {
        return Err(malformed("ORA thumbnail exceeds the baseline PNG profile"));
    }

    let thumbnail_pixels = u64::from(w) * u64::from(h);
    let mut encoded: Vec<(usize, Cow<'_, [u8]>)> = budget.vector(sources.len())?;
    for index in sources {
        check()?;
        encoded.push((index, archive.read(index, limits, check)?));
    }
    let mut aggregate = 0u64;
    let mut plans = budget.vector(stack.layers.len())?;
    for layer in &stack.layers {
        check()?;
        let index = archive.index(&layer.src)?;
        let encoded_index = encoded
            .iter()
            .position(|(id, _)| *id == index)
            .ok_or_else(|| malformed("Missing ORA layer source"))?;
        let (width, height, _, _) = png_header(&encoded[encoded_index].1, limits)?;
        aggregate = aggregate
            .checked_add(u64::from(width) * u64::from(height))
            .ok_or_else(|| resource("ORA layer pixel count overflow"))?;
        if aggregate > limits.pixels
            || aggregate
                .checked_mul(4)
                .is_none_or(|bytes| bytes > limits.decoded_bytes as u64)
        {
            return Err(resource("ORA aggregate layer pixels exceed allowance"));
        }
        plans.push((encoded_index, width, height));
    }
    if stack.groups > 0 {
        let comparison = u64::from(stack.width)
            .checked_mul(u64::from(stack.height))
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| n.checked_add(aggregate.checked_mul(4)?));
        let preview_peak = u64::from(stack.width)
            .checked_mul(u64::from(stack.height))
            .and_then(|n| n.checked_add(thumbnail_pixels))
            .and_then(|n| n.checked_mul(4));
        if comparison.is_none_or(|n| n > limits.decoded_bytes as u64)
            || preview_peak.is_none_or(|n| n > limits.decoded_bytes as u64)
        {
            return Err(resource(
                "ORA group layers/merged/render pixels exceed decoded allowance",
            ));
        }
    }
    let mut projection = None;
    // All canonical layer geometry is admitted before even temporary preview
    // pixel buffers are created. Preview decoding is sequential and discarded.
    for (index, preview) in [&merged_bytes, &thumb].into_iter().enumerate() {
        check()?;
        let decoded = crate::image::decode_with_pixel_limit(preview, limits.pixels)?;
        if decoded
            .losses
            .iter()
            .any(|code| code == "animation-first-frame-only")
        {
            return Err(unsupported("ORA previews must be static PNG images"));
        }
        if stack.groups > 0 && index == 0 {
            projection = Some(decoded.rgba);
        }
        check()?;
    }
    drop(merged_bytes);
    drop(thumb);
    let mut layers = budget.vector(stack.layers.len())?;
    for (layer, (index, width, height)) in stack.layers.drain(..).zip(plans) {
        check()?;
        let image = crate::image::decode_with_pixel_limit(&encoded[index].1, limits.pixels)?;
        check()?;
        if image.format != crate::image::ImageFormat::Png
            || (image.width, image.height) != (width, height)
        {
            return Err(malformed(
                "ORA image decoder disagrees with the planned layer",
            ));
        }
        if image
            .losses
            .iter()
            .any(|code| code == "animation-first-frame-only")
        {
            return Err(unsupported("Animated ORA layer sources are not supported"));
        }
        if encoded[index].1[25] == 3 {
            loss(&mut stack.losses, "palette-indices-expanded");
        }
        for code in image.losses {
            loss(&mut stack.losses, &code);
        }
        layers.push(DecodedLayer {
            name: layer.name,
            origin: layer.origin,
            visible: layer.visible,
            opacity: layer.opacity,
            width,
            height,
            rgba: image.rgba,
        });
    }
    // OpenRaster lists its top layer first; PaintCraft roots are bottom to top.
    layers.reverse();
    check()?;
    Ok(GroupCopy {
        decoded: Decoded {
            width: stack.width,
            height: stack.height,
            title: stack.title,
            resolution: stack.resolution,
            layers,
            losses: stack.losses,
        },
        merged: projection,
    })
}

#[cfg(test)]
#[path = "ora/group_tests.rs"]
mod group_tests;

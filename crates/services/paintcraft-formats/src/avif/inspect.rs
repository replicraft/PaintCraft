//! Physical AVIF still-container inspection only, never decoder admission.
//!
//! Item associations/extents, AV1 sequence headers and their agreement with
//! these declarations must be validated separately before any pixel allocation.
use super::*;
use paintcraft_paint::{BoxInfo, BoxTree, Layout};

pub const INPUT_BYTES_MAX: usize = 64 << 20;
pub const METADATA_BYTES_MAX: usize = 4 << 20;
pub const INSPECT_PIXELS_MAX: u64 = 4_194_304;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpatialExtent {
    /// One-based property index; this is not an item or primary-image identity.
    pub property_index: u16,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Av1Configuration<'a> {
    pub property_index: u16,
    pub profile: u8,
    pub level: u8,
    pub tier: bool,
    pub bit_depth: u8,
    pub monochrome: bool,
    pub subsampling_x: bool,
    pub subsampling_y: bool,
    pub chroma_position: u8,
    /// Borrowed config OBUs, not parsed or passed to a decoder by inspection.
    pub config_obus: &'a [u8],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container<'a> {
    pub spatial_extents: Vec<SpatialExtent>,
    pub av1_configurations: Vec<Av1Configuration<'a>>,
    pub property_count: usize,
    pub box_count: usize,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn layout(info: &BoxInfo<'_>, parent: Option<[u8; 4]>) -> Result<Layout> {
    let prefix = match (&info.kind, parent) {
        (b"meta", None) => {
            if info.payload.get(..4) != Some(&[0, 0, 0, 0]) {
                return Err(unsupported("AVIF meta version or flags"));
            }
            4
        }
        (b"iprp", Some(p)) if p == *b"meta" => 0,
        (b"ipco", Some(p)) if p == *b"iprp" => 0,
        _ => return Ok(Layout::Opaque),
    };
    Ok(Layout::Children {
        prefix_bytes: prefix,
    })
}
fn unique(tree: &BoxTree<'_>, parent: Option<usize>, kind: [u8; 4]) -> Result<usize> {
    let mut found = tree
        .boxes
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parent == parent && b.kind == kind);
    let index = found
        .next()
        .map(|(i, _)| i)
        .ok_or_else(|| invalid("Missing AVIF container box"))?;
    if found.next().is_some() {
        return Err(invalid("Duplicate AVIF container box"));
    }
    Ok(index)
}
pub(super) fn av1_configuration(index: u16, bytes: &[u8]) -> Result<Av1Configuration<'_>> {
    let header = bytes
        .get(..4)
        .ok_or_else(|| invalid("Truncated AVIF AV1 configuration"))?;
    // AV1-ISOBMFF v1.3 section2.3.3: marker1, version1; reserved fields zero.
    if header[0] != 0x81
        || header[3] & 0xe0 != 0
        || (header[3] & 0x10 == 0 && header[3] & 0x0f != 0)
    {
        return Err(unsupported(
            "AVIF AV1 configuration marker/version/reserved fields",
        ));
    }
    let profile = header[1] >> 5;
    let high = header[2] & 0x40 != 0;
    let twelve = header[2] & 0x20 != 0;
    if profile > 2 || (twelve && (!high || profile != 2)) {
        return Err(invalid("Inconsistent AVIF AV1 configuration profile/depth"));
    }
    Ok(Av1Configuration {
        property_index: index,
        profile,
        level: header[1] & 31,
        tier: header[2] & 0x80 != 0,
        bit_depth: if twelve {
            12
        } else if high {
            10
        } else {
            8
        },
        monochrome: header[2] & 0x10 != 0,
        subsampling_x: header[2] & 8 != 0,
        subsampling_y: header[2] & 4 != 0,
        chroma_position: header[2] & 3,
        config_obus: &bytes[4..],
    })
}
/// Inspect bounded physical metadata and property declarations in a still AVIF.
/// This does not identify the primary item's associated properties, validate
/// coded dimensions/color, parse OBUs or authorize any decoder invocation.
pub fn inspect_container<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Container<'a>> {
    let limits = paintcraft_paint::Limits {
        source_bytes: INPUT_BYTES_MAX,
        boxes: 4096,
        depth: 8,
        metadata_bytes: METADATA_BYTES_MAX,
        compatible_brands: 64,
        extents: 4096,
    };
    let tree = paintcraft_paint::inspect_with_layout(bytes, &limits, check, &layout)?;
    let brand = tree
        .file_type
        .as_ref()
        .ok_or_else(|| invalid("Missing AVIF file type"))?;
    if !brand.has_brand(*b"avif") || brand.has_brand(*b"avis") {
        return Err(unsupported(
            "Only AVIF still-container inspection is available",
        ));
    }
    if tree.boxes.first().is_none_or(|b| b.kind != *b"ftyp") {
        return Err(invalid("AVIF file type must precede metadata"));
    }
    if tree
        .boxes
        .iter()
        .any(|b| b.parent.is_none() && !matches!(&b.kind, b"ftyp" | b"meta" | b"mdat" | b"free"))
    {
        return Err(unsupported("Unqualified AVIF top-level box"));
    }
    if !tree
        .boxes
        .iter()
        .any(|b| matches!(&b.kind, b"mdat" | b"idat") && !b.payload.is_empty())
    {
        return Err(invalid("AVIF item storage is missing"));
    }
    let meta = unique(&tree, None, *b"meta")?;
    let iprp = unique(&tree, Some(meta), *b"iprp")?;
    let ipco = unique(&tree, Some(iprp), *b"ipco")?;
    let mut metadata = 0usize;
    for b in &tree.boxes {
        check()?;
        if !b.children_parsed && &b.kind != b"mdat" {
            metadata = metadata
                .checked_add(b.payload.len())
                .ok_or_else(|| resource("AVIF metadata extent overflow"))?;
            if metadata > METADATA_BYTES_MAX {
                return Err(resource("AVIF metadata exceeds allowance"));
            }
        }
    }
    let mut result = Container {
        spatial_extents: Vec::new(),
        av1_configurations: Vec::new(),
        property_count: 0,
        box_count: tree.boxes.len(),
    };
    for property in tree.boxes.iter().filter(|b| b.parent == Some(ipco)) {
        check()?;
        result.property_count += 1;
        if result.property_count > 1024 {
            return Err(resource("AVIF property count exceeds allowance"));
        }
        let index = result.property_count as u16;
        match &property.kind {
            b"ispe" => {
                let p = property.payload;
                if p.len() != 12 || p[..4] != [0; 4] {
                    return Err(invalid("Invalid AVIF spatial extent property"));
                }
                let width = u32::from_be_bytes(p[4..8].try_into().unwrap());
                let height = u32::from_be_bytes(p[8..12].try_into().unwrap());
                if width == 0 || height == 0 {
                    return Err(invalid("AVIF spatial extent is empty"));
                }
                if width > 65536
                    || height > 65536
                    || u64::from(width) * u64::from(height) > pixels.min(INSPECT_PIXELS_MAX)
                {
                    return Err(resource("AVIF declared dimensions exceed allowance"));
                }
                result.spatial_extents.push(SpatialExtent {
                    property_index: index,
                    width,
                    height,
                });
            }
            b"av1C" => result
                .av1_configurations
                .push(av1_configuration(index, property.payload)?),
            _ => {} // Not interpreted by this inspection; no semantic approval.
        }
    }
    if result.spatial_extents.is_empty() || result.av1_configurations.is_empty() {
        return Err(invalid("AVIF image properties are incomplete"));
    }
    check()?;
    Ok(result)
}
#[cfg(test)]
mod tests;

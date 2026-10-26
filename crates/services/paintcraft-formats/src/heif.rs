//! Bounded HEIF metadata inspection, not an enabled HEVC decoder.
//!
//! Container dimensions/depth are declarations; they do not establish agreement
//! with every coded SPS or cap decoder reference/scratch allocations. No pixel
//! decoding, color conversion, grid assembly or transform application occurs.
mod metadata;
mod properties;
#[cfg(test)]
mod tests;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_paint::{BoxInfo, BoxTree, Layout};
pub use properties::{Color, Property};

pub const INPUT_BYTES_MAX: usize = 64 << 20;
pub const METADATA_BYTES_MAX: usize = 4 << 20;
pub const ITEMS_MAX: usize = 256;
pub const PROPERTIES_MAX: usize = 1024;
pub const LINKS_MAX: usize = 4096;
pub const PIXELS_MAX: u64 = 4_194_304;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item<'a> {
    pub id: u32,
    pub kind: [u8; 4],
    pub name: &'a str,
    pub hidden: bool,
    /// Association order is retained; transformations are not applied.
    pub properties: Vec<Association<'a>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Association<'a> {
    pub index: u16,
    pub essential: bool,
    pub property: Property<'a>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub kind: [u8; 4],
    pub from: u32,
    /// Original order, including the raster order of grid tiles.
    pub to: Vec<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection<'a> {
    pub primary_id: u32,
    pub items: Vec<Item<'a>>,
    pub references: Vec<Reference>,
}
impl Inspection<'_> {
    pub fn primary(&self) -> Option<&Item<'_>> {
        self.item(self.primary_id)
    }
    pub fn item(&self, id: u32) -> Option<&Item<'_>> {
        self.items.iter().find(|item| item.id == id)
    }
    pub fn references(&self, kind: [u8; 4], from: u32) -> Option<&[u32]> {
        self.references
            .iter()
            .find(|r| r.kind == kind && r.from == from)
            .map(|r| r.to.as_slice())
    }
    fn primary_source(&self, id: u32) -> bool {
        let mut current = self.primary_id;
        for _ in 0..=16 {
            if current == id {
                return true;
            }
            if self.item(current).is_none_or(|item| item.kind != *b"iden") {
                return false;
            }
            let Some([source]) = self.references(*b"dimg", current) else {
                return false;
            };
            current = *source;
        }
        false
    }
    /// Explicitly typed alpha linked TO the primary or its identity source.
    /// Other auxiliary images are not silently treated as opacity.
    pub fn alpha(&self) -> Option<&Item<'_>> {
        self.items.iter().find(|item| {
            self.references(*b"auxl", item.id)
                .is_some_and(|ids| ids.iter().any(|id| self.primary_source(*id)))
                && item.properties.iter().any(|p| {
                    matches!(
                        p.property,
                        Property::Auxiliary {
                            kind: "urn:mpeg:hevc:2015:auxid:1",
                            ..
                        }
                    )
                })
        })
    }
    pub fn premultiplied_alpha(&self) -> bool {
        self.alpha().is_some_and(|alpha| {
            self.references.iter().any(|link| {
                link.kind == *b"prem"
                    && self.primary_source(link.from)
                    && link.to.contains(&alpha.id)
            })
        })
    }
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
fn push<T>(values: &mut Vec<T>, value: T, limit: usize) -> Result<()> {
    if values.len() >= limit {
        return Err(resource("HEIF collection exceeds allowance"));
    }
    values
        .try_reserve(1)
        .map_err(|_| resource("HEIF inspection allocation failed"))?;
    values.push(value);
    Ok(())
}
fn full(body: &[u8]) -> Result<(u8, u32, &[u8])> {
    let p = body
        .get(..4)
        .ok_or_else(|| invalid("truncated HEIF full box"))?;
    Ok((p[0], u32::from_be_bytes([0, p[1], p[2], p[3]]), &body[4..]))
}
fn layout(info: &BoxInfo<'_>, parent: Option<[u8; 4]>) -> Result<Layout> {
    let prefix = match (&info.kind, parent) {
        (b"meta", None) => {
            if full(info.payload)?.0 != 0 || full(info.payload)?.1 != 0 {
                return Err(unsupported("HEIF meta version/flags"));
            }
            4
        }
        (b"iprp", Some(p)) if p == *b"meta" => 0,
        (b"ipco", Some(p)) if p == *b"iprp" => 0,
        (b"iinf", Some(p)) if p == *b"meta" => {
            let (v, f, _) = full(info.payload)?;
            if f != 0 || v > 1 {
                return Err(unsupported("HEIF item inventory version/flags"));
            }
            if v == 0 { 6 } else { 8 }
        }
        (b"iref", Some(p)) if p == *b"meta" => {
            let (v, f, _) = full(info.payload)?;
            if f != 0 || v > 1 {
                return Err(unsupported("HEIF reference version/flags"));
            }
            4
        }
        _ => return Ok(Layout::Opaque),
    };
    Ok(Layout::Children {
        prefix_bytes: prefix,
    })
}
fn children<'t, 'a>(
    tree: &'t BoxTree<'a>,
    parent: usize,
) -> impl Iterator<Item = (usize, &'t BoxInfo<'a>)> {
    tree.boxes
        .iter()
        .enumerate()
        .filter(move |(_, b)| b.parent == Some(parent))
}
fn unique_child(tree: &BoxTree<'_>, parent: usize, kind: [u8; 4]) -> Result<Option<usize>> {
    let mut matches = tree
        .boxes
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parent == Some(parent) && b.kind == kind);
    let found = matches.next().map(|(i, _)| i);
    if matches.next().is_some() {
        return Err(invalid("duplicate HEIF metadata box"));
    }
    Ok(found)
}
/// Inspect the explicit primary item and item/property/reference graph. Values
/// are borrowed wherever possible. Unknown essential properties fail closed.
/// This is a prerequisite to SPS/decoder qualification, never decode admission.
pub fn inspect_checked<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection<'a>> {
    let limits = paintcraft_paint::Limits {
        source_bytes: INPUT_BYTES_MAX,
        boxes: 4096,
        depth: 8,
        metadata_bytes: METADATA_BYTES_MAX,
        compatible_brands: 64,
        extents: LINKS_MAX,
    };
    let tree = paintcraft_paint::inspect_with_layout(bytes, &limits, check, &layout)?;
    let brand = tree
        .file_type
        .as_ref()
        .ok_or_else(|| invalid("missing HEIF file type"))?;
    if ![*b"mif1", *b"heic", *b"heix", *b"hevc", *b"hevx"]
        .iter()
        .any(|b| brand.has_brand(*b))
    {
        return Err(unsupported("not a HEIF/HEVC still-image container"));
    }
    if tree.boxes.first().is_none_or(|b| b.kind != *b"ftyp") {
        return Err(invalid("HEIF file type must precede item metadata"));
    }
    let mut metas = tree
        .boxes
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parent.is_none() && b.kind == *b"meta");
    let meta = metas
        .next()
        .map(|(i, _)| i)
        .ok_or_else(|| invalid("missing HEIF metadata"))?;
    if metas.next().is_some() {
        return Err(invalid("duplicate HEIF metadata"));
    }
    // Count raw leaf metadata too; the generic walker separately bounds its
    // traversal objects. mdat/idat are encoded item storage, never followed here.
    let mut metadata_bytes = 0usize;
    for b in &tree.boxes {
        check()?;
        if !b.children_parsed && !matches!(&b.kind, b"mdat" | b"idat") {
            metadata_bytes = metadata_bytes
                .checked_add(b.payload.len())
                .ok_or_else(|| resource("HEIF metadata length overflow"))?;
            if metadata_bytes > METADATA_BYTES_MAX {
                return Err(resource("HEIF metadata exceeds allowance"));
            }
        }
    }
    let pitm = unique_child(&tree, meta, *b"pitm")?
        .ok_or_else(|| invalid("missing explicit HEIF primary item"))?;
    let primary_id = metadata::primary(tree.boxes[pitm].payload)?;
    let iinf = unique_child(&tree, meta, *b"iinf")?
        .ok_or_else(|| invalid("missing HEIF item inventory"))?;
    let mut items = metadata::items(&tree, iinf, check)?;
    let references = if let Some(iref) = unique_child(&tree, meta, *b"iref")? {
        metadata::references(&tree, iref, check)?
    } else {
        Vec::new()
    };
    let iprp = unique_child(&tree, meta, *b"iprp")?
        .ok_or_else(|| invalid("HEIF image properties are missing"))?;
    properties::associations(&tree, iprp, &mut items, pixels.min(PIXELS_MAX), check)?;
    let result = Inspection {
        primary_id,
        items,
        references,
    };
    let primary = result
        .item(primary_id)
        .ok_or_else(|| invalid("primary item absent from inventory"))?;
    if primary.hidden {
        return Err(invalid("HEIF primary image is marked hidden"));
    }
    if !matches!(&primary.kind, b"hvc1" | b"grid" | b"iden") {
        return Err(unsupported(
            "HEIF primary image kind is not admitted for inspection",
        ));
    }
    metadata::validate_links(&result, check)?;
    check()?;
    Ok(result)
}

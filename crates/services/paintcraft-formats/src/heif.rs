//! Bounded HEIF metadata inspection, not an enabled HEVC decoder.
//!
//! Container dimensions/depth are declarations; they do not establish agreement
//! with every coded SPS or cap decoder reference/scratch allocations. No pixel
//! decoding, color conversion, grid assembly or transform application occurs.
#[cfg(feature = "heif-codec")]
pub mod admission;
mod import;
pub use import::{COLOR_POLICY, ImportAssessment, KNOWN_IMPORT_LOSSES, assess_import};
mod metadata;
mod payloads;
mod properties;
#[cfg(test)]
mod qualification;
#[cfg(test)]
mod tests;
pub use crate::item_graph::{Grid, Payload};
#[cfg(test)]
use paintcraft_core::types::ErrorCode;
use paintcraft_core::types::{Error, Result};
use paintcraft_paint::{BoxInfo, BoxTree};
pub use properties::{Color, Property};

pub use crate::item_graph::{
    INPUT_BYTES_MAX, ITEMS_MAX, LINKS_MAX, METADATA_BYTES_MAX, PROPERTIES_MAX,
};
pub type Item<'a> = crate::item_graph::Item<'a, Property<'a>>;
pub type Association<'a> = crate::item_graph::Association<Property<'a>>;
pub use crate::item_graph::Reference;
pub type Inspection<'a> = crate::item_graph::Inspection<'a, Property<'a>>;
pub const PIXELS_MAX: u64 = 4_194_304;
impl Inspection<'_> {
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
use crate::item_graph::{full, invalid, layout, push, resource, unique_child, unsupported};
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
    let mut result = Inspection {
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
    if let Some(iloc) = unique_child(&tree, meta, *b"iloc")? {
        payloads::resolve(bytes, &tree, meta, iloc, &mut result, check)?;
    }
    check()?;
    Ok(result)
}

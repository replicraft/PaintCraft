//! Shared bounded ISO item inventory, property associations and finite storage.
//! Codec semantics and public admission remain with the owning format adapter.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_paint::{BoxInfo, BoxTree, Layout};
pub(crate) mod metadata;
pub(crate) mod payloads;
pub(crate) mod properties;
pub(crate) use metadata::Reader;
pub const INPUT_BYTES_MAX: usize = 64 << 20;
pub const METADATA_BYTES_MAX: usize = 4 << 20;
pub const ITEMS_MAX: usize = 256;
pub const PROPERTIES_MAX: usize = 1024;
pub const LINKS_MAX: usize = 4096;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    pub rows: u16,
    pub columns: u16,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload<'a> {
    pub parts: Vec<&'a [u8]>,
    pub byte_len: usize,
    pub grid: Option<Grid>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item<'a, P> {
    pub id: u32,
    pub kind: [u8; 4],
    pub name: &'a str,
    pub hidden: bool,
    pub payload: Option<Payload<'a>>,
    /// Association order is retained; transformations are not applied.
    pub properties: Vec<Association<P>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Association<P> {
    pub index: u16,
    pub essential: bool,
    pub property: P,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub kind: [u8; 4],
    pub from: u32,
    /// Original order, including the raster order of grid tiles.
    pub to: Vec<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection<'a, P> {
    pub primary_id: u32,
    pub items: Vec<Item<'a, P>>,
    pub references: Vec<Reference>,
}
impl<P> Inspection<'_, P> {
    pub fn primary(&self) -> Option<&Item<'_, P>> {
        self.item(self.primary_id)
    }
    pub fn item(&self, id: u32) -> Option<&Item<'_, P>> {
        self.items.iter().find(|item| item.id == id)
    }
    pub fn references(&self, kind: [u8; 4], from: u32) -> Option<&[u32]> {
        self.references
            .iter()
            .find(|r| r.kind == kind && r.from == from)
            .map(|r| r.to.as_slice())
    }
}
pub(crate) fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
pub(crate) fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
pub(crate) fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
pub(crate) fn push<T>(values: &mut Vec<T>, value: T, limit: usize) -> Result<()> {
    if values.len() >= limit {
        return Err(resource("HEIF collection exceeds allowance"));
    }
    values
        .try_reserve(1)
        .map_err(|_| resource("HEIF inspection allocation failed"))?;
    values.push(value);
    Ok(())
}
pub(crate) fn full(body: &[u8]) -> Result<(u8, u32, &[u8])> {
    let p = body
        .get(..4)
        .ok_or_else(|| invalid("truncated HEIF full box"))?;
    Ok((p[0], u32::from_be_bytes([0, p[1], p[2], p[3]]), &body[4..]))
}
pub(crate) fn layout(info: &BoxInfo<'_>, parent: Option<[u8; 4]>) -> Result<Layout> {
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
pub(crate) fn children<'t, 'a>(
    tree: &'t BoxTree<'a>,
    parent: usize,
) -> impl Iterator<Item = (usize, &'t BoxInfo<'a>)> {
    tree.boxes
        .iter()
        .enumerate()
        .filter(move |(_, b)| b.parent == Some(parent))
}
pub(crate) fn unique_child(
    tree: &BoxTree<'_>,
    parent: usize,
    kind: [u8; 4],
) -> Result<Option<usize>> {
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

#[cfg(test)]
mod tests;

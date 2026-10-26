//! Primary identity and finite item storage use the shared ISO graph owner.
use super::{properties::Property, *};
use crate::item_graph::{self as graph, invalid, resource, unique_child, unsupported};
pub(super) type Inspection<'a> = graph::Inspection<'a, Property<'a>>;
pub(super) fn inspect<'a>(
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
        extents: graph::LINKS_MAX,
    };
    let tree = paintcraft_paint::inspect_with_layout(bytes, &limits, check, &graph::layout)?;
    let brand = tree
        .file_type
        .as_ref()
        .ok_or_else(|| invalid("Missing AVIF file type"))?;
    if !brand.has_brand(*b"avif") || brand.has_brand(*b"avis") {
        return Err(unsupported(
            "Only AVIF still-image items are qualified for admission",
        ));
    }
    if tree.boxes.first().is_none_or(|b| b.kind != *b"ftyp") {
        return Err(invalid("AVIF file type must precede metadata"));
    }
    let mut meta = None;
    let mut metadata = 0usize;
    for (index, b) in tree.boxes.iter().enumerate() {
        check()?;
        if b.parent.is_none() {
            if !matches!(&b.kind, b"ftyp" | b"meta" | b"mdat" | b"free") {
                return Err(unsupported("AVIF top-level box is unqualified"));
            }
            if b.kind == *b"meta" && meta.replace(index).is_some() {
                return Err(invalid("Duplicate AVIF metadata"));
            }
        }
        if !b.children_parsed && !matches!(&b.kind, b"mdat" | b"idat") {
            metadata = metadata
                .checked_add(b.payload.len())
                .ok_or_else(|| resource("AVIF metadata overflow"))?;
            if metadata > METADATA_BYTES_MAX {
                return Err(resource("AVIF metadata exceeds allowance"));
            }
        }
    }
    let meta = meta.ok_or_else(|| invalid("Missing AVIF metadata"))?;
    for (_, b) in graph::children(&tree, meta) {
        if !matches!(
            &b.kind,
            b"hdlr" | b"pitm" | b"iloc" | b"iinf" | b"iprp" | b"iref" | b"idat"
        ) {
            return Err(unsupported("AVIF metadata child is unqualified"));
        }
    }
    let required = |kind| {
        unique_child(&tree, meta, kind)?.ok_or_else(|| invalid("Missing AVIF required item box"))
    };
    let handler = tree.boxes[required(*b"hdlr")?].payload;
    let (version, flags, body) = graph::full(handler)?;
    if version != 0
        || flags != 0
        || body.len() < 20
        || body[..4] != [0; 4]
        || body[4..8] != *b"pict"
        || body[8..20] != [0; 12]
    {
        return Err(unsupported("AVIF image handler is unqualified"));
    }
    if body.len() > 20 {
        let mut name = graph::Reader::new(&body[20..]);
        name.string()?;
        name.end()?;
    }
    let primary_id = graph::metadata::primary(tree.boxes[required(*b"pitm")?].payload)?;
    let mut items = graph::metadata::items(&tree, required(*b"iinf")?, check)?;
    let references = match unique_child(&tree, meta, *b"iref")? {
        Some(index) => graph::metadata::references(&tree, index, check)?,
        None => Vec::new(),
    };
    let iprp = required(*b"iprp")?;
    for (_, child) in graph::children(&tree, iprp) {
        check()?;
        if !matches!(&child.kind, b"ipco" | b"ipma") {
            return Err(unsupported("AVIF image property child is unqualified"));
        }
    }
    graph::properties::associations(
        &tree,
        iprp,
        &mut items,
        &|info| properties::parse(info, pixels),
        &|value| !matches!(value, Property::Unsupported(_)),
        check,
    )?;
    // Initial conversion has no policy for silently discarding unused metadata.
    // Every physical ipco property must participate in the inspected item graph.
    let ipco = unique_child(&tree, iprp, *b"ipco")?
        .ok_or_else(|| invalid("Missing AVIF property container"))?;
    let count = graph::children(&tree, ipco).count();
    let mut used = [false; graph::PROPERTIES_MAX];
    for item in &items {
        check()?;
        for association in &item.properties {
            used[usize::from(association.index) - 1] = true;
        }
    }
    if used[..count].iter().any(|used| !used) {
        return Err(unsupported(
            "AVIF unassociated properties require a separate metadata policy",
        ));
    }
    let mut value = Inspection {
        primary_id,
        items,
        references,
    };
    let primary = value
        .primary()
        .ok_or_else(|| invalid("AVIF primary is absent from inventory"))?;
    if primary.hidden {
        return Err(invalid("AVIF primary is hidden"));
    }
    if primary.kind != *b"av01" {
        return Err(unsupported("AVIF derived/grid primary is unqualified"));
    }
    for link in &value.references {
        check()?;
        if value.item(link.from).is_none() || link.to.iter().any(|id| value.item(*id).is_none()) {
            return Err(invalid("AVIF reference names an absent item"));
        }
    }
    graph::payloads::resolve(bytes, &tree, meta, required(*b"iloc")?, &mut value, check)?;
    check()?;
    Ok(value)
}

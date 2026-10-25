use super::*;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxInfo<'a> {
    pub kind: [u8; 4],
    pub range: Range<usize>,
    pub header_bytes: usize,
    pub user_type: Option<[u8; 16]>,
    pub payload: &'a [u8],
    pub parent: Option<usize>,
    pub depth: usize,
    pub children_parsed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileType<'a> {
    pub major_brand: [u8; 4],
    pub minor_version: u32,
    /// Borrowed four-byte compatible brands, not a list of decoded formats.
    pub compatible_brands: &'a [u8],
}
impl FileType<'_> {
    pub fn has_brand(&self, brand: [u8; 4]) -> bool {
        self.major_brand == brand || self.compatible_brands.as_chunks::<4>().0.contains(&brand)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxTree<'a> {
    /// Preorder, with stable parent indices. Unknown payloads remain opaque.
    pub boxes: Vec<BoxInfo<'a>>,
    pub file_type: Option<FileType<'a>>,
}
pub type LayoutResolver<'a> = dyn Fn(&BoxInfo<'_>, Option<[u8; 4]>) -> Result<Layout> + 'a;

struct Frame {
    offset: usize,
    end: usize,
    parent: Option<usize>,
    depth: usize,
}

fn read_box<'a>(bytes: &'a [u8], frame: &Frame) -> Result<BoxInfo<'a>> {
    let offset = frame.offset;
    let rest = bytes
        .get(offset..frame.end)
        .ok_or_else(|| malformed("ISO box scope exceeds input"))?;
    let header = rest
        .get(..8)
        .ok_or_else(|| malformed("Truncated ISO box header"))?;
    let declared = u32::from_be_bytes(header[..4].try_into().unwrap());
    let kind = header[4..8].try_into().unwrap();
    let mut header_bytes = 8usize;
    let size = match declared {
        0 => {
            if frame.parent.is_some() {
                return Err(unsupported(
                    "Zero-size ISO boxes are supported only at top level",
                ));
            }
            rest.len()
        }
        1 => {
            header_bytes = 16;
            let large = rest
                .get(8..16)
                .ok_or_else(|| malformed("Truncated ISO extended box size"))?;
            usize::try_from(u64::from_be_bytes(large.try_into().unwrap()))
                .map_err(|_| resource("ISO box size exceeds target address space"))?
        }
        value => value as usize,
    };
    let user_type = if &kind == b"uuid" {
        let end = header_bytes + 16;
        let uuid = rest
            .get(header_bytes..end)
            .ok_or_else(|| malformed("Truncated ISO UUID box header"))?;
        header_bytes = end;
        Some(uuid.try_into().unwrap())
    } else {
        None
    };
    if size < header_bytes {
        return Err(malformed("ISO box size is smaller than its header"));
    }
    let end = offset
        .checked_add(size)
        .filter(|end| *end <= frame.end)
        .ok_or_else(|| malformed("ISO box exceeds its parent scope"))?;
    Ok(BoxInfo {
        kind,
        range: offset..end,
        header_bytes,
        user_type,
        payload: &bytes[offset + header_bytes..end],
        parent: frame.parent,
        depth: frame.depth,
        children_parsed: false,
    })
}
fn file_type<'a>(info: &BoxInfo<'a>, limits: &Limits) -> Result<FileType<'a>> {
    let payload = info.payload;
    if payload.len() < 8 || !(payload.len() - 8).is_multiple_of(4) {
        return Err(malformed("Invalid ISO file type box length"));
    }
    if (payload.len() - 8) / 4 > limits.compatible_brands {
        return Err(resource("ISO compatible brand count exceeds limit"));
    }
    Ok(FileType {
        major_brand: payload[..4].try_into().unwrap(),
        minor_version: u32::from_be_bytes(payload[4..8].try_into().unwrap()),
        compatible_brands: &payload[8..],
    })
}
fn structural_layout(info: &BoxInfo<'_>, parent: Option<[u8; 4]>) -> Result<Layout> {
    match (&info.kind, parent) {
        (b"meta", None) => {
            let full = info
                .payload
                .get(..4)
                .ok_or_else(|| malformed("Truncated ISO meta full-box prefix"))?;
            if full != [0, 0, 0, 0] {
                return Err(unsupported("Unsupported ISO meta version or flags"));
            }
            Ok(Layout::Children { prefix_bytes: 4 })
        }
        (b"iprp", Some(parent)) if parent == *b"meta" => Ok(Layout::Children { prefix_bytes: 0 }),
        (b"ipco", Some(parent)) if parent == *b"iprp" => Ok(Layout::Children { prefix_bytes: 0 }),
        _ => Ok(Layout::Opaque),
    }
}
/// Parse physical structure, without treating a shared HEIF brand as .paint.
/// Traversal is iterative and every child is confined to its parent's range.
pub fn inspect<'a>(
    bytes: &'a [u8],
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<BoxTree<'a>> {
    inspect_with_layout(bytes, limits, check, &structural_layout)
}
/// Layout injection is useful for independently verified future container
/// profiles. Prefixes are checked; opaque data is never guessed to be a tree.
pub fn inspect_with_layout<'a>(
    bytes: &'a [u8],
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
    layout: &LayoutResolver<'_>,
) -> Result<BoxTree<'a>> {
    check()?;
    if bytes.len() > limits.source_bytes {
        return Err(resource("ISO input exceeds the byte limit"));
    }
    let mut tree = BoxTree {
        boxes: Vec::new(),
        file_type: None,
    };
    // Fixed maximum stack depth prevents caller settings from enabling stack
    // exhaustion. Box traversal itself uses no Rust recursion.
    let max_depth = limits.depth.min(64);
    let mut stack = Vec::new();
    stack
        .try_reserve_exact(max_depth + 1)
        .map_err(|_| resource("ISO traversal allocation failed"))?;
    stack.push(Frame {
        offset: 0,
        end: bytes.len(),
        parent: None,
        depth: 0,
    });
    while let Some(frame) = stack.last_mut() {
        check()?;
        if frame.offset == frame.end {
            stack.pop();
            continue;
        }
        if tree.boxes.len() >= limits.boxes {
            return Err(resource("ISO box count exceeds the limit"));
        }
        let mut info = read_box(bytes, frame)?;
        frame.offset = info.range.end;
        let parent_kind = info.parent.map(|index: usize| tree.boxes[index].kind);
        if info.kind == *b"ftyp" && info.parent.is_none() {
            if tree.file_type.is_some() {
                return Err(malformed("Duplicate ISO file type boxes"));
            }
            tree.file_type = Some(file_type(&info, limits)?);
        }
        let children = match layout(&info, parent_kind)? {
            Layout::Opaque => None,
            Layout::Children { prefix_bytes } => {
                if info.depth >= max_depth {
                    return Err(resource("ISO nesting depth exceeds the limit"));
                }
                if prefix_bytes > info.payload.len() {
                    return Err(malformed("ISO container prefix exceeds its payload"));
                }
                info.children_parsed = true;
                Some(Frame {
                    offset: info.range.start + info.header_bytes + prefix_bytes,
                    end: info.range.end,
                    parent: Some(tree.boxes.len()),
                    depth: info.depth + 1,
                })
            }
        };
        push(&mut tree.boxes, info, limits.metadata_bytes)?;
        if let Some(children) = children {
            stack.push(children);
        }
    }
    check()?;
    Ok(tree)
}
fn push<'a>(boxes: &mut Vec<BoxInfo<'a>>, info: BoxInfo<'a>, budget: usize) -> Result<()> {
    let max = budget / std::mem::size_of::<BoxInfo<'a>>();
    if boxes.len() >= max {
        return Err(resource("ISO box metadata exceeds the byte limit"));
    }
    if boxes.len() == boxes.capacity() {
        let capacity = boxes.capacity().saturating_mul(2).max(16).min(max);
        boxes
            .try_reserve_exact(capacity - boxes.len())
            .map_err(|_| resource("ISO box metadata allocation failed"))?;
    }
    boxes.push(info);
    Ok(())
}

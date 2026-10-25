use super::*;
use std::cell::Cell;

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    pub fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(len)
            .ok_or_else(|| malformed("PAINT field overflow"))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| malformed("Truncated PAINT field"))?;
        self.position = end;
        Ok(value)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn full(&mut self, version: u8) -> Result<()> {
        if self.take(4)? != [version, 0, 0, 0] {
            return Err(unsupported("Unsupported PAINT full-box version or flags"));
        }
        Ok(())
    }
    pub fn end(&self) -> Result<()> {
        if self.position != self.bytes.len() {
            return Err(unsupported("Unrecognized PAINT trailing fields"));
        }
        Ok(())
    }
    fn name(&mut self) -> Result<&'a str> {
        let length = self.bytes[self.position..]
            .iter()
            .take(1025)
            .position(|b| *b == 0)
            .ok_or_else(|| resource("PAINT item name is unterminated or exceeds limits"))?;
        let text = std::str::from_utf8(self.take(length)?)
            .map_err(|_| malformed("PAINT item name must be UTF-8"))?;
        self.take(1)?;
        Ok(text)
    }
}
#[derive(Debug)]
pub(super) struct Item<'a> {
    pub id: u32,
    pub kind: [u8; 4],
    pub name: &'a str,
    location: Option<(u16, Extent)>,
    pub associations: Vec<usize>,
}
pub(super) struct Graph<'a> {
    bytes: &'a [u8],
    pub tree: BoxTree<'a>,
    pub items: Vec<Item<'a>>,
    pub primary: u32,
    pub order: Vec<u32>,
    properties: Vec<usize>,
    idat: usize,
    mdat: usize,
    metadata: Cell<usize>,
    metadata_limit: usize,
}
fn unique(tree: &BoxTree<'_>, parent: Option<usize>, kind: [u8; 4]) -> Result<usize> {
    let mut found = tree
        .boxes
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parent == parent && b.kind == kind)
        .map(|(i, _)| i);
    let index = found
        .next()
        .ok_or_else(|| malformed("Missing required PAINT box"))?;
    if found.next().is_some() {
        return Err(malformed("Duplicate PAINT box"));
    }
    Ok(index)
}
fn layout(info: &BoxInfo<'_>, parent: Option<[u8; 4]>) -> Result<Layout> {
    let prefix = match (parent, info.kind) {
        (None, kind) if kind == *b"meta" => {
            Reader::new(info.payload).full(0)?;
            Some(4)
        }
        (Some(p), kind) if p == *b"meta" && kind == *b"iinf" => {
            Reader::new(info.payload).full(0)?;
            Some(6)
        }
        (Some(p), kind) if p == *b"meta" && kind == *b"iref" => {
            Reader::new(info.payload).full(0)?;
            Some(4)
        }
        (Some(p), kind)
            if (p == *b"meta" && kind == *b"iprp") || (p == *b"iprp" && kind == *b"ipco") =>
        {
            Some(0)
        }
        _ => None,
    };
    Ok(
        prefix.map_or(Layout::Opaque, |prefix_bytes| Layout::Children {
            prefix_bytes,
        }),
    )
}
impl<'a> Graph<'a> {
    fn charge<T>(&self, n: usize) -> Result<()> {
        let bytes = n
            .checked_mul(std::mem::size_of::<T>())
            .and_then(|n| self.metadata.get().checked_add(n))
            .ok_or_else(|| resource("PAINT metadata size overflow"))?;
        if bytes > self.metadata_limit {
            return Err(resource("PAINT graph metadata exceeds limits"));
        }
        self.metadata.set(bytes);
        Ok(())
    }
    pub fn vector<T>(&self, n: usize) -> Result<Vec<T>> {
        self.charge::<T>(n)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(n)
            .map_err(|_| resource("Cannot allocate PAINT graph metadata"))?;
        Ok(values)
    }
    pub fn item(&self, id: u32) -> Result<&Item<'a>> {
        self.items
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| malformed("PAINT reference has no item"))
    }
    fn item_mut(&mut self, id: u32) -> Result<&mut Item<'a>> {
        self.items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| malformed("PAINT reference has no item"))
    }
    pub fn property(&self, item: &Item<'a>, kind: [u8; 4]) -> Result<&BoxInfo<'a>> {
        let mut found = item
            .associations
            .iter()
            .map(|index| &self.tree.boxes[self.properties[*index - 1]])
            .filter(|p| p.kind == kind);
        let result = found
            .next()
            .ok_or_else(|| malformed("Missing required PAINT item property"))?;
        if found.next().is_some() {
            return Err(malformed("Duplicate PAINT item property"));
        }
        Ok(result)
    }
    pub fn data(&self, item: &Item<'a>) -> Result<&'a [u8]> {
        let (method, extent) = item
            .location
            .ok_or_else(|| malformed("PAINT item lacks a location"))?;
        let scope = &self.tree.boxes[if method == 0 { self.mdat } else { self.idat }];
        let payload = (scope.range.start + scope.header_bytes) as u64;
        let extent = Extent {
            offset: if method == 0 {
                extent.offset
            } else {
                payload
                    .checked_add(extent.offset)
                    .ok_or_else(|| malformed("PAINT location overflow"))?
            },
            length: extent.length,
        };
        resolve_extent(self.bytes, payload..scope.range.end as u64, extent)
    }
    pub fn parse(
        bytes: &'a [u8],
        limits: &DecodeLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let tree = inspect_with_layout(bytes, &limits.container, check, &layout)?;
        if !tree
            .file_type
            .as_ref()
            .is_some_and(|file| file.has_brand(*b"mif1"))
        {
            return Err(unsupported(
                "PAINT decode requires the observed mif1 item profile",
            ));
        }
        for info in &tree.boxes {
            if info.parent.is_none()
                && ![*b"ftyp", *b"meta", *b"mdat", *b"free"].contains(&info.kind)
            {
                return Err(unsupported("Unrecognized PAINT top-level semantic box"));
            }
        }
        let meta = unique(&tree, None, *b"meta")?;
        if tree.boxes[meta].payload.len() > limits.container.metadata_bytes {
            return Err(resource("PAINT meta payload exceeds limits"));
        }
        let mdat = unique(&tree, None, *b"mdat")?;
        let idat = unique(&tree, Some(meta), *b"idat")?;
        for b in tree.boxes.iter().filter(|b| b.parent == Some(meta)) {
            if ![
                *b"hdlr", *b"pitm", *b"iinf", *b"iloc", *b"iprp", *b"iref", *b"idat",
            ]
            .contains(&b.kind)
            {
                return Err(unsupported("Unrecognized PAINT metadata semantics"));
            }
        }
        let mut hdlr = Reader::new(tree.boxes[unique(&tree, Some(meta), *b"hdlr")?].payload);
        hdlr.full(0)?;
        if hdlr.u32()? != 0 || hdlr.take(4)? != b"pict" || hdlr.take(12)? != [0; 12] {
            return Err(unsupported("Unsupported PAINT item handler"));
        }
        hdlr.name()?;
        hdlr.end()?;
        let mut pitm = Reader::new(tree.boxes[unique(&tree, Some(meta), *b"pitm")?].payload);
        pitm.full(0)?;
        let primary = u32::from(pitm.u16()?);
        pitm.end()?;
        let metadata = Cell::new(tree.boxes.capacity() * std::mem::size_of::<BoxInfo<'_>>());
        let mut graph = Self {
            bytes,
            tree,
            items: Vec::new(),
            primary,
            order: Vec::new(),
            properties: Vec::new(),
            idat,
            mdat,
            metadata,
            metadata_limit: limits.container.metadata_bytes,
        };
        graph.parse_items(meta, limits, check)?;
        graph.parse_locations(meta, limits, check)?;
        graph.parse_properties(meta, check)?;
        graph.parse_order(meta, limits, check)?;
        // Explicitly reject aliasing between any declared item payloads.
        let mut ranges = graph.vector(graph.items.len())?;
        for item in &graph.items {
            let data = graph.data(item)?;
            ranges.push(Extent {
                offset: (data.as_ptr() as usize - bytes.as_ptr() as usize) as u64,
                length: data.len() as u64,
            });
        }
        graph.charge::<std::ops::Range<u64>>(ranges.len())?;
        validate_extents(
            &ranges,
            0..bytes.len() as u64,
            AliasPolicy::Reject,
            &limits.container,
            check,
        )?;
        Ok(graph)
    }
    fn parse_items(
        &mut self,
        meta: usize,
        limits: &DecodeLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let parent = unique(&self.tree, Some(meta), *b"iinf")?;
        let mut header = Reader::new(self.tree.boxes[parent].payload);
        header.full(0)?;
        let count = usize::from(header.u16()?);
        if count < 2 || count > limits.items || count > limits.layers.saturating_add(1) {
            return Err(resource("PAINT item count exceeds limits"));
        }
        let mut items = self.vector(count)?;
        for info in self.tree.boxes.iter().filter(|b| b.parent == Some(parent)) {
            check()?;
            if info.kind != *b"infe" {
                return Err(unsupported("Unrecognized PAINT item info"));
            }
            let mut r = Reader::new(info.payload);
            r.full(2)?;
            let id = u32::from(r.u16()?);
            if id == 0 || items.iter().any(|item: &Item<'_>| item.id == id) {
                return Err(malformed("Duplicate or zero PAINT item identifier"));
            }
            if r.u16()? != 0 {
                return Err(unsupported("Protected PAINT item"));
            }
            let kind = r.take(4)?.try_into().unwrap();
            if ![*b"unci", *b"iovl"].contains(&kind) {
                return Err(unsupported("Unsupported PAINT image item type"));
            }
            let name = r.name()?;
            r.end()?;
            if items.len() == count {
                return Err(malformed("PAINT item count disagrees with entries"));
            }
            items.push(Item {
                id,
                kind,
                name,
                location: None,
                associations: Vec::new(),
            });
        }
        if items.len() != count {
            return Err(malformed("PAINT item count disagrees with entries"));
        }
        self.items = items;
        Ok(())
    }
    fn parse_locations(
        &mut self,
        meta: usize,
        limits: &DecodeLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let mut r = Reader::new(self.tree.boxes[unique(&self.tree, Some(meta), *b"iloc")?].payload);
        r.full(1)?;
        if r.take(2)? != [0x44, 0] {
            return Err(unsupported("Unsupported PAINT location field widths"));
        }
        if usize::from(r.u16()?) != self.items.len() {
            return Err(malformed("PAINT location count differs from item count"));
        }
        if self.items.len() > limits.container.extents {
            return Err(resource("PAINT extent count exceeds limits"));
        }
        for _ in 0..self.items.len() {
            check()?;
            let id = u32::from(r.u16()?);
            let method = r.u16()?;
            if method > 1 || r.u16()? != 0 {
                return Err(unsupported(
                    "PAINT external, reserved or item-relative location",
                ));
            }
            if r.u16()? != 1 {
                return Err(unsupported(
                    "PAINT requires one extent per item in this profile",
                ));
            }
            let extent = Extent {
                offset: u64::from(r.u32()?),
                length: u64::from(r.u32()?),
            };
            if extent.length == 0 {
                return Err(malformed("Empty PAINT item extent"));
            }
            let item = self.item_mut(id)?;
            if item.location.is_some() {
                return Err(malformed("Duplicate PAINT location"));
            }
            if (item.kind == *b"iovl") != (method == 1) {
                return Err(unsupported(
                    "PAINT overlay must use idat and layer pixels must use mdat",
                ));
            }
            item.location = Some((method, extent));
        }
        r.end()
    }
    fn parse_properties(&mut self, meta: usize, check: &dyn Fn() -> Result<()>) -> Result<()> {
        let iprp = unique(&self.tree, Some(meta), *b"iprp")?;
        let ipco = unique(&self.tree, Some(iprp), *b"ipco")?;
        for child in self.tree.boxes.iter().filter(|b| b.parent == Some(iprp)) {
            if ![*b"ipco", *b"ipma"].contains(&child.kind) {
                return Err(unsupported("Unrecognized PAINT property container"));
            }
        }
        let count = self
            .tree
            .boxes
            .iter()
            .filter(|b| b.parent == Some(ipco))
            .count();
        self.properties = self.vector(count)?;
        for (index, info) in self
            .tree
            .boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.parent == Some(ipco))
        {
            check()?;
            if ![*b"cmpC", *b"ispe", *b"colr", *b"cmpd", *b"uncC", *b"pixi"].contains(&info.kind) {
                return Err(unsupported(
                    "Unqualified PAINT property; semantics must not be dropped",
                ));
            }
            self.properties.push(index);
        }
        let mut r = Reader::new(self.tree.boxes[unique(&self.tree, Some(iprp), *b"ipma")?].payload);
        r.full(0)?;
        if r.u32()? as usize != self.items.len() {
            return Err(malformed(
                "PAINT property association count differs from items",
            ));
        }
        for _ in 0..self.items.len() {
            check()?;
            let id = u32::from(r.u16()?);
            let n = usize::from(r.u8()?);
            if n == 0 {
                return Err(malformed("PAINT item requires property associations"));
            }
            let mut associations = self.vector(n)?;
            for _ in 0..n {
                let raw = r.u8()?;
                let index = usize::from(raw & 0x7f);
                if index == 0 || index > self.properties.len() || associations.contains(&index) {
                    return Err(malformed("Invalid or duplicate PAINT property association"));
                }
                associations.push(index);
            }
            let item = self.item_mut(id)?;
            if !item.associations.is_empty() {
                return Err(malformed("Duplicate PAINT item associations"));
            }
            item.associations = associations;
        }
        r.end()
    }
    fn parse_order(
        &mut self,
        meta: usize,
        limits: &DecodeLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let parent = unique(&self.tree, Some(meta), *b"iref")?;
        let reference = unique(&self.tree, Some(parent), *b"dimg")?;
        if self
            .tree
            .boxes
            .iter()
            .filter(|b| b.parent == Some(parent))
            .count()
            != 1
        {
            return Err(unsupported("Unsupported PAINT reference relationships"));
        }
        let mut r = Reader::new(self.tree.boxes[reference].payload);
        if u32::from(r.u16()?) != self.primary {
            return Err(malformed("PAINT overlay reference source is not primary"));
        }
        let n = usize::from(r.u16()?);
        if n > limits.layers || n.checked_add(1) != Some(self.items.len()) {
            return Err(unsupported(
                "PAINT profile requires one overlay referencing every layer",
            ));
        }
        self.order = self.vector(n)?;
        for _ in 0..n {
            check()?;
            let id = u32::from(r.u16()?);
            if id == self.primary || self.order.contains(&id) || self.item(id)?.kind != *b"unci" {
                return Err(malformed(
                    "Cyclic, duplicate or invalid PAINT layer reference",
                ));
            }
            self.order.push(id);
        }
        r.end()
    }
}

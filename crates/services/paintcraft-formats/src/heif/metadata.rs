use super::*;
pub(super) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| invalid("HEIF field overflow"))?;
        let value = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("truncated HEIF metadata"))?;
        self.pos = end;
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
    pub fn id(&mut self, wide: bool) -> Result<u32> {
        if wide {
            self.u32()
        } else {
            Ok(u32::from(self.u16()?))
        }
    }
    pub fn end(&self) -> Result<()> {
        if self.pos == self.bytes.len() {
            Ok(())
        } else {
            Err(invalid("unexpected HEIF metadata suffix"))
        }
    }
    pub fn string(&mut self) -> Result<&'a str> {
        let n = self.bytes[self.pos..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| invalid("unterminated HEIF name/type"))?;
        if n > 4096 {
            return Err(resource("HEIF text exceeds allowance"));
        }
        let value =
            std::str::from_utf8(self.take(n)?).map_err(|_| invalid("invalid HEIF UTF-8 text"))?;
        self.take(1)?;
        Ok(value)
    }
}
pub(super) fn primary(body: &[u8]) -> Result<u32> {
    let (v, f, body) = full(body)?;
    if v > 1 || f != 0 {
        return Err(unsupported("HEIF primary version/flags"));
    }
    let mut r = Reader::new(body);
    let id = r.id(v == 1)?;
    r.end()?;
    if id == 0 {
        return Err(invalid("zero HEIF primary ID"));
    }
    Ok(id)
}
pub(super) fn items<'a>(
    tree: &BoxTree<'a>,
    iinf: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<Item<'a>>> {
    let (v, _, body) = full(tree.boxes[iinf].payload)?;
    let mut r = Reader::new(body);
    let expected = r.id(v == 1)? as usize;
    if expected > ITEMS_MAX {
        return Err(resource("HEIF item count exceeds allowance"));
    }
    let mut result = Vec::new();
    for (_, info) in children(tree, iinf) {
        check()?;
        if info.kind != *b"infe" {
            return Err(unsupported("unknown HEIF item inventory child"));
        }
        let (v, flags, body) = full(info.payload)?;
        if !matches!(v, 2 | 3) || flags > 1 {
            return Err(unsupported("HEIF item info version/flags"));
        }
        let mut r = Reader::new(body);
        let id = r.id(v == 3)?;
        if id == 0 || result.iter().any(|item: &Item<'_>| item.id == id) {
            return Err(invalid("duplicate or zero HEIF item ID"));
        }
        if r.u16()? != 0 {
            return Err(unsupported("protected HEIF items are not admitted"));
        }
        let kind = r.take(4)?.try_into().unwrap();
        let name = r.string()?;
        // MIME/URI descriptors remain inert; no URI or external path is read.
        if kind == *b"mime" {
            r.string()?;
            if r.pos < r.bytes.len() {
                r.string()?;
            }
        } else if kind == *b"uri " {
            r.string()?;
        }
        r.end()?;
        push(
            &mut result,
            Item {
                id,
                kind,
                name,
                hidden: flags == 1,
                properties: Vec::new(),
            },
            ITEMS_MAX,
        )?;
    }
    if result.len() != expected {
        return Err(invalid("HEIF item count disagrees with inventory"));
    }
    Ok(result)
}
pub(super) fn references(
    tree: &BoxTree<'_>,
    iref: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<Reference>> {
    let (v, _, _) = full(tree.boxes[iref].payload)?;
    let mut result = Vec::new();
    let mut total = 0usize;
    for (_, info) in children(tree, iref) {
        check()?;
        let mut r = Reader::new(info.payload);
        let from = r.id(v == 1)?;
        let n = usize::from(r.u16()?);
        total = total
            .checked_add(n)
            .ok_or_else(|| resource("HEIF reference count overflow"))?;
        if total > LINKS_MAX {
            return Err(resource("HEIF references exceed allowance"));
        }
        if result
            .iter()
            .any(|link: &Reference| link.kind == info.kind && link.from == from)
        {
            return Err(invalid("duplicate HEIF reference source/type"));
        }
        let mut to = Vec::new();
        for _ in 0..n {
            check()?;
            let id = r.id(v == 1)?;
            if to.contains(&id) {
                return Err(invalid("duplicate HEIF reference target"));
            }
            push(&mut to, id, LINKS_MAX)?;
        }
        r.end()?;
        push(
            &mut result,
            Reference {
                kind: info.kind,
                from,
                to,
            },
            LINKS_MAX,
        )?;
    }
    Ok(result)
}
pub(super) fn validate_links(value: &Inspection<'_>, check: &dyn Fn() -> Result<()>) -> Result<()> {
    for link in &value.references {
        check()?;
        if value.item(link.from).is_none() || link.to.iter().any(|id| value.item(*id).is_none()) {
            return Err(invalid("HEIF reference names an absent item"));
        }
    }
    for item in &value.items {
        check()?;
        if item.kind == *b"grid" {
            let tiles = value
                .references(*b"dimg", item.id)
                .ok_or_else(|| invalid("HEIF grid lacks ordered tile references"))?;
            if tiles.is_empty()
                || tiles
                    .iter()
                    .any(|id| value.item(*id).is_none_or(|tile| tile.kind != *b"hvc1"))
            {
                return Err(unsupported("HEIF grid tiles must be direct HEVC images"));
            }
        }
        if item.kind == *b"iden" {
            let sources = value
                .references(*b"dimg", item.id)
                .ok_or_else(|| invalid("HEIF identity lacks source"))?;
            if sources.len() != 1 {
                return Err(invalid("HEIF identity requires exactly one source"));
            }
        }
        // Follow only image derivation references, and bound every path before
        // a candidate decoder could recursively visit an item graph.
        let mut stack = Vec::new();
        push(&mut stack, (item.id, 0usize), LINKS_MAX)?;
        let mut visited = 0usize;
        while let Some((id, depth)) = stack.pop() {
            check()?;
            visited += 1;
            if depth > 16 || visited > LINKS_MAX {
                return Err(resource("HEIF derivation exceeds traversal allowance"));
            }
            if let Some(targets) = value.references(*b"dimg", id) {
                for target in targets {
                    if *target == item.id {
                        return Err(invalid("cyclic HEIF derivation"));
                    }
                    push(&mut stack, (*target, depth + 1), LINKS_MAX)?;
                }
            }
        }
    }
    let alpha_count = value
        .items
        .iter()
        .filter(|item| {
            value
                .references(*b"auxl", item.id)
                .is_some_and(|ids| ids.iter().any(|id| value.primary_source(*id)))
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
        .count();
    if alpha_count > 1 {
        return Err(unsupported("multiple HEIF alpha auxiliaries are ambiguous"));
    }
    for link in value.references.iter().filter(|r| r.kind == *b"prem") {
        if !value.primary_source(link.from)
            || link.to.len() != 1
            || value.alpha().is_none_or(|alpha| link.to[0] != alpha.id)
        {
            return Err(invalid(
                "HEIF premultiplication must name the primary alpha item",
            ));
        }
    }
    Ok(())
}

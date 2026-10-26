use super::*;
pub(crate) struct Reader<'a> {
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
pub(crate) fn primary(body: &[u8]) -> Result<u32> {
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
pub(crate) fn items<'a, P>(
    tree: &BoxTree<'a>,
    iinf: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<Item<'a, P>>> {
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
        if id == 0 || result.iter().any(|item: &Item<'_, P>| item.id == id) {
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
                payload: None,
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
pub(crate) fn references(
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

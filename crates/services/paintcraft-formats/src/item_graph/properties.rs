use super::*;
pub(crate) fn associations<'a, P: Clone>(
    tree: &BoxTree<'a>,
    iprp: usize,
    items: &mut [Item<'a, P>],
    parse: &dyn Fn(&BoxInfo<'a>) -> Result<P>,
    known: &dyn Fn(&P) -> bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let ipco = unique_child(tree, iprp, *b"ipco")?
        .ok_or_else(|| invalid("HEIF properties lack container"))?;
    let mut properties = Vec::new();
    for (_, info) in children(tree, ipco) {
        check()?;
        push(&mut properties, parse(info)?, PROPERTIES_MAX)?;
    }
    let ipma = unique_child(tree, iprp, *b"ipma")?
        .ok_or_else(|| invalid("HEIF properties lack associations"))?;
    let (v, flags, body) = full(tree.boxes[ipma].payload)?;
    if v > 1 || flags > 1 {
        return Err(unsupported("HEIF association version/flags"));
    }
    let mut r = Reader::new(body);
    let count = r.u32()? as usize;
    if count > ITEMS_MAX {
        return Err(resource("HEIF association item count exceeds allowance"));
    }
    let mut seen = Vec::new();
    let mut total = 0usize;
    for _ in 0..count {
        check()?;
        let id = r.id(v == 1)?;
        if seen.contains(&id) {
            return Err(invalid("duplicate HEIF association item"));
        }
        push(&mut seen, id, ITEMS_MAX)?;
        let item = items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| invalid("HEIF association names an absent item"))?;
        let n = usize::from(r.u8()?);
        total += n;
        if total > LINKS_MAX {
            return Err(resource("HEIF property associations exceed allowance"));
        }
        for _ in 0..n {
            check()?;
            let raw = if flags == 1 {
                r.u16()?
            } else {
                u16::from(r.u8()?)
            };
            let mask = if flags == 1 { 0x8000 } else { 0x80 };
            let index = raw & (mask - 1);
            let essential = raw & mask != 0;
            if index == 0 {
                if essential {
                    return Err(invalid("essential HEIF association has no property"));
                }
                continue;
            }
            if item.properties.iter().any(|p| p.index == index) {
                return Err(invalid("duplicate HEIF property association"));
            }
            let property = properties
                .get(usize::from(index) - 1)
                .ok_or_else(|| invalid("HEIF association index exceeds property inventory"))?
                .clone();
            if essential && !known(&property) {
                return Err(unsupported("unknown essential HEIF property"));
            }
            push(
                &mut item.properties,
                Association {
                    index,
                    essential,
                    property,
                },
                LINKS_MAX,
            )?;
        }
    }
    r.end()?;
    Ok(())
}

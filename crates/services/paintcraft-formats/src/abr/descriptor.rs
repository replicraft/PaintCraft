//! Bounded, allocation-free descriptor walk before upstream tree construction.
use super::{preflight::Cursor, *};

const NODES_MAX: usize = 32768;
const NAME_UNITS_MAX: usize = 4096;
const TEXT_BYTES_MAX: usize = 1 << 20;
const BLOB_BYTES_MAX: usize = 4 << 20;
const DEPTH_MAX: usize = 16;

#[derive(Default)]
pub(super) struct Budget {
    nodes: usize,
    text: usize,
    blobs: usize,
}
impl Budget {
    fn nodes(&mut self, n: usize) -> Result<()> {
        self.nodes = self
            .nodes
            .checked_add(n)
            .ok_or_else(|| resource("ABR descriptor node overflow"))?;
        if self.nodes > NODES_MAX {
            return Err(resource("ABR descriptor node allowance exceeded"));
        }
        Ok(())
    }
    fn text(&mut self, n: usize) -> Result<()> {
        self.text += n;
        if self.text > TEXT_BYTES_MAX {
            return Err(resource("ABR descriptor text allowance exceeded"));
        }
        Ok(())
    }
}

pub(super) fn inspect(
    source: &mut Cursor<'_>,
    budget: &mut Budget,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if source.u32()? != 16 {
        return Err(unsupported("unsupported ABR descriptor version"));
    }
    descriptor(source, budget, 0, check)
}
fn count(source: &mut Cursor<'_>, budget: &mut Budget) -> Result<usize> {
    let n = source.u32()? as usize;
    budget.nodes(n)?;
    Ok(n)
}
fn text(
    source: &mut Cursor<'_>,
    budget: &mut Budget,
    little: bool,
    units: Option<usize>,
) -> Result<()> {
    let n = match units {
        Some(n) => n,
        None => source.u32()? as usize,
    };
    if n > NAME_UNITS_MAX {
        return Err(resource("ABR descriptor string allowance exceeded"));
    }
    budget.text(n * 2)?;
    let data = source.take(n * 2)?;
    let data = data.strip_suffix(&[0, 0]).unwrap_or(data);
    let units = data.as_chunks::<2>().0.iter().map(|v| {
        if little {
            u16::from_le_bytes(*v)
        } else {
            u16::from_be_bytes(*v)
        }
    });
    for character in char::decode_utf16(units) {
        if character.map_or(true, |v| v == '\0') {
            return Err(invalid("ABR descriptor text is not strict UTF-16"));
        }
    }
    Ok(())
}
fn ascii(source: &mut Cursor<'_>, budget: &mut Budget, n: usize) -> Result<()> {
    if n > NAME_UNITS_MAX {
        return Err(resource("ABR descriptor key allowance exceeded"));
    }
    budget.text(n)?;
    if !source.take(n)?.iter().all(|b| b.is_ascii()) {
        return Err(invalid("ABR descriptor key is not ASCII"));
    }
    Ok(())
}
fn id(source: &mut Cursor<'_>, budget: &mut Budget) -> Result<()> {
    let n = source.u32()? as usize;
    ascii(source, budget, if n == 0 { 4 } else { n })
}
fn class(source: &mut Cursor<'_>, budget: &mut Budget) -> Result<()> {
    text(source, budget, false, None)?;
    id(source, budget)
}
fn descriptor(
    source: &mut Cursor<'_>,
    budget: &mut Budget,
    depth: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if depth > DEPTH_MAX {
        return Err(resource("ABR descriptor nesting allowance exceeded"));
    }
    check()?;
    class(source, budget)?;
    for _ in 0..count(source, budget)? {
        check()?;
        id(source, budget)?;
        let kind = source.take(4)?;
        value(source, budget, depth + 1, kind, check)?;
    }
    Ok(())
}
fn float(source: &mut Cursor<'_>, wide: bool) -> Result<()> {
    let finite = if wide {
        f64::from_bits(u64::from_be_bytes(source.take(8)?.try_into().unwrap())).is_finite()
    } else {
        f32::from_bits(source.u32()?).is_finite()
    };
    if !finite {
        return Err(invalid("ABR descriptor number must be finite"));
    }
    Ok(())
}
fn units(source: &mut Cursor<'_>) -> Result<()> {
    if !matches!(
        source.take(4)?,
        b"#Ang"
            | b"#Rsl"
            | b"#Rlt"
            | b"#Nne"
            | b"#Prc"
            | b"#Pxl"
            | b"#Mlm"
            | b"#Pnt"
            | b"RrPi"
            | b"RrIn"
            | b"RrCm"
    ) {
        return Err(unsupported("unknown ABR descriptor units"));
    }
    Ok(())
}
fn value(
    source: &mut Cursor<'_>,
    budget: &mut Budget,
    depth: usize,
    kind: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if depth > DEPTH_MAX {
        return Err(resource("ABR descriptor nesting allowance exceeded"));
    }
    match kind {
        b"Objc" | b"GlbO" => descriptor(source, budget, depth, check)?,
        b"VlLs" => {
            for _ in 0..count(source, budget)? {
                let kind = source.take(4)?;
                value(source, budget, depth + 1, kind, check)?;
            }
        }
        b"doub" => float(source, true)?,
        b"UntF" | b"UnFl" => {
            units(source)?;
            float(source, kind == b"UntF")?;
        }
        b"TEXT" => text(source, budget, false, None)?,
        b"enum" => {
            id(source, budget)?;
            id(source, budget)?;
        }
        b"long" => {
            source.take(4)?;
        }
        b"comp" => {
            source.take(8)?;
        }
        b"bool" => {
            if source.take(1)?[0] > 1 {
                return Err(invalid("ABR descriptor Boolean must be zero or one"));
            }
        }
        b"type" | b"GlbC" => class(source, budget)?,
        b"alis" => {
            let n = source.u32()? as usize;
            ascii(source, budget, n)?;
        }
        b"tdta" => {
            let n = source.u32()? as usize;
            budget.blobs = budget
                .blobs
                .checked_add(n)
                .ok_or_else(|| resource("ABR blob allowance overflow"))?;
            if budget.blobs > BLOB_BYTES_MAX {
                return Err(resource("ABR descriptor blob allowance exceeded"));
            }
            source.take(n)?;
        }
        b"obj " => {
            for _ in 0..count(source, budget)? {
                check()?;
                match source.take(4)? {
                    b"prop" => {
                        class(source, budget)?;
                        id(source, budget)?;
                    }
                    b"Clss" => class(source, budget)?,
                    b"Enmr" => {
                        class(source, budget)?;
                        id(source, budget)?;
                        id(source, budget)?;
                    }
                    b"rele" => {
                        class(source, budget)?;
                        source.take(4)?;
                    }
                    b"Idnt" | b"indx" => {
                        source.take(4)?;
                    }
                    b"name" => {
                        class(source, budget)?;
                        text(source, budget, false, None)?;
                    }
                    _ => return Err(unsupported("unknown ABR descriptor reference kind")),
                }
            }
        }
        b"ObAr" => {
            if source.u32()? != 16 {
                return Err(unsupported("unsupported ABR object-array version"));
            }
            class(source, budget)?;
            for _ in 0..count(source, budget)? {
                check()?;
                id(source, budget)?;
                if source.take(4)? != b"UnFl" {
                    return Err(unsupported("unsupported ABR object-array type"));
                }
                units(source)?;
                for _ in 0..count(source, budget)? {
                    check()?;
                    float(source, true)?;
                }
            }
        }
        b"Pth " => {
            let n = source.u32()? as usize;
            if n > TEXT_BYTES_MAX {
                return Err(resource("ABR descriptor path allowance exceeded"));
            }
            let mut path = Cursor {
                bytes: source.take(n)?,
                pos: 0,
            };
            ascii(&mut path, budget, 4)?;
            let inner = u32::from_le_bytes(path.take(4)?.try_into().unwrap()) as usize;
            if inner != n {
                return Err(invalid("ABR descriptor path extent mismatch"));
            }
            let chars = u32::from_le_bytes(path.take(4)?.try_into().unwrap()) as usize;
            text(&mut path, budget, true, Some(chars))?;
            path.finished()?;
        }
        _ => return Err(unsupported("unknown ABR descriptor value kind")),
    }
    Ok(())
}

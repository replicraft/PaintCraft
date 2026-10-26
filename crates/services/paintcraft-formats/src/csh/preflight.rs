use super::*;

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| invalid("CSH extent overflow"))?;
        let value = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("truncated CSH resource"))?;
        self.pos = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn zeros(&mut self, n: usize) -> Result<()> {
        if self.take(n)?.iter().any(|b| *b != 0) {
            return Err(invalid("unsupported nonzero CSH reserved data"));
        }
        Ok(())
    }
    fn name(&mut self, total: &mut usize) -> Result<String> {
        let n = self.u32()? as usize;
        if n > NAME_UNITS_MAX {
            return Err(resource("CSH name unit allowance exceeded"));
        }
        *total += n * 2;
        if *total > NAME_BYTES_MAX {
            return Err(resource("CSH aggregate name allowance exceeded"));
        }
        let bytes = self.take(n * 2)?;
        let bytes = bytes.strip_suffix(&[0, 0]).unwrap_or(bytes);
        let units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| u16::from_be_bytes(*v));
        let mut text = String::new();
        for character in char::decode_utf16(units) {
            let character = character.map_err(|_| invalid("CSH name has invalid UTF-16"))?;
            if character == '\0' {
                return Err(invalid("CSH name contains an interior null"));
            }
            text.push(character);
        }
        Ok(text)
    }
}

pub(super) fn inspect(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Vec<Shape>> {
    check()?;
    if bytes.len() > BYTES_MAX {
        return Err(resource("CSH source byte allowance exceeded"));
    }
    let mut source = Cursor { bytes, pos: 0 };
    if source.take(8)? != b"cush\0\0\0\x02" {
        return Err(invalid("unsupported CSH signature/version"));
    }
    let count = source.u32()? as usize;
    if count > SHAPES_MAX {
        return Err(resource("CSH shape allowance exceeded"));
    }
    let (mut names, mut records) = (0, 0);
    let mut shapes = Vec::new();
    for _ in 0..count {
        check()?;
        let name = source.name(&mut names)?;
        source.zeros((4 - source.pos % 4) % 4)?;
        if source.u32()? != 1 {
            return Err(invalid("unsupported CSH shape version"));
        }
        let size = source.u32()? as usize;
        let mut body = Cursor {
            bytes: source.take(size)?,
            pos: 0,
        };
        let id_len = usize::from(body.take(1)?[0]);
        let id = body.take(id_len)?;
        if !id.iter().all(|b| b.is_ascii() && *b != 0) {
            return Err(invalid("CSH resource ID must be null-free ASCII"));
        }
        let id = String::from_utf8(id.to_vec()).unwrap();
        let bounds = [body.u32()?, body.u32()?, body.u32()?, body.u32()?];
        if bounds[2] < bounds[0] || bounds[3] < bounds[1] {
            return Err(invalid("CSH bounds are inverted"));
        }
        let remaining = body.bytes.len() - body.pos;
        if !remaining.is_multiple_of(26) {
            return Err(invalid("CSH path has a partial record"));
        }
        records += remaining / 26;
        if records > RECORDS_MAX {
            return Err(resource("CSH aggregate path record allowance exceeded"));
        }
        let mut path = Vec::new();
        let mut current = None;
        let (mut clipboard, mut fill, mut initial) = (false, false, false);
        while body.pos < body.bytes.len() {
            check()?;
            let selector = body.u16()?;
            let record = match selector {
                0 | 3 => {
                    finish_path(current)?;
                    let knots = body.u16()?;
                    let operation = match body.u16()? as i16 {
                        -1 => None,
                        0 => Some(Operation::Exclude),
                        1 => Some(Operation::Combine),
                        2 => Some(Operation::Subtract),
                        3 => Some(Operation::Intersect),
                        _ => return Err(invalid("unknown CSH Boolean operation")),
                    };
                    let fill_rule = match body.u16()? {
                        0 => PathFillRule::EvenOddLegacy,
                        1 => PathFillRule::EvenOdd,
                        2 => PathFillRule::NonZero,
                        _ => return Err(invalid("unsupported CSH path flags")),
                    };
                    body.zeros(18)?;
                    let open = selector == 3;
                    current = Some((open, usize::from(knots)));
                    Record::Subpath {
                        open,
                        knots,
                        operation,
                        fill_rule,
                    }
                }
                1 | 2 | 4 | 5 => {
                    let Some((open, left)) = &mut current else {
                        return Err(invalid("CSH knot has no subpath"));
                    };
                    if *open != (selector >= 4) || *left == 0 {
                        return Err(invalid("CSH knot topology/count mismatch"));
                    }
                    *left -= 1;
                    let mut wire = [0; 6];
                    for value in &mut wire {
                        *value = body.u32()? as i32;
                    }
                    Record::Knot {
                        open: *open,
                        linked: selector == 1 || selector == 4,
                        points: [wire[1], wire[0], wire[3], wire[2], wire[5], wire[4]],
                    }
                }
                6 if !fill => {
                    finish_path(current)?;
                    fill = true;
                    let rule = body.u16()?;
                    if rule != 0 {
                        return Err(invalid("unsupported CSH path fill rule"));
                    }
                    body.zeros(22)?;
                    Record::FillRule(rule)
                }
                7 if !clipboard => {
                    finish_path(current)?;
                    clipboard = true;
                    let mut values = [0; 5];
                    for value in &mut values {
                        *value = body.u32()? as i32;
                    }
                    body.zeros(4)?;
                    Record::Clipboard(values)
                }
                8 if !initial => {
                    finish_path(current)?;
                    initial = true;
                    let all = match body.u16()? {
                        0 => false,
                        1 => true,
                        _ => return Err(invalid("invalid CSH initial fill value")),
                    };
                    body.zeros(22)?;
                    Record::InitialFill(all)
                }
                _ => return Err(invalid("unknown or repeated CSH path control record")),
            };
            path.push(record);
        }
        finish_path(current)?;
        shapes.push(Shape {
            name,
            id,
            bounds,
            records: path,
        });
    }
    if source.pos != bytes.len() {
        return Err(invalid("CSH resource has trailing data"));
    }
    Ok(shapes)
}

fn finish_path(current: Option<(bool, usize)>) -> Result<()> {
    if current.is_some_and(|(_, left)| left != 0) {
        return Err(invalid("CSH subpath knot count is incomplete"));
    }
    Ok(())
}

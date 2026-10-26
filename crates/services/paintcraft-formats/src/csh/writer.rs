//! ag-psd writes the CSH container/name/ID header. Its path writer discards
//! selector 6, reorders controls, resets the origin and converts fixed integers
//! through pixel-space floats. Replace that incomplete body with exact records.
use super::*;

pub(super) fn encode(shapes: &[Shape], check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    if shapes.len() > SHAPES_MAX {
        return Err(resource("CSH shape allowance exceeded"));
    }
    let (mut names, mut records, mut size) = (0usize, 0usize, 12usize);
    // Bound every variable-size input before invoking the selected writer.
    for shape in shapes {
        check()?;
        if shape.name.len() > NAME_UNITS_MAX * 4 || shape.name.contains('\0') {
            return Err(invalid("CSH name is oversized or contains a null"));
        }
        let units = shape.name.encode_utf16().count();
        names += units * 2;
        records = records
            .checked_add(shape.records.len())
            .ok_or_else(|| resource("CSH record count overflow"))?;
        if units > NAME_UNITS_MAX || names > NAME_BYTES_MAX || records > RECORDS_MAX {
            return Err(resource("CSH name/path allowance exceeded"));
        }
        if shape.id.len() > 255 || !shape.id.bytes().all(|b| b.is_ascii() && b != 0) {
            return Err(invalid(
                "CSH resource ID must fit a null-free ASCII Pascal string",
            ));
        }
        if shape.bounds[2] < shape.bounds[0] || shape.bounds[3] < shape.bounds[1] {
            return Err(invalid("CSH bounds are inverted"));
        }
        size += 4 + units * 2;
        size += (4 - size % 4) % 4;
        size += 8 + 1 + shape.id.len() + 16 + shape.records.len() * 26;
        if size > BYTES_MAX {
            return Err(resource("CSH output byte allowance exceeded"));
        }
    }
    let mut bytes = b"cush\0\0\0\x02".to_vec();
    bytes.extend((shapes.len() as u32).to_be_bytes());
    for shape in shapes {
        check()?;
        let header = ag_psd::csh::write_csh(&ag_psd::csh::Csh {
            shapes: vec![ag_psd::csh::CshShape {
                name: shape.name.clone(),
                id: shape.id.clone(),
                width: shape.bounds[3] - shape.bounds[1],
                height: shape.bounds[2] - shape.bounds[0],
                mask: Default::default(),
            }],
        });
        check()?;
        preflight::inspect(&header, check)?;
        let name_end = 16 + shape.name.encode_utf16().count() * 2;
        let body_start = name_end.next_multiple_of(4) + 8;
        bytes.extend(&header[12..name_end]);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes.extend(1u32.to_be_bytes());
        let body_size = 1 + shape.id.len() + 16 + shape.records.len() * 26;
        bytes.extend((body_size as u32).to_be_bytes());
        bytes.extend(&header[body_start..body_start + 1 + shape.id.len()]);
        for value in shape.bounds {
            bytes.extend(value.to_be_bytes());
        }
        for record in &shape.records {
            check()?;
            bytes.extend(record_bytes(record));
        }
    }
    if bytes.len() != size {
        return Err(invalid("CSH output size disagrees with admission"));
    }
    preflight::inspect(&bytes, check)?;
    Ok(bytes)
}

fn record_bytes(record: &Record) -> [u8; 26] {
    let mut result = [0; 26];
    let selector: u16 = match record {
        Record::Subpath {
            open,
            knots,
            operation,
            fill_rule,
        } => {
            result[2..4].copy_from_slice(&knots.to_be_bytes());
            let operation: i16 = match operation {
                None => -1,
                Some(Operation::Exclude) => 0,
                Some(Operation::Combine) => 1,
                Some(Operation::Subtract) => 2,
                Some(Operation::Intersect) => 3,
            };
            result[4..6].copy_from_slice(&operation.to_be_bytes());
            let flags: u16 = match fill_rule {
                PathFillRule::EvenOddLegacy => 0,
                PathFillRule::EvenOdd => 1,
                PathFillRule::NonZero => 2,
            };
            result[6..8].copy_from_slice(&flags.to_be_bytes());
            if *open { 3 } else { 0 }
        }
        Record::Knot {
            open,
            linked,
            points,
        } => {
            for (slot, value) in result[2..].as_chunks_mut::<4>().0.iter_mut().zip([
                points[1], points[0], points[3], points[2], points[5], points[4],
            ]) {
                *slot = value.to_be_bytes();
            }
            match (*open, *linked) {
                (false, true) => 1,
                (false, false) => 2,
                (true, true) => 4,
                (true, false) => 5,
            }
        }
        Record::FillRule(rule) => {
            result[2..4].copy_from_slice(&rule.to_be_bytes());
            6
        }
        Record::Clipboard(values) => {
            for (slot, value) in result[2..22].as_chunks_mut::<4>().0.iter_mut().zip(values) {
                *slot = value.to_be_bytes();
            }
            7
        }
        Record::InitialFill(all) => {
            result[2..4].copy_from_slice(&u16::from(*all).to_be_bytes());
            8
        }
    };
    result[..2].copy_from_slice(&selector.to_be_bytes());
    result
}

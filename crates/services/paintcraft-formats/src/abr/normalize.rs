//! The pinned Rust reader lacks 16-bit PackBits. Feed it a private equivalent
//! raw-16 sample view, keeping the original resource sections and exact masks.
use super::{
    preflight::{Cursor, Prepared},
    *,
};
use std::borrow::Cow;

pub(super) fn view<'a>(
    original: &'a [u8],
    prepared: &Prepared,
    check: &dyn Fn() -> Result<()>,
) -> Result<Cow<'a, [u8]>> {
    check()?;
    if !prepared.normalize {
        return Ok(Cow::Borrowed(original));
    }
    let size = size(prepared, check)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(size)
        .map_err(|_| resource("cannot allocate normalized ABR backend view"))?;
    result.extend_from_slice(&original[..4]);
    let mut samples = prepared.samples.iter();
    for section in &prepared.sections {
        check()?;
        if section.kind == SectionKind::Patterns {
            continue;
        }
        let key: &[u8; 4] = match section.kind {
            SectionKind::Samples => b"samp",
            SectionKind::Descriptors => b"desc",
            SectionKind::Hierarchy => b"phry",
            SectionKind::Patterns => b"patt",
        };
        result.extend_from_slice(b"8BIM");
        result.extend_from_slice(key);
        let length_offset = result.len();
        result.extend_from_slice(&0u32.to_be_bytes());
        let start = result.len();
        if section.kind == SectionKind::Samples {
            let mut input = Cursor {
                bytes: &section.payload,
                pos: 0,
            };
            while input.pos < input.bytes.len() {
                check()?;
                let length = input.u32()? as usize;
                let data = input.take(length)?;
                let source = samples
                    .next()
                    .ok_or_else(|| invalid("ABR normalization sample inventory mismatch"))?;
                let header = header_len(data, prepared.minor_version)?;
                if is_rle16(data, header)? {
                    write_sample(
                        &mut result,
                        data,
                        header,
                        source,
                        prepared.minor_version,
                        check,
                    )?;
                } else {
                    result.extend_from_slice(&(length as u32).to_be_bytes());
                    result.extend_from_slice(data);
                    result.resize(result.len().next_multiple_of(4), 0);
                }
                input.zeros((4 - length % 4) % 4)?;
            }
        } else {
            result.extend_from_slice(&section.payload);
        }
        let length = result.len() - start;
        result[length_offset..length_offset + 4].copy_from_slice(&(length as u32).to_be_bytes());
        result.resize(result.len().next_multiple_of(4), 0);
        if result.len() > NORMALIZED_BYTES_MAX {
            return Err(resource("normalized ABR byte allowance exceeded"));
        }
    }
    if samples.next().is_some() || result.len() != size {
        return Err(invalid("ABR normalized output disagrees with admission"));
    }
    check()?;
    Ok(Cow::Owned(result))
}

fn size(prepared: &Prepared, check: &dyn Fn() -> Result<()>) -> Result<usize> {
    let mut result = 4usize;
    let mut samples = prepared.samples.iter();
    for section in &prepared.sections {
        check()?;
        if section.kind == SectionKind::Patterns {
            continue;
        }
        let mut length = section.payload.len();
        if section.kind == SectionKind::Samples {
            length = 0;
            let mut input = Cursor {
                bytes: &section.payload,
                pos: 0,
            };
            while input.pos < input.bytes.len() {
                check()?;
                let count = input.u32()? as usize;
                let data = input.take(count)?;
                let sample = samples
                    .next()
                    .ok_or_else(|| invalid("ABR normalization sample inventory mismatch"))?;
                let header = header_len(data, prepared.minor_version)?;
                let count = if is_rle16(data, header)? {
                    header + sample.alpha.len() * 2
                } else {
                    count
                };
                length += 4 + count.next_multiple_of(4);
                input.zeros((4 - data.len() % 4) % 4)?;
            }
        }
        result = result
            .checked_add(12 + length.next_multiple_of(4))
            .ok_or_else(|| resource("normalized ABR extent overflow"))?;
        if result > NORMALIZED_BYTES_MAX {
            return Err(resource("normalized ABR byte allowance exceeded"));
        }
    }
    if samples.next().is_some() {
        return Err(invalid("ABR normalization sample inventory mismatch"));
    }
    Ok(result)
}

fn header_len(data: &[u8], minor: u16) -> Result<usize> {
    let id = usize::from(
        *data
            .first()
            .ok_or_else(|| invalid("missing normalized ABR sample ID"))?,
    );
    let header = 1 + id + if minor == 1 { 10 } else { 264 } + 16 + 2 + 1;
    if data.len() < header {
        return Err(invalid("truncated normalized ABR sample header"));
    }
    Ok(header)
}
fn is_rle16(data: &[u8], header: usize) -> Result<bool> {
    let metadata = data
        .get(header - 3..header)
        .ok_or_else(|| invalid("truncated normalized ABR depth/compression"))?;
    Ok(metadata == [0, 16, 1])
}
fn write_sample(
    output: &mut Vec<u8>,
    original: &[u8],
    header: usize,
    sample: &Sample,
    minor: u16,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let Alpha::U16(values) = &sample.alpha else {
        return Err(invalid("ABR 16-bit normalization lost exact source values"));
    };
    let length = header + values.len() * 2;
    output.extend_from_slice(&(length as u32).to_be_bytes());
    let start = output.len();
    output.extend_from_slice(&original[..header - 1]);
    output.push(0); // raw16, retaining ID, opaque header bytes, bounds and depth
    let width = (sample.bounds[3] - sample.bounds[1]) as usize;
    for row in values.chunks(width) {
        check()?;
        for value in row {
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
    // Check every emitted low bit and all typed metadata before the selected
    // library gets the private view; it will separately verify the high bytes.
    let (verified, normalized) = preflight::sample(&output[start..], minor, &mut 0, check)?;
    if normalized
        || verified != *sample
        || output[start..start + header - 1] != original[..header - 1]
    {
        return Err(invalid(
            "ABR normalized sample changed source metadata or values",
        ));
    }
    output.resize(output.len().next_multiple_of(4), 0);
    Ok(())
}

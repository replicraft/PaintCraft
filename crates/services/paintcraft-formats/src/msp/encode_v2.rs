//! Fresh LinS row-RLE from exact monochrome samples. No zero-length row inference.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V2WritePlan {
    pub packed_bytes: usize,
    pub encoded_bytes: usize,
    /// Borrowed packed source + encoded output; no full-image codec scratch.
    pub transient_bytes: usize,
}
/// Walk one row deterministically. Repeats of at least 3 bytes use (0,count,value),
/// literals contain 1..255 bytes. Each packet stays inside this row and each step
/// consumes source. The same walker prices output before any output allocation.
pub(super) fn packets(
    row: &[u8],
    check: &dyn Fn() -> Result<()>,
    mut emit: impl FnMut(&[u8]),
) -> Result<usize> {
    let mut i = 0;
    let mut bytes = 0;
    let mut steps = 0usize;
    while i < row.len() {
        if steps.is_multiple_of(256) {
            check()?;
        }
        steps += 1;
        let run = |at: usize| {
            let mut n = 1;
            while n < 255 && at + n < row.len() && row[at + n] == row[at] {
                n += 1;
            }
            n
        };
        let n = run(i);
        if n >= 3 {
            emit(&[0, n as u8, row[i]]);
            bytes += 3;
            i += n;
        } else {
            let start = i;
            i += n;
            while i < row.len() && i - start < 255 {
                let n = run(i);
                if n >= 3 {
                    break;
                }
                i += n.min(255 - (i - start));
            }
            emit(&[(i - start) as u8]);
            emit(&row[start..i]);
            bytes += 1 + i - start;
        }
    }
    Ok(bytes)
}
pub fn preflight_v2(
    width: u32,
    height: u32,
    packed: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<V2WritePlan> {
    check()?;
    let (_, row) = geometry(width, height)?;
    if packed.len() != row * height as usize {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "MSP2 packed extent disagrees with dimensions",
        ));
    }
    let mut encoded_bytes = 32 + height as usize * 2;
    for row in packed.chunks_exact(row) {
        check()?;
        let length = packets(row, check, |_| {})?;
        if length == 0 || length > usize::from(u16::MAX) {
            return Err(limit("MSP2 encoded row exceeds nonzero u16 map"));
        }
        encoded_bytes = encoded_bytes
            .checked_add(length)
            .filter(|n| *n <= BYTES_MAX)
            .ok_or_else(|| limit("MSP2 encoded output allowance"))?;
    }
    check()?;
    Ok(V2WritePlan {
        packed_bytes: packed.len(),
        encoded_bytes,
        transient_bytes: packed.len() + encoded_bytes,
    })
}
/// Encode exact borrowed packed rows. Unused last-byte bits are retained. No
/// metadata-preserving Save, physical-resolution inference or implicit threshold.
pub fn encode_v2(
    width: u32,
    height: u32,
    packed: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let plan = preflight_v2(width, height, packed, check)?;
    let (_, row_bytes) = geometry(width, height)?;
    let mut output = buffer(plan.encoded_bytes)?;
    output.extend_from_slice(&super::encode::header(width, height, *b"LinS"));
    output.resize(32 + height as usize * 2, 0);
    for (index, row) in packed.chunks_exact(row_bytes).enumerate() {
        check()?;
        let length = packets(row, check, |part| output.extend_from_slice(part))?;
        output[32 + index * 2..34 + index * 2].copy_from_slice(&(length as u16).to_le_bytes());
    }
    check()?;
    if output.len() != plan.encoded_bytes {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "MSP2 output disagrees with preflight",
        ));
    }
    Ok(output)
}
/// Exact opaque black/white only. Temporary packed pixels add at most ceil(w/8)*h
/// bytes to the supplied RGBA source and encoded output; no RGBA clone is made.
pub fn encode_v2_rgba_exact(
    width: u32,
    height: u32,
    rgba: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let packed = super::encode::pack_rgba_exact(width, height, rgba, check)?;
    encode_v2(width, height, &packed, check)
}

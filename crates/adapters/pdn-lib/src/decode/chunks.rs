use super::*;
use flate2::{Decompress, FlushDecompress, Status};
fn take<'a>(bytes: &'a [u8], position: &mut usize, n: usize) -> Result<&'a [u8]> {
    let end = position
        .checked_add(n)
        .ok_or_else(|| resource("PDN pixel offset overflow"))?;
    let value = bytes
        .get(*position..end)
        .ok_or_else(|| malformed("Truncated PDN deferred pixel data"))?;
    *position = end;
    Ok(value)
}
fn u32be(bytes: &[u8], position: &mut usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        take(bytes, position, 4)?.try_into().unwrap(),
    ))
}
fn gunzip(
    bytes: &[u8],
    output: &mut [u8],
    limits: &DecodeLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let header = body::inspect(bytes, &limits.envelope, check)?;
    let BodyHeader::Gzip {
        flags,
        header_bytes,
    } = header
    else {
        return Err(malformed("PDN compressed chunk must be gzip"));
    };
    if flags & 2 != 0 {
        let expected =
            u16::from_le_bytes(bytes[header_bytes - 2..header_bytes].try_into().unwrap());
        if crc32fast::hash(&bytes[..header_bytes - 2]) as u16 != expected {
            return Err(malformed("PDN gzip header CRC mismatch"));
        }
    }
    let trailer = &bytes[bytes.len() - 8..];
    let crc = u32::from_le_bytes(trailer[..4].try_into().unwrap());
    let length = u32::from_le_bytes(trailer[4..].try_into().unwrap());
    if output.len() as u64 != u64::from(length) {
        return Err(malformed("PDN gzip trailer size disagrees with chunk"));
    }
    let input = &bytes[header_bytes..bytes.len() - 8];
    let mut decoder = Decompress::new(false);
    let mut consumed = 0usize;
    let mut written = 0usize;
    let mut excess = [0u8; 1];
    loop {
        check()?;
        let old_in = decoder.total_in();
        let old_out = decoder.total_out();
        let chunk = &input[consumed..input.len().min(consumed.saturating_add(65536))];
        let target = if written < output.len() {
            let end = output.len().min(written.saturating_add(65536));
            &mut output[written..end]
        } else {
            &mut excess
        };
        let status = decoder
            .decompress(chunk, target, FlushDecompress::None)
            .map_err(|_| malformed("Invalid PDN gzip DEFLATE payload"))?;
        let read = (decoder.total_in() - old_in) as usize;
        let produced = (decoder.total_out() - old_out) as usize;
        consumed += read;
        written += produced;
        if written > output.len() {
            return Err(malformed("PDN gzip output exceeds declared pixels"));
        }
        if status == Status::StreamEnd {
            if written != output.len() || consumed != input.len() {
                return Err(malformed("PDN gzip payload length mismatch"));
            }
            break;
        }
        if read == 0 && produced == 0 {
            return Err(malformed("Truncated PDN gzip DEFLATE stream"));
        }
    }
    let mut hash = crc32fast::Hasher::new();
    for chunk in output.chunks(65536) {
        check()?;
        hash.update(chunk);
    }
    if hash.finalize() != crc {
        return Err(malformed("PDN gzip pixel CRC mismatch"));
    }
    Ok(())
}
pub(super) fn decode(
    bytes: &[u8],
    expected: usize,
    limits: &DecodeLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Vec<u8>, usize)> {
    check()?;
    let mut position = 0;
    let format = take(bytes, &mut position, 1)?[0];
    if format > 1 {
        return Err(unsupported("Unsupported PDN deferred pixel version"));
    }
    let size = u32be(bytes, &mut position)? as usize;
    if size == 0 {
        return Err(malformed("PDN deferred chunk size must be positive"));
    }
    let count = expected.div_ceil(size);
    if count == 0 || count > limits.chunks || count > limits.graph_metadata_bytes {
        return Err(resource("PDN deferred chunk count exceeds limits"));
    }
    let mut seen = vec_checked(count, limits.graph_metadata_bytes)?;
    seen.resize(count, false);
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(expected)
        .map_err(|_| resource("Cannot allocate PDN layer pixels"))?;
    rgba.resize(expected, 0);
    for _ in 0..count {
        check()?;
        let index = u32be(bytes, &mut position)? as usize;
        if index >= count || seen[index] {
            return Err(malformed("PDN chunk index is invalid or duplicated"));
        }
        seen[index] = true;
        let length = u32be(bytes, &mut position)? as usize;
        let data = take(bytes, &mut position, length)?;
        let start = index
            .checked_mul(size)
            .ok_or_else(|| resource("PDN pixel chunk offset overflow"))?;
        let end = expected.min(start.saturating_add(size));
        if format == 0 {
            gunzip(data, &mut rgba[start..end], limits, check)?;
        } else {
            if data.len() != end - start {
                return Err(malformed(
                    "PDN raw chunk size disagrees with expected pixels",
                ));
            }
            for (source, target) in data.chunks(65536).zip(rgba[start..end].chunks_mut(65536)) {
                check()?;
                target.copy_from_slice(source);
            }
        }
    }
    for pixels in rgba.chunks_mut(65536) {
        check()?;
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
    }
    check()?;
    Ok((rgba, position))
}

//! Exact RGBA runs: little-endian u16 (pixel count minus one), then RGBA.
//! One run covers up to a complete tile; no canvas-sized decoder allocation.
use super::*;

pub(super) fn encode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut pixels = data.chunks_exact(4).peekable();
    while let Some(pixel) = pixels.next() {
        let mut count = 1u32;
        while count < 65_536 && pixels.peek().is_some_and(|next| *next == pixel) {
            pixels.next();
            count += 1;
        }
        output.extend_from_slice(&((count - 1) as u16).to_le_bytes());
        output.extend_from_slice(pixel);
    }
    output
}

pub(super) fn decode(
    bytes: &[u8],
    length: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    if length == 0
        || length > BLOCK_BYTES_MAX as usize
        || !length.is_multiple_of(4)
        || !bytes.len().is_multiple_of(6)
    {
        return Err(codec("invalid RGBA run extent"));
    }
    check()?;
    let mut output = Vec::new();
    output.try_reserve_exact(length).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "cannot allocate decoded pixel runs",
        )
    })?;
    for run in bytes.chunks_exact(6) {
        check()?;
        let count = usize::from(u16::from_le_bytes([run[0], run[1]])) + 1;
        if count * 4 > length - output.len() {
            return Err(codec("RGBA run exceeds block extent"));
        }
        for _ in 0..count {
            output.extend_from_slice(&run[2..]);
        }
    }
    if output.len() != length {
        return Err(codec("incomplete RGBA runs"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_preserve_hidden_rgb_boundaries_and_reject_malformed_extents() {
        for data in [
            vec![255; 262_144],
            vec![17, 34, 51, 0],
            [vec![255; 8192], vec![17, 34, 51, 0], vec![255; 4096]].concat(),
        ] {
            let runs = encode(&data);
            assert_eq!(decode(&runs, data.len(), &|| Ok(())).unwrap(), data);
            for cut in 0..runs.len() {
                assert!(decode(&runs[..cut], data.len(), &|| Ok(())).is_err());
            }
            let mut extra = runs.clone();
            extra.extend_from_slice(&[0; 6]);
            assert!(decode(&extra, data.len(), &|| Ok(())).is_err());
        }
        assert!(decode(&[255, 255, 0, 0, 0, 0], 4, &|| Ok(())).is_err());
        assert!(decode(&[], BLOCK_BYTES_MAX as usize + 4, &|| Ok(())).is_err());
    }
}

use super::*;
/// A fresh MSP1 file from exact MSB-first packed monochrome rows. This is not
/// metadata-preserving Save of a decoded source and does not emit MSP2.
pub fn encode_v1(
    width: u32,
    height: u32,
    packed: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let (_, row) = geometry(width, height)?;
    if packed.len() != row * height as usize {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "MSP packed extent disagrees with dimensions",
        ));
    }
    let mut bytes = buffer(32 + packed.len())?;
    bytes.extend_from_slice(&header(width, height, *b"DanM"));
    for chunk in packed.chunks(65536) {
        check()?;
        bytes.extend_from_slice(chunk);
    }
    check()?;
    Ok(bytes)
}
/// Fresh encoding only when the caller already supplied exact opaque black or
/// white pixels. Alpha removal, thresholding and dithering are never implicit.
pub fn encode_v1_rgba_exact(
    width: u32,
    height: u32,
    rgba: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let packed = pack_rgba_exact(width, height, rgba, check)?;
    encode_v1(width, height, &packed, check)
}
pub(super) fn pack_rgba_exact(
    width: u32,
    height: u32,
    rgba: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let (width_usize, row) = geometry(width, height)?;
    if rgba.len() != width_usize * height as usize * 4 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "MSP RGBA extent disagrees with dimensions",
        ));
    }
    let mut packed = buffer(row * height as usize)?;
    packed.resize(row * height as usize, 0);
    for y in 0..height as usize {
        let rgba_row = &rgba[y * width_usize * 4..(y + 1) * width_usize * 4];
        pack_row_rgba_exact(rgba_row, &mut packed[y * row..(y + 1) * row], check)?;
    }
    check()?;
    Ok(packed)
}

/// One canonical fresh packed row; shared by allocation-free counting and encode.
/// The caller supplies exactly ceil(pixel_count/8) bytes. Unused tail bits zero.
pub(super) fn pack_row_rgba_exact(
    rgba: &[u8],
    packed: &mut [u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if !rgba.len().is_multiple_of(4) || packed.len() != (rgba.len() / 4).div_ceil(8) {
        return Err(bad("MSP packed row extent disagrees with RGBA"));
    }
    packed.fill(0);
    for (x, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if x.is_multiple_of(4096) {
            check()?;
        }
        if pixel != &[0, 0, 0, 255] && pixel != &[255, 255, 255, 255] {
            return Err(unsupported(
                "MSP requires explicit opaque black/white conversion before export",
            ));
        }
        if pixel[0] == 255 {
            packed[x / 8] |= 0x80 >> (x % 8);
        }
    }
    Ok(())
}

pub(super) fn header(width: u32, height: u32, magic: [u8; 4]) -> [u8; 32] {
    let mut words = [0u16; 16];
    words[0] = u16::from_le_bytes([magic[0], magic[1]]);
    words[1] = u16::from_le_bytes([magic[2], magic[3]]);
    words[2] = width as u16;
    words[3] = height as u16;
    words[4..8].fill(1);
    words[8] = width as u16;
    words[9] = height as u16;
    words[12] = words.iter().fold(0, |sum, word| sum ^ word);
    let mut bytes = [0; 32];
    for (word, slot) in words.into_iter().zip(bytes.as_chunks_mut::<2>().0) {
        *slot = word.to_le_bytes();
    }
    bytes
}

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
    let mut words = [0u16; 16];
    words[0] = u16::from_le_bytes(*b"Da");
    words[1] = u16::from_le_bytes(*b"nM");
    words[2] = width as u16;
    words[3] = height as u16;
    words[4..8].fill(1);
    words[8] = width as u16;
    words[9] = height as u16;
    words[12] = words.iter().fold(0, |sum, word| sum ^ word);
    let mut bytes = buffer(32 + packed.len())?;
    for word in words {
        bytes.extend(word.to_le_bytes());
    }
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
        check()?;
        for x in 0..width_usize {
            let index = (y * width_usize + x) * 4;
            let pixel = &rgba[index..index + 4];
            if pixel != [0, 0, 0, 255] && pixel != [255, 255, 255, 255] {
                return Err(unsupported(
                    "MSP requires explicit opaque black/white conversion before export",
                ));
            }
            if pixel[0] == 255 {
                packed[y * row + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    encode_v1(width, height, &packed, check)
}

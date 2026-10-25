//! PCX v5 RGB24 and packed/planar indexed conversion using the selected pcx crate.
use super::*;

struct Profile {
    width: u32,
    height: u32,
    count: u64,
    paletted: bool,
    colors: usize,
}
fn admit(bytes: &[u8], pixels: u64) -> Result<Profile> {
    let h = bytes
        .get(..128)
        .ok_or_else(|| codec("truncated PCX header"))?;
    if h[0] != 10 {
        return Err(codec("invalid PCX signature"));
    }
    let (bits, planes) = (h[3], h[65]);
    if h[1] != 5 || h[2] != 1 || !matches!((bits, planes), (8, 3) | (1 | 2 | 4 | 8, 1) | (1, 2..=4))
    {
        return Err(unsupported(
            "PCX conversion supports v5 RLE packed/planar indexed and RGB24 only",
        ));
    }
    if h[64] != 0 || h[74..128].iter().any(|v| *v != 0) {
        return Err(unsupported(
            "PCX reserved header extensions are not qualified",
        ));
    }
    let word = |at| u16::from_le_bytes([h[at], h[at + 1]]);
    let width = u32::from(
        word(8)
            .checked_sub(word(4))
            .ok_or_else(|| codec("invalid PCX horizontal extent"))?,
    ) + 1;
    let height = u32::from(
        word(10)
            .checked_sub(word(6))
            .ok_or_else(|| codec("invalid PCX vertical extent"))?,
    ) + 1;
    // The selected crate represents dimensions with u16 and rejects 65536.
    if width > u32::from(u16::MAX) || height > u32::from(u16::MAX) {
        return Err(unsupported("PCX extent exceeds selected codec dimensions"));
    }
    let count = admit_dimensions(width, height, pixels)?;
    let lane = u32::from(word(66));
    let proper = (width * u32::from(bits)).div_ceil(8);
    if lane < proper || lane > proper + 1 || lane % 2 != 0 {
        return Err(unsupported("PCX requires minimally padded even scanlines"));
    }
    let paletted = (bits, planes) != (8, 3);
    let colors = if paletted {
        1usize << (u32::from(bits) * u32::from(planes))
    } else {
        0
    };
    if colors == 2 && h[16..22] != [0, 0, 0, 255, 255, 255] {
        return Err(unsupported(
            "PCX monochrome header palette must be black and white for the selected codec",
        ));
    }
    let payload_end = if colors == 256 {
        let end = bytes
            .len()
            .checked_sub(769)
            .filter(|end| *end >= 128)
            .ok_or_else(|| codec("truncated PCX palette"))?;
        if bytes[end] != 12 {
            return Err(codec("missing PCX 256-color palette marker"));
        }
        end
    } else {
        bytes.len()
    };
    let mut at = 128usize;
    for _ in 0..u64::from(height) * u64::from(h[65]) {
        let mut produced = 0u32;
        while produced < lane {
            let tag = *bytes
                .get(at)
                .filter(|_| at < payload_end)
                .ok_or_else(|| codec("truncated PCX scanline"))?;
            at += 1;
            let run = if tag & 0xc0 == 0xc0 {
                let run = u32::from(tag & 63);
                if run == 0 || at >= payload_end {
                    return Err(codec("invalid PCX RLE packet"));
                }
                at += 1;
                run
            } else {
                1
            };
            produced += run;
            if produced > lane {
                return Err(codec("PCX RLE packet crosses a plane boundary"));
            }
        }
    }
    if at != payload_end {
        return Err(codec("PCX trailing data is not qualified"));
    }
    Ok(Profile {
        width,
        height,
        count,
        paletted,
        colors,
    })
}
/// Header, exact RLE boundaries and palette admission without decoded allocation.
pub(crate) fn inspect(bytes: &[u8], pixels: u64) -> Result<(u32, u32)> {
    let profile = admit(bytes, pixels)?;
    Ok((profile.width, profile.height))
}
pub(crate) fn decode(bytes: &[u8], pixels: u64) -> Result<Decoded> {
    let Profile {
        width,
        height,
        count,
        paletted,
        colors,
    } = admit(bytes, pixels)?;
    let mut reader = ::pcx::Reader::from_mem(bytes).map_err(|e| codec(e.to_string()))?;
    let mut rgba = buffer(count * 4)?;
    if paletted {
        let mut palette = [0u8; 768];
        if reader
            .get_palette(&mut palette)
            .map_err(|e| codec(e.to_string()))?
            != colors
        {
            return Err(codec("PCX palette length mismatch"));
        }
        let mut row = buffer(u64::from(width))?;
        for y in 0..height as usize {
            // Avoid read_rgb_pixels: it intentionally tolerates truncated
            // paletted rows. Each admitted row must finish successfully here.
            reader
                .next_row_paletted(&mut row)
                .map_err(|e| codec(e.to_string()))?;
            for (x, index) in row.iter().enumerate() {
                let dst = (y * width as usize + x) * 4;
                let source = usize::from(*index) * 3;
                rgba[dst..dst + 3].copy_from_slice(&palette[source..source + 3]);
                rgba[dst + 3] = 255;
            }
        }
    } else {
        let mut row = buffer(u64::from(width) * 3)?;
        for y in 0..height as usize {
            reader
                .next_row_rgb(&mut row)
                .map_err(|e| codec(e.to_string()))?;
            for x in 0..width as usize {
                let dst = (y * width as usize + x) * 4;
                rgba[dst..dst + 3].copy_from_slice(&row[x * 3..x * 3 + 3]);
                rgba[dst + 3] = 255;
            }
        }
    }
    let mut losses = vec!["pcx-metadata-ignored".into()];
    if paletted {
        losses.push("palette-indices-expanded".into());
    }
    Ok(Decoded {
        format: ImageFormat::Pcx,
        width,
        height,
        rgba,
        losses,
    })
}

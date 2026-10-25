//! Bounded TGA integer-pixel subset. Extended color/alpha semantics are refused.
use super::*;

const FOOTER: &[u8] = b"TRUEVISION-XFILE.\0";
pub(super) fn sniff(bytes: &[u8]) -> bool {
    if bytes.ends_with(FOOTER) {
        return true;
    }
    bytes.get(..18).is_some_and(|h| {
        h[1] <= 1
            && matches!(h[2], 1 | 2 | 3 | 9 | 10 | 11)
            && matches!(h[16], 8 | 16 | 24 | 32)
            && h[17] & 0xc0 == 0
            && h[12..14] != [0, 0]
            && h[14..16] != [0, 0]
    })
}
fn take<'a>(bytes: &'a [u8], pos: &mut usize, length: usize) -> Result<&'a [u8]> {
    let end = pos
        .checked_add(length)
        .ok_or_else(|| codec("TGA offset overflow"))?;
    let result = bytes
        .get(*pos..end)
        .ok_or_else(|| codec("truncated TGA payload"))?;
    *pos = end;
    Ok(result)
}
pub(super) fn admit(bytes: &[u8], pixels: u64) -> Result<Vec<String>> {
    let h = bytes
        .get(..18)
        .ok_or_else(|| codec("truncated TGA header"))?;
    let word = |at| u16::from_le_bytes([h[at], h[at + 1]]) as u64;
    let (width, height) = (word(12), word(14));
    check_size(width, height)?;
    let count = width * height;
    if count > pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "TGA exceeds the pixel budget",
        ));
    }
    if h[17] & 0xc0 != 0 {
        return Err(unsupported("interleaved TGA is not qualified"));
    }
    let mapped = matches!(h[2], 1 | 9);
    let gray = matches!(h[2], 3 | 11);
    let rle = matches!(h[2], 9..=11);
    if !matches!(h[2], 1 | 2 | 3 | 9 | 10 | 11) {
        return Err(unsupported("TGA image type is not qualified"));
    }
    let alpha = h[17] & 15;
    let palette_depth = h[7];
    if mapped {
        if h[1] != 1
            || !matches!(h[16], 8 | 16)
            || !matches!((palette_depth, alpha), (24, 0) | (32, 8))
        {
            return Err(unsupported(
                "TGA palette/index/alpha profile is not qualified",
            ));
        }
    } else {
        if h[1] != 0 || h[3..8] != [0; 5] {
            return Err(unsupported("unused TGA palettes are not qualified"));
        }
        let allowed = if gray {
            matches!((h[16], alpha), (8, 0) | (16, 8))
        } else {
            matches!((h[16], alpha), (24, 0) | (32, 8))
        };
        if !allowed {
            return Err(unsupported(
                "TGA depth or ambiguous alpha profile is not qualified",
            ));
        }
    }
    let mut pos = 18;
    take(bytes, &mut pos, h[0] as usize)?;
    let (first, entries) = (word(3), word(5));
    if mapped {
        if entries == 0 || first + entries > if h[16] == 8 { 256 } else { 65536 } {
            return Err(codec("invalid TGA palette domain"));
        }
        take(
            bytes,
            &mut pos,
            entries as usize * usize::from(palette_depth / 8),
        )?;
    }
    let stride = usize::from(h[16] / 8);
    let check_index = |sample: &[u8]| -> Result<()> {
        if mapped {
            let index = if stride == 1 {
                u64::from(sample[0])
            } else {
                u64::from(u16::from_le_bytes([sample[0], sample[1]]))
            };
            if index < first || index >= first + entries {
                return Err(codec("TGA palette index outside declared domain"));
            }
        }
        Ok(())
    };
    let mut remaining = count as usize;
    while remaining != 0 {
        let (run, repeated) = if rle {
            let packet = take(bytes, &mut pos, 1)?[0];
            (usize::from(packet & 127) + 1, packet & 128 != 0)
        } else {
            (remaining, false)
        };
        if run > remaining {
            return Err(codec("TGA RLE packet exceeds image dimensions"));
        }
        let data = take(
            bytes,
            &mut pos,
            if repeated { stride } else { run * stride },
        )?;
        if mapped {
            for sample in data.chunks(stride) {
                check_index(sample)?;
            }
        }
        remaining -= run;
    }
    if pos != bytes.len() {
        let footer = take(bytes, &mut pos, 26)?;
        if pos != bytes.len() || &footer[8..] != FOOTER {
            return Err(codec("unrecognized TGA trailing data"));
        }
        if footer[..8] != [0; 8] {
            return Err(unsupported(
                "TGA extension/developer color and alpha semantics are not qualified",
            ));
        }
    }
    let mut losses = Vec::new();
    if mapped {
        losses.push("palette-indices-expanded".into());
    }
    if h[0] != 0 || h[8..12] != [0; 4] {
        losses.push("tga-metadata-ignored".into());
    }
    Ok(losses)
}
pub(super) fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    if width > u16::MAX as u32 || height > u16::MAX as u32 {
        return Err(unsupported("TGA supports at most 65535 pixels per side"));
    }
    let mut output = Vec::new();
    ::image::codecs::tga::TgaEncoder::new(&mut output)
        .write_image(rgba, width, height, ::image::ExtendedColorType::Rgba8)
        .map_err(image_error)?;
    output.extend([0; 8]);
    output.extend(FOOTER);
    Ok(output)
}

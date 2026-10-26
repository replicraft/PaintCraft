//! Small RGB/mask interoperability profile: exact-length interleaved raw RGB, otherwise
//! three separately bounded ICNS RLE channels. No guessed four-zero prefix.
use super::*;
pub(super) fn mask_tag(tag: [u8; 4]) -> Option<[u8; 4]> {
    match &tag {
        b"is32" => Some(*b"s8mk"),
        b"il32" => Some(*b"l8mk"),
        _ => None,
    }
}
pub(super) fn payload<'a>(bytes: &'a [u8], record: &Record) -> &'a [u8] {
    &bytes[record.offset as usize + 8..(record.offset + record.length) as usize]
}
/// Walk every packet before allocating pixels. An output slice is optional so
/// inspection and decoding apply identical exact extents and channel rules.
fn channels(
    bytes: &[u8],
    count: usize,
    mut rgba: Option<&mut [u8]>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Encoding> {
    if bytes.len() == count * 3 {
        if let Some(output) = rgba {
            for (pixel, rgb) in output
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(bytes.as_chunks::<3>().0.iter())
            {
                pixel[..3].copy_from_slice(rgb);
            }
        }
        check()?;
        return Ok(Encoding::LegacyRaw);
    }
    let mut cursor = 0usize;
    for channel in 0..3 {
        let mut pixel = 0usize;
        while pixel < count {
            check()?;
            let control = *bytes
                .get(cursor)
                .ok_or_else(|| bad("truncated ICNS RGB packet"))?;
            cursor += 1;
            let run = if control < 128 {
                usize::from(control) + 1
            } else {
                usize::from(control) - 125
            };
            if run > count - pixel {
                return Err(bad("ICNS RGB packet crosses channel extent"));
            }
            let length = if control < 128 { run } else { 1 };
            let data = bytes
                .get(cursor..cursor + length)
                .ok_or_else(|| bad("truncated ICNS RGB packet data"))?;
            if let Some(output) = rgba.as_deref_mut() {
                for offset in 0..run {
                    output[(pixel + offset) * 4 + channel] =
                        data[if control < 128 { offset } else { 0 }];
                }
            }
            cursor += length;
            pixel += run;
        }
    }
    if cursor != bytes.len() {
        return Err(bad("ICNS RGB packet stream has trailing data"));
    }
    Ok(Encoding::LegacyRle)
}
pub(super) fn qualify(
    bytes: &[u8],
    directory: &mut Directory,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for entry in &mut directory.entries {
        let Some(tag) = mask_tag(entry.tag) else {
            continue;
        };
        // A mask is keyed by type, not adjacency. Multiple RGB or mask records
        // cannot be paired unambiguously, so retain them without rendering.
        if directory
            .records
            .iter()
            .filter(|record| record.tag == entry.tag)
            .count()
            != 1
        {
            continue;
        }
        let masks: Vec<_> = directory
            .records
            .iter()
            .filter(|record| record.tag == tag)
            .collect();
        if masks.len() != 1 {
            continue;
        }
        let count = (entry.width * entry.height) as usize;
        if payload(bytes, masks[0]).len() != count {
            return Err(bad("ICNS legacy alpha mask extent mismatch"));
        }
        entry.encoding = channels(
            payload(bytes, &directory.records[usize::from(entry.record)]),
            count,
            None,
            check,
        )?;
    }
    Ok(())
}
pub(super) fn decode(
    bytes: &[u8],
    directory: &Directory,
    entry: &Entry,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let count = (entry.width * entry.height) as usize;
    let mut rgba = allocation(count * 4)?;
    rgba.resize(count * 4, 0);
    let actual = channels(
        payload(bytes, &directory.records[usize::from(entry.record)]),
        count,
        Some(&mut rgba),
        check,
    )?;
    if actual != entry.encoding {
        return Err(bad("ICNS legacy preflight changed encoding"));
    }
    let tag = mask_tag(entry.tag).ok_or_else(|| bad("ICNS legacy mask type missing"))?;
    let mask = directory
        .records
        .iter()
        .find(|record| record.tag == tag)
        .ok_or_else(|| bad("ICNS legacy mask missing"))?;
    for (pixel, alpha) in rgba
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(payload(bytes, mask))
    {
        pixel[3] = *alpha;
    }
    check()?;
    Ok(rgba)
}
pub(super) fn encode(
    entry: &WriteEntry<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<::icns::IconElement>> {
    check()?;
    let side = entry.kind.pixel_size();
    let mut owned = allocation(entry.rgba.len())?;
    owned.extend_from_slice(entry.rgba);
    let image = ::icns::Image::from_data(::icns::PixelFormat::RGBA, side, side, owned)
        .map_err(|_| bad("ICNS legacy input image failed"))?;
    let kind = if side == 16 {
        ::icns::IconType::RGB24_16x16
    } else {
        ::icns::IconType::RGB24_32x32
    };
    let mut rgb = ::icns::IconElement::encode_image_with_type(&image, kind)
        .map_err(|_| bad("selected ICNS RGB encoder failed"))?;
    let count = (side * side) as usize;
    // Independent readers distinguish raw solely by exact3*N length. Preserve
    // the requested pixels while choosing unambiguous literal RLE if needed;
    // native Apple readback does not qualify interleaved raw output.
    if rgb.data.len() == count * 3 {
        rgb.data.clear();
        for channel in 0..3 {
            for pixels in entry.rgba.chunks(128 * 4) {
                rgb.data.push((pixels.len() / 4 - 1) as u8);
                for pixel in pixels.as_chunks::<4>().0.iter() {
                    rgb.data.push(pixel[channel]);
                }
            }
        }
    }
    let maskkind = if side == 16 {
        ::icns::IconType::Mask8_16x16
    } else {
        ::icns::IconType::Mask8_32x32
    };
    let mask = ::icns::IconElement::encode_image_with_type(&image, maskkind)
        .map_err(|_| bad("selected ICNS alpha encoder failed"))?;
    let mut verify = allocation(count * 4)?;
    verify.resize(count * 4, 0);
    channels(&rgb.data, count, Some(&mut verify), check)?;
    if mask.data.len() != count {
        return Err(bad("selected ICNS alpha encoder extent mismatch"));
    }
    for (pixel, alpha) in verify.as_chunks_mut::<4>().0.iter_mut().zip(&mask.data) {
        pixel[3] = *alpha;
    }
    if verify != entry.rgba {
        return Err(bad("selected ICNS legacy encoder changed RGBA"));
    }
    check()?;
    Ok(vec![rgb, mask])
}

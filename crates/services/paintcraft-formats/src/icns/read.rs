use super::*;
use std::io::Cursor;
/// Complete container and admitted PNG/RGB preflight, without pixel allocations.
/// Unqualified legacy/JPEG2000 image records remain identified and inert; they
/// cannot be selected as PNGs or silently replaced by another same-size entry.
pub fn inspect(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Directory> {
    check()?;
    if bytes.len() > BYTES_MAX {
        return Err(limit("ICNS encoded byte allowance exceeded"));
    }
    let header = bytes.get(..8).ok_or_else(|| bad("truncated ICNS header"))?;
    if &header[..4] != b"icns"
        || u32::from_be_bytes(header[4..].try_into().unwrap()) as usize != bytes.len()
    {
        return Err(bad("ICNS signature or complete file length mismatch"));
    }
    let mut directory = Directory {
        encoded_length: bytes.len() as u64,
        records: allocation(RECORDS_MAX)?,
        entries: allocation(RECORDS_MAX)?,
    };
    let (mut offset, mut pixels, mut opaque) = (8usize, 0u64, 0usize);
    while offset < bytes.len() {
        check()?;
        if directory.records.len() == RECORDS_MAX {
            return Err(limit("ICNS record allowance exceeded"));
        }
        let header = bytes
            .get(offset..offset + 8)
            .ok_or_else(|| bad("truncated ICNS record"))?;
        let tag: [u8; 4] = header[..4].try_into().unwrap();
        let length = u32::from_be_bytes(header[4..].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(length)
            .filter(|end| length >= 8 && *end <= bytes.len())
            .ok_or_else(|| bad("ICNS record length outside source"))?;
        let payload = &bytes[offset + 8..end];
        if let Some(kind) =
            ::icns::IconType::from_ostype(::icns::OSType(tag)).filter(|kind| !kind.is_mask())
        {
            // Apple IconStorage.h names ic04/ic05 as16/32-pixel ARGB.
            // icns0.5 mislabels ic05 as16@2; do not inherit that density.
            let (width, height, logical_width, logical_height, scale) = match &tag {
                b"ic04" => (16, 16, 16, 16, 1),
                b"ic05" => (32, 32, 32, 32, 1),
                _ => (
                    kind.pixel_width(),
                    kind.pixel_height(),
                    kind.screen_width(),
                    kind.screen_height(),
                    kind.pixel_density(),
                ),
            };
            pixels = pixels
                .checked_add(u64::from(width) * u64::from(height))
                .filter(|value| *value <= PIXELS_MAX)
                .ok_or_else(|| limit("ICNS aggregate pixel allowance exceeded"))?;
            if payload.len() > ENTRY_BYTES_MAX {
                return Err(limit("ICNS image payload allowance exceeded"));
            }
            let encoding = if IconType::MODERN.iter().any(|kind| kind.tag() == tag)
                && payload.starts_with(b"\x89PNG\r\n\x1a\n")
            {
                crate::icons::inspect_icns_png(payload, width, height)?;
                Encoding::Png
            } else {
                Encoding::Unqualified
            };
            directory.entries.push(Entry {
                index: directory.entries.len() as u16,
                record: directory.records.len() as u16,
                tag,
                logical_width,
                logical_height,
                scale,
                width,
                height,
                encoding,
            });
        } else {
            opaque = opaque
                .checked_add(payload.len())
                .filter(|count| *count <= OPAQUE_BYTES_MAX)
                .ok_or_else(|| limit("ICNS opaque metadata/mask allowance exceeded"))?;
        }
        directory.records.push(Record {
            tag,
            offset: offset as u64,
            length: length as u64,
        });
        offset = end;
    }
    if directory.entries.is_empty() {
        return Err(unsupported("ICNS contains no recognized image entries"));
    }
    toc(bytes, &directory)?;
    legacy::qualify(bytes, &mut directory, check)?;
    check()?;
    Ok(directory)
}
fn toc(bytes: &[u8], directory: &Directory) -> Result<()> {
    let mut found = false;
    for record in directory
        .records
        .iter()
        .filter(|record| record.tag == *b"TOC ")
    {
        if found {
            return Err(bad("duplicate ICNS TOC"));
        }
        found = true;
        let payload = &bytes[record.offset as usize + 8..(record.offset + record.length) as usize];
        let records: Vec<_> = directory
            .records
            .iter()
            .filter(|record| record.tag != *b"TOC ")
            .collect();
        if payload.len() != records.len() * 8 {
            return Err(bad("ICNS TOC record count mismatch"));
        }
        for (row, record) in payload.as_chunks::<8>().0.iter().zip(records) {
            if row[..4] != record.tag
                || u64::from(u32::from_be_bytes(row[4..].try_into().unwrap())) != record.length
            {
                return Err(bad("ICNS TOC type/length/order mismatch"));
            }
        }
    }
    Ok(())
}
fn selected_family(
    bytes: &[u8],
    directory: &Directory,
    check: &dyn Fn() -> Result<()>,
) -> Result<::icns::IconFamily> {
    check()?;
    let mut source = Cursor::new(bytes);
    let family = ::icns::IconFamily::read(&mut source)
        .map_err(|_| bad("selected ICNS container reader failed"))?;
    check()?;
    if source.position() != bytes.len() as u64 || family.elements.len() != directory.records.len() {
        return Err(bad("selected ICNS reader did not consume complete source"));
    }
    for (element, record) in family.elements.iter().zip(&directory.records) {
        check()?;
        if element.ostype.0 != record.tag
            || element.data
                != bytes[record.offset as usize + 8..(record.offset + record.length) as usize]
        {
            return Err(bad(
                "selected ICNS reader changed ordered record metadata/payload",
            ));
        }
    }
    Ok(family)
}
pub fn decode_entry(
    bytes: &[u8],
    index: u16,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    let directory = inspect(bytes, check)?;
    let entry = directory.entries.get(usize::from(index)).ok_or_else(|| {
        Error::new(
            ErrorCode::InputSchema,
            "ICNS selected entry outside directory",
        )
    })?;
    if entry.encoding == Encoding::Unqualified {
        return Err(unsupported(
            "ICNS selected entry encoding or mask pairing is unqualified",
        ));
    }
    if u64::from(entry.width) * u64::from(entry.height) > pixels {
        return Err(limit("ICNS selected entry exceeds caller pixel allowance"));
    }
    let family = selected_family(bytes, &directory, check)?;
    if matches!(entry.encoding, Encoding::LegacyRaw | Encoding::LegacyRle) {
        return Ok(Decoded {
            entry: entry.clone(),
            rgba: legacy::decode(bytes, &directory, entry, check)?,
            losses: if directory.entries.len() > 1 {
                vec![
                    "icns-metadata-ignored".into(),
                    "icon-other-entries-not-imported".into(),
                ]
            } else {
                vec!["icns-metadata-ignored".into()]
            },
        });
    }
    let payload = &family.elements[usize::from(entry.record)].data;
    check()?;
    let decoded = crate::image::decode_with_pixel_limit(payload, pixels)?;
    check()?;
    let mut losses = decoded.losses;
    // Palette expansion is reported once with the existing icon-specific code.
    losses.retain(|loss| loss != "palette-indices-expanded");
    losses.push("icns-metadata-ignored".into());
    if payload[25] == 3 {
        losses.push("icon-palette-expanded".into());
    }
    if entry.scale != 1 {
        losses.push("icon-density-not-retained-in-flat-document".into());
    }
    if directory.entries.len() > 1 {
        losses.push("icon-other-entries-not-imported".into());
    }
    Ok(Decoded {
        entry: entry.clone(),
        rgba: decoded.rgba,
        losses,
    })
}

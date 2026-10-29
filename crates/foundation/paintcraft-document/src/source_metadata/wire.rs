//! Canonical attachment framing; profile bytes follow this bounded fixed header.
use super::*;

pub(super) fn header(inner: &Inner) -> [u8; WIRE_HEADER_BYTES] {
    let mut out = [0; WIRE_HEADER_BYTES];
    let d = inner.descriptor;
    out[..8].copy_from_slice(b"PCSMETA1");
    out[8..10].copy_from_slice(&1u16.to_be_bytes());
    out[10] = d.format as u8;
    out[11] = u8::from(d.selection.is_some());
    out[12..20].copy_from_slice(&d.source_bytes.to_be_bytes());
    out[20..52].copy_from_slice(&d.source_sha256);
    out[52..56].copy_from_slice(&d.selection.unwrap_or(0).to_be_bytes());
    out[56..60].copy_from_slice(&d.source_dimensions.0.to_be_bytes());
    out[60..64].copy_from_slice(&d.source_dimensions.1.to_be_bytes());
    out[64..68].copy_from_slice(&d.output_dimensions.0.to_be_bytes());
    out[68..72].copy_from_slice(&d.output_dimensions.1.to_be_bytes());
    out[72] = d.orientation.unwrap_or(0);
    out[73] = d.orientation_policy as u8;
    if let Some(r) = d.resolution {
        out[74] = 1;
        out[75] = r.unit as u8;
        out[76..84].copy_from_slice(&r.horizontal.numerator.to_be_bytes());
        out[84..92].copy_from_slice(&r.horizontal.denominator.to_be_bytes());
        out[92..100].copy_from_slice(&r.vertical.numerator.to_be_bytes());
        out[100..108].copy_from_slice(&r.vertical.denominator.to_be_bytes());
        out[108] = u8::from(r.raw_unit.is_some());
        out[184..188].copy_from_slice(&r.raw_unit.unwrap_or(0).to_be_bytes());
    }
    out[109] = u8::from(inner.profile_header.is_some());
    out[110] = d.source_model as u8;
    out[112..120].copy_from_slice(&(inner.profile.len() as u64).to_be_bytes());
    out[120..152].copy_from_slice(&inner.profile_sha256);
    out[152..184].copy_from_slice(&inner.digest);
    out
}

pub(super) fn read(
    header: &[u8],
    profile: &[u8],
    limit: u64,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<SourceMetadata> {
    check()?;
    if header.len() != WIRE_HEADER_BYTES
        || &header[..8] != b"PCSMETA1"
        || header[8..10] != 1u16.to_be_bytes()
        || header[11] > 1
        || header[74] > 1
        || header[108] > 1
        || header[109] > 1
        || header[111] != 0
        || header[188..] != [0; 4]
    {
        return Err(invalid("Source metadata framing/version"));
    }
    let long = |at| u64::from_be_bytes(header[at..at + 8].try_into().unwrap());
    let word = |at| u32::from_be_bytes(header[at..at + 4].try_into().unwrap());
    if long(112) != profile.len() as u64
        || (header[109] == 0 && (!profile.is_empty() || header[120..152] != [0; 32]))
        || (header[11] == 0 && word(52) != 0)
        || (header[74] == 0 && (header[75..109] != [0; 34] || word(184) != 0))
        || (header[108] == 0 && word(184) != 0)
    {
        return Err(invalid("Source metadata profile/declaration extent"));
    }
    let format = match header[10] {
        1 => SourceFormat::Png,
        2 => SourceFormat::Jpeg,
        3 => SourceFormat::Gif,
        4 => SourceFormat::Bmp,
        5 => SourceFormat::Tiff,
        6 => SourceFormat::Webp,
        7 => SourceFormat::Jxl,
        8 => SourceFormat::Avif,
        9 => SourceFormat::Heif,
        10 => SourceFormat::Jxr,
        11 => SourceFormat::Other,
        _ => return Err(invalid("Source metadata format")),
    };
    let orientation_policy = match header[73] {
        0 => OrientationPolicy::Unmodified,
        1 => OrientationPolicy::ExifMaterializedV1,
        _ => return Err(invalid("Source orientation policy")),
    };
    let source_model = match header[110] {
        0 => SourceColorModel::Unknown,
        1 => SourceColorModel::Gray,
        2 => SourceColorModel::Rgb,
        3 => SourceColorModel::Cmyk,
        4 => SourceColorModel::YCbCr,
        5 => SourceColorModel::Ycck,
        6 => SourceColorModel::Indexed,
        _ => return Err(invalid("Original source sample model")),
    };
    let resolution = if header[74] == 1 {
        let unit = match header[75] {
            0 => ResolutionUnit::Unspecified,
            1 => ResolutionUnit::Inch,
            2 => ResolutionUnit::Centimetre,
            3 => ResolutionUnit::Metre,
            _ => return Err(invalid("Source resolution unit")),
        };
        Some(Resolution {
            horizontal: Rational {
                numerator: long(76) as i64,
                denominator: long(84),
            },
            vertical: Rational {
                numerator: long(92) as i64,
                denominator: long(100),
            },
            unit,
            raw_unit: (header[108] == 1).then(|| word(184)),
        })
    } else {
        None
    };
    let descriptor = SourceDescriptor {
        format,
        source_model,
        source_sha256: header[20..52].try_into().unwrap(),
        source_bytes: long(12),
        selection: (header[11] == 1).then(|| word(52)),
        source_dimensions: (word(56), word(60)),
        output_dimensions: (word(64), word(68)),
        orientation: (header[72] != 0).then_some(header[72]),
        orientation_policy,
        resolution,
    };
    let retained = SourceMetadata::capture(
        descriptor,
        (header[109] == 1).then_some(profile),
        limit,
        budget,
        check,
    )?;
    if retained.wire_header() != header {
        return Err(invalid("Source metadata digest/content differs"));
    }
    Ok(retained)
}

use super::*;
use paintcraft_document::source_metadata::{
    OrientationPolicy, Rational, Resolution, ResolutionUnit, SourceColorModel, SourceDescriptor,
    SourceFormat,
};

fn embedded_profile<'a>(bytes: &'a [u8], source: &bmp_metadata::Descriptor) -> Result<&'a [u8]> {
    if source.dib_bytes != 124 || source.color_space != Some(bmp_metadata::PROFILE_EMBEDDED) {
        return Err(codec("Inspected BMP ICC lacks an embedded V5 profile"));
    }
    let field = |at: usize| -> Result<u64> {
        let raw = bytes
            .get(at..at + 4)
            .ok_or_else(|| codec("BMP ICC header extent"))?;
        Ok(u64::from(u32::from_le_bytes(raw.try_into().unwrap())))
    };
    let start = field(126)?
        .checked_add(14)
        .ok_or_else(|| codec("BMP ICC offset"))?;
    let end = start
        .checked_add(field(130)?)
        .ok_or_else(|| codec("BMP ICC extent"))?;
    let start = usize::try_from(start).map_err(|_| codec("BMP ICC address extent"))?;
    let end = usize::try_from(end).map_err(|_| codec("BMP ICC address extent"))?;
    // Inspection has already rejected header/palette/pixel overlaps, linked
    // profiles and malformed ICC. This is a bounded view of that same source.
    bytes
        .get(start..end)
        .ok_or_else(|| codec("BMP ICC outside inspected source"))
}

pub(super) fn source_metadata(
    bytes: &[u8],
    source: &bmp_metadata::Descriptor,
    metadata: crate::pcnd::MetadataAdmission<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<SourceMetadata>> {
    if source.icc.is_none() && source.resolution_raw == [0, 0] {
        return Ok(None);
    }
    let resolution = (source.resolution_raw != [0, 0]).then(|| Resolution {
        horizontal: Rational {
            numerator: i64::from(source.resolution_raw[0]),
            denominator: 1,
        },
        vertical: Rational {
            numerator: i64::from(source.resolution_raw[1]),
            denominator: 1,
        },
        unit: ResolutionUnit::Metre,
        // BMP's unit is defined by the fields, not a separate stored unit tag.
        raw_unit: None,
    });
    let descriptor = SourceDescriptor {
        format: SourceFormat::Bmp,
        source_model: if source.bits_per_pixel <= 8 {
            SourceColorModel::Indexed
        } else {
            SourceColorModel::Rgb
        },
        source_sha256: source.source.sha256,
        source_bytes: source.source.bytes,
        selection: None,
        source_dimensions: (source.width, source.height),
        output_dimensions: (source.width, source.height),
        // BMP top-down/bottom-up rows are storage, not an EXIF orientation.
        orientation: None,
        orientation_policy: OrientationPolicy::Unmodified,
        resolution,
    };
    let retained = if let Some(expected) = &source.icc {
        let original = embedded_profile(bytes, source)?;
        if original.len() as u64 != expected.payload.bytes {
            return Err(codec("BMP ICC extent differs from inspected profile"));
        }
        let mut reader = Cursor::new(original);
        let retained = SourceMetadata::capture_profile_reader(
            descriptor,
            expected.payload.bytes,
            expected.payload.sha256,
            &mut reader,
            metadata.bytes,
            metadata.budget,
            check,
        )?;
        let actual = retained
            .profile_header()
            .ok_or_else(|| codec("Retained BMP ICC header missing"))?;
        if u64::from(actual.declared_bytes) != expected.payload.bytes
            || actual.data_color_space != expected.color_space
            || actual.connection_space != expected.connection_space
            || actual.tag_count != expected.tag_count
        {
            return Err(codec(
                "Retained BMP ICC interpretation differs from inspected source",
            ));
        }
        retained
    } else {
        SourceMetadata::capture(descriptor, None, metadata.bytes, metadata.budget, check)?
    };
    check()?;
    Ok(Some(retained))
}

use super::*;
use flate2::read::ZlibDecoder;
use paintcraft_document::source_metadata::{
    OrientationPolicy, Rational, Resolution, ResolutionUnit, SourceColorModel, SourceDescriptor,
    SourceFormat,
};

fn compressed_profile<'a>(bytes: &'a [u8], check: &dyn Fn() -> Result<()>) -> Result<&'a [u8]> {
    let mut at = 8usize;
    while at < bytes.len() {
        check()?;
        let header = bytes
            .get(
                at..at
                    .checked_add(8)
                    .ok_or_else(|| codec("PNG ICC chunk extent"))?,
            )
            .ok_or_else(|| codec("PNG ICC chunk header"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = at
            .checked_add(length)
            .and_then(|n| n.checked_add(12))
            .ok_or_else(|| codec("PNG ICC chunk extent"))?;
        let chunk = bytes
            .get(at..end)
            .ok_or_else(|| codec("PNG ICC chunk outside source"))?;
        if &header[4..8] == b"iCCP" {
            let data = &chunk[8..8 + length];
            let nul = data[..data.len().min(80)]
                .iter()
                .position(|v| *v == 0)
                .ok_or_else(|| codec("PNG ICC keyword terminator"))?;
            if data.get(nul + 1) != Some(&0) {
                return Err(codec("PNG ICC compression method"));
            }
            return data
                .get(nul + 2..)
                .ok_or_else(|| codec("PNG ICC compressed payload"));
        }
        at = end;
    }
    Err(codec("Inspected PNG ICC payload is absent"))
}
pub(super) fn source_metadata(
    bytes: &[u8],
    source: &png_metadata::Descriptor,
    metadata: crate::pcnd::MetadataAdmission<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<SourceMetadata>> {
    if source.icc.is_none() && source.resolution.is_none() {
        return Ok(None);
    }
    let resolution = source.resolution.map(|r| {
        let known = r.unit == super::super::raster_source_metadata::ResolutionUnit::Meter;
        Resolution {
            horizontal: Rational {
                numerator: i64::from(r.x),
                denominator: 1,
            },
            vertical: Rational {
                numerator: i64::from(r.y),
                denominator: 1,
            },
            unit: if known {
                ResolutionUnit::Metre
            } else {
                ResolutionUnit::Unspecified
            },
            raw_unit: Some(u32::from(known)),
        }
    });
    let descriptor = SourceDescriptor {
        format: SourceFormat::Png,
        source_model: match source.color_type {
            0 | 4 => SourceColorModel::Gray,
            2 | 6 => SourceColorModel::Rgb,
            3 => SourceColorModel::Indexed,
            _ => return Err(codec("PNG original sample model")),
        },
        source_sha256: source.source.sha256,
        source_bytes: source.source.bytes,
        selection: None,
        source_dimensions: (source.width, source.height),
        output_dimensions: (source.width, source.height),
        orientation: None,
        orientation_policy: OrientationPolicy::Unmodified,
        resolution,
    };
    let retained = if let Some(expected) = &source.icc {
        let compressed = compressed_profile(bytes, check)?;
        let mut reader = ZlibDecoder::new(compressed);
        let retained = SourceMetadata::capture_profile_reader(
            descriptor,
            expected.payload.bytes,
            expected.payload.sha256,
            &mut reader,
            metadata.bytes,
            metadata.budget,
            check,
        )?;
        if reader.total_in() != compressed.len() as u64 {
            return Err(codec("PNG ICC trailing compressed data"));
        }
        let actual = retained
            .profile_header()
            .ok_or_else(|| codec("Retained PNG ICC header missing"))?;
        if u64::from(actual.declared_bytes) != expected.payload.bytes
            || actual.data_color_space != expected.color_space
            || actual.connection_space != expected.connection_space
            || actual.tag_count != expected.tag_count
        {
            return Err(codec(
                "Retained PNG ICC interpretation differs from source inspection",
            ));
        }
        retained
    } else {
        SourceMetadata::capture(descriptor, None, metadata.bytes, metadata.budget, check)?
    };
    check()?;
    Ok(Some(retained))
}

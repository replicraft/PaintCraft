//! Retentive BMP is an additional bounded route, never a legacy decode fallback.
use super::{invariant, samples};
use crate::PIXELS_MAX;
use paintcraft_core::types::{Result, resources::SharedBudget};
use paintcraft_formats::{image, pcnd};
use sha2::{Digest, Sha256};
const METADATA_BYTES: u64 = 16 << 10;

pub(super) fn exercise(
    bytes: &[u8],
    legacy: &image::Decoded,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    with_budget(bytes, legacy, &SharedBudget::new(METADATA_BYTES), check)
}

fn with_budget(
    bytes: &[u8],
    legacy: &image::Decoded,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    // The private candidate drops on every path before the final charge check,
    // including invariant failures, resource denials and callback cancellation.
    let outcome = (|| {
        let prepared = image::bmp_retention::decode(
            bytes,
            PIXELS_MAX,
            pcnd::MetadataAdmission {
                bytes: METADATA_BYTES,
                budget,
            },
            check,
        )?;
        samples(
            prepared.image.width,
            prepared.image.height,
            &prepared.image.rgba,
        )?;
        let source = image::bmp_metadata::inspect(bytes, PIXELS_MAX, check)?;
        let notices =
            image::bmp_retention::required_acknowledgements(&source, legacy.losses.clone());
        if prepared.image.format != image::ImageFormat::Bmp
            || prepared.image.rgba != legacy.rgba
            || (prepared.image.width, prepared.image.height) != (legacy.width, legacy.height)
            || prepared.image.losses != notices
            || notices
                .iter()
                .any(|loss| !image::KNOWN_IMPORT_LOSSES.contains(&loss.as_str()))
        {
            return Err(invariant("retained BMP samples/geometry/notice contract"));
        }
        let density = source.resolution_raw != [0, 0];
        if prepared.attachment.is_some() != (source.icc.is_some() || density) {
            return Err(invariant(
                "retained BMP invented or omitted source attachment",
            ));
        }
        if let Some(metadata) = &prepared.attachment {
            let descriptor = metadata.descriptor();
            // These stable PCND source-header tags avoid a redundant direct
            // document dependency: BMP=4, RGB=2, indexed=6, unmodified=0.
            let wire = metadata.wire_header();
            if wire[10] != 4
                || wire[110] != if source.bits_per_pixel <= 8 { 6 } else { 2 }
                || wire[73] != 0
                || descriptor.source_sha256 != <[u8; 32]>::from(Sha256::digest(bytes))
                || descriptor.source_bytes != bytes.len() as u64
                || descriptor.source_dimensions != (legacy.width, legacy.height)
                || descriptor.output_dimensions != (legacy.width, legacy.height)
                || descriptor.selection.is_some()
                || descriptor.orientation.is_some()
                || metadata.storage_usage().1 != budget.usage().reserved
            {
                return Err(invariant(
                    "retained BMP raw facts/source identity/live charge",
                ));
            }
            if descriptor.resolution.is_some() != density {
                return Err(invariant("retained BMP invented or omitted density"));
            }
            if let Some(resolution) = descriptor.resolution
                && (resolution.horizontal.numerator != i64::from(source.resolution_raw[0])
                    || resolution.vertical.numerator != i64::from(source.resolution_raw[1])
                    || resolution.horizontal.denominator != 1
                    || resolution.vertical.denominator != 1
                    || wire[75] != 3 // canonical Metre tag, not active print settings
                    || resolution.raw_unit.is_some()
                    || resolution.physical().is_some() != source.resolution_raw.iter().all(|v| *v > 0))
            {
                return Err(invariant(
                    "retained BMP signed density/physical interpretation",
                ));
            }
            match (&source.icc, metadata.profile()) {
                (None, None) => {}
                (Some(expected), Some(actual)) => {
                    let start =
                        14u64 + u64::from(u32::from_le_bytes(bytes[126..130].try_into().unwrap()));
                    let length = u64::from(u32::from_le_bytes(bytes[130..134].try_into().unwrap()));
                    let end = start
                        .checked_add(length)
                        .ok_or_else(|| invariant("BMP profile extent"))?;
                    let original = bytes
                        .get(start as usize..end as usize)
                        .ok_or_else(|| invariant("BMP profile source view"))?;
                    if actual != original
                        || actual.len() as u64 != expected.payload.bytes
                        || <[u8; 32]>::from(Sha256::digest(actual)) != expected.payload.sha256
                    {
                        return Err(invariant("retained BMP changed original ICC bytes"));
                    }
                }
                _ => return Err(invariant("retained BMP ICC presence")),
            }
        } else if budget.usage().reserved != 0 {
            return Err(invariant("metadata-free BMP retained a charge"));
        }
        Ok(())
    })();
    if budget.usage().reserved != 0 {
        return Err(invariant(
            "retained BMP leaked a success/refusal/cancel charge",
        ));
    }
    outcome
}

#[cfg(test)]
mod tests;

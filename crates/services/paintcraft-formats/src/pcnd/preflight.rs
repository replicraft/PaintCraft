//! Save viability without normal-case raster copying/compression/body hashing.
use super::*;

fn integer_bytes(mut value: u64) -> u64 {
    let mut count = 1;
    while value >= 128 {
        count += 1;
        value >>= 7;
    }
    count
}
fn signed_bytes(value: i64) -> Option<u64> {
    (value.unsigned_abs() <= COORD_ABS_MAX as u64)
        .then(|| integer_bytes(((value << 1) ^ (value >> 63)) as u64))
}

/// Includes every resident tile's possible raw record, even uniform/default
/// tiles the real writer can omit. Compression cannot make stored lengths grow.
/// An unproven document shape falls back to the authoritative raw writer.
fn upper_bound(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Option<u64>> {
    check()?;
    if [doc.canvas.0, doc.canvas.1]
        .iter()
        .any(|v| !(1..=1_048_576).contains(v))
        || doc.roots.len() > LAYER_COUNT_MAX
        || doc.roots.len() != doc.layers.len()
        || doc
            .title
            .as_ref()
            .is_some_and(|s| s.len() > 16384 || s.chars().count() > 4096)
    {
        return Ok(None);
    }
    let mut bytes = 8
        + 32
        + integer_bytes(u64::from(doc.canvas.0))
        + integer_bytes(u64::from(doc.canvas.1))
        + 1;
    if let Some(title) = &doc.title {
        bytes += integer_bytes(title.len() as u64) + title.len() as u64;
    }
    if let Some(source) = &doc.source_metadata {
        bytes += paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64
            + source.profile().map_or(0, |p| p.len() as u64);
    }
    bytes += integer_bytes(doc.roots.len() as u64);
    for (index, id) in doc.roots.iter().enumerate() {
        check()?;
        let Some(layer) = doc.layers.get(id) else {
            return Ok(None);
        };
        if layer.id != *id
            || doc.roots[..index].contains(id)
            || layer.name.len() > 16384
            || layer.name.chars().count() > 1024
            || [layer.raster.width(), layer.raster.height()]
                .iter()
                .any(|v| !(1..=1_048_576).contains(v))
        {
            return Ok(None);
        }
        let (Some(x), Some(y)) = (signed_bytes(layer.origin.0), signed_bytes(layer.origin.1))
        else {
            return Ok(None);
        };
        bytes += 16
            + integer_bytes(layer.name.len() as u64)
            + layer.name.len() as u64
            + 2
            + x
            + y
            + integer_bytes(u64::from(layer.raster.width()))
            + integer_bytes(u64::from(layer.raster.height()))
            + 4
            + 1;
        for (coord, _) in layer.raster.tile_leases() {
            check()?;
            let (Ok(bx), Ok(by)) = (u32::try_from(coord.tx), u32::try_from(coord.ty)) else {
                return Ok(None);
            };
            let (Some(x), Some(y)) = (bx.checked_mul(TILE_EDGE), by.checked_mul(TILE_EDGE)) else {
                return Ok(None);
            };
            if x >= layer.raster.width() || y >= layer.raster.height() {
                return Ok(None);
            }
            let w = (layer.raster.width() - x).min(TILE_EDGE);
            let h = (layer.raster.height() - y).min(TILE_EDGE);
            bytes += 1
                + integer_bytes(u64::from(bx))
                + integer_bytes(u64::from(by))
                + integer_bytes(u64::from(w) * u64::from(h) * 4);
        }
    }
    Ok(Some(bytes))
}

/// Current-snapshot raw metadata must fit before initial publication. Normal
/// cases inspect only fields/tile coordinates. Near the limit, the actual raw
/// writer counts canonical metadata with payload output and SHA hashing disabled.
/// This does not guarantee arbitrary future layer/title growth remains savable.
pub(super) fn check(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    if upper_bound(doc, check)?.is_some_and(|bytes| bytes <= DOCUMENT_MAX) {
        return check();
    }
    binary::preflight_metadata(doc, check)
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_document::source_metadata::{BYTES_MAX, SourceMetadata};

    #[test]
    fn exact_metadata_boundary_matches_real_writer_without_arbitrary_headroom() {
        let bytes = include_bytes!("../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd");
        let mut document = read(bytes).unwrap();
        let original = document.source_metadata.take().unwrap();
        let descriptor = original.descriptor().clone();
        let raw = write(&document).unwrap();
        // This independent specimen has no raster records: all body bytes are metadata.
        assert!(document.layers.is_empty());
        let framing = raw.len() - 32 + paintcraft_document::source_metadata::WIRE_HEADER_BYTES;
        let exact_profile = DOCUMENT_MAX as usize - framing;
        for extra in [0, 1] {
            let mut profile = original.profile().unwrap().to_vec();
            profile.resize(exact_profile + extra, 0);
            let length = profile.len() as u32;
            profile[..4].copy_from_slice(&length.to_be_bytes());
            let realm = SharedBudget::new(BYTES_MAX + 4096);
            let metadata = SourceMetadata::capture(
                descriptor.clone(),
                Some(&profile),
                BYTES_MAX,
                &realm,
                &|| Ok(()),
            )
            .unwrap();
            let candidate = document
                .with_source_metadata_checked(Some(metadata), &|| Ok(()))
                .unwrap();
            let preflight = check_metadata_admission(&candidate, &|| Ok(()));
            let actual = write(&candidate);
            assert_eq!(preflight.is_ok(), extra == 0);
            assert_eq!(actual.is_ok(), extra == 0);
            if let Err(error) = preflight {
                assert_eq!(error.code, ErrorCode::ResourceLimit);
            }
            drop(candidate);
            assert_eq!(realm.usage().reserved, 0);
        }
    }

    #[test]
    fn fast_bound_and_authoritative_counter_agree_and_cancel() {
        let document =
            paintcraft_document::import_image(2, 1, &[3, 4, 5, 6, 7, 8, 9, 10], "preflight")
                .unwrap();
        assert!(upper_bound(&document, &|| Ok(())).unwrap().unwrap() < DOCUMENT_MAX);
        check_metadata_admission(&document, &|| Ok(())).unwrap();
        binary::preflight_metadata(&document, &|| Ok(())).unwrap();
        let error = check_metadata_admission(&document, &|| {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        let mut invalid = document;
        invalid.roots.push(invalid.roots[0]);
        assert!(check_metadata_admission(&invalid, &|| Ok(())).is_err());
    }
}

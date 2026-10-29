use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::{
    Layer,
    source_metadata::{BYTES_MAX, SourceMetadata},
};
use std::cell::Cell;

fn specimen(profile_bytes: usize, color: Option<Rgba>) -> (Document, SharedBudget) {
    let mut doc = read(include_bytes!(
        "../../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ))
    .unwrap();
    let original = doc.source_metadata.take().unwrap();
    let descriptor = *original.descriptor();
    let mut profile = original.profile().unwrap().to_vec();
    profile.resize(profile_bytes, 0);
    profile[..4].copy_from_slice(&(profile_bytes as u32).to_be_bytes());
    let budget = SharedBudget::new(BYTES_MAX + 4096);
    let source =
        SourceMetadata::capture(descriptor, Some(&profile), BYTES_MAX, &budget, &|| Ok(()))
            .unwrap();
    doc.title = None;
    doc.canvas = (128, 128);
    doc.roots.clear();
    doc.layers.clear();
    let id = Uuid::from_u128(2);
    let mut raster = RasterLayer::new(128, 128, Rgba::TRANSPARENT);
    if let Some(color) = color {
        raster
            .set_block(0, 0, &color.0.repeat(128 * 128), &mut Admission::new(65536))
            .unwrap();
    }
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "x".into(),
            origin: (0, 0),
            visible: true,
            opacity: 255,
            raster,
        },
    );
    (
        doc.with_source_metadata_checked(Some(source), &|| Ok(()))
            .unwrap(),
        budget,
    )
}

#[test]
fn selected_compact_boundary_accepts_two_bytes_rejected_by_raw_and_refuses_one_more() {
    // Independently qualified Python/real CLI witness: header46+source192+
    // layer31+compact block4 =273. Raw block length65536 needs2 extraLEB bytes.
    let (doc, budget) = specimen(DOCUMENT_MAX as usize - 273, Some(Rgba([255, 0, 0, 255])));
    let initial_charge = budget.usage().reserved;
    assert_eq!(
        check_metadata_admission(&doc, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(write(&doc).unwrap_err().code, ErrorCode::ResourceLimit);
    check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
    let compact = write_compact(&doc).unwrap();
    assert_eq!(compact.len(), DOCUMENT_MAX as usize + 6 + 32);
    assert!(read(&compact).unwrap().same_semantics(&doc));
    let mut changed = doc.clone();
    changed.layers.values_mut().next().unwrap().name = "y".into();
    check_compact_metadata_admission(&changed, &|| Ok(())).unwrap();
    assert_eq!(write_compact(&changed).unwrap().len(), compact.len());
    changed.layers.values_mut().next().unwrap().name = "xy".into();
    assert_eq!(
        check_compact_metadata_admission(&changed, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        write_compact(&changed).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, initial_charge);
    drop(changed);
    drop(doc);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn resident_default_tile_is_elided_at_the_exact_boundary() {
    // No record: header46+source192+layer31 =269, despite one resident tile.
    let (doc, _budget) = specimen(DOCUMENT_MAX as usize - 269, Some(Rgba::TRANSPARENT));
    assert_eq!(
        doc.layers.values().next().unwrap().raster.resident_tiles(),
        1
    );
    assert!(upper_bound(&doc, &|| Ok(())).unwrap().unwrap() > DOCUMENT_MAX);
    check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
    assert_eq!(
        write_compact(&doc).unwrap().len(),
        DOCUMENT_MAX as usize + 32
    );
}

#[test]
fn selected_compact_rare_path_propagates_every_one_shot_and_sticky_cancellation() {
    let (doc, budget) = specimen(DOCUMENT_MAX as usize - 273, Some(Rgba([255, 0, 0, 255])));
    let charge = budget.usage().reserved;
    let count = Cell::new(0);
    check_compact_metadata_admission(&doc, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for sticky in [false, true] {
        for stop in 1..=count.get() {
            let visits = Cell::new(0);
            let err = check_compact_metadata_admission(&doc, &|| {
                visits.set(visits.get() + 1);
                if visits.get() == stop || sticky && visits.get() > stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "compact preflight sentinel",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(err.code, ErrorCode::OperationCancelled);
            assert_eq!(budget.usage().reserved, charge);
        }
    }
}

#[test]
fn ordinary_safe_bound_remains_a_fast_acceptance_and_cancellable() {
    let doc =
        paintcraft_document::import_image(2, 1, &[3, 4, 5, 0, 7, 8, 9, 255], "compact ordinary")
            .unwrap();
    assert!(upper_bound(&doc, &|| Ok(())).unwrap().unwrap() < DOCUMENT_MAX);
    check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
    binary::preflight_compact_metadata(&doc, &|| Ok(())).unwrap();
    let err = check_compact_metadata_admission(&doc, &|| {
        Err(Error::new(ErrorCode::OperationCancelled, "ordinary cancel"))
    })
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::OperationCancelled);
}

#[test]
fn retained_bmp_initial_adoption_uses_the_selected_compact_boundary() {
    use sha2::{Digest, Sha256};
    // Actual imported layer name "Image" adds four bytes to the "x" witness.
    // Metadata277 + profile fits exactly; compact red runs6 use one length byte.
    let (template, template_budget) = specimen(DOCUMENT_MAX as usize - 277, None);
    let profile = template
        .source_metadata
        .as_ref()
        .unwrap()
        .profile()
        .unwrap()
        .to_vec();
    drop(template);
    assert_eq!(template_budget.usage().reserved, 0);
    let pixels = 128 * 128;
    let pixel_bytes = pixels * 3;
    let profile_start = 14 + 124 + pixel_bytes;
    let mut bmp = vec![0; profile_start + profile.len()];
    bmp[..2].copy_from_slice(b"BM");
    let total = bmp.len() as u32;
    for (offset, value) in [
        (2, total),
        (10, 138),
        (14, 124),
        (18, 128),
        (22, 128),
        (34, pixel_bytes as u32),
        (70, 0x4d424544),
        (122, 4),
        (126, (124 + pixel_bytes) as u32),
        (130, profile.len() as u32),
    ] {
        bmp[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
    for pixel in bmp[138..profile_start].as_chunks_mut::<3>().0 {
        pixel.copy_from_slice(&[0, 0, 255]);
    }
    bmp[profile_start..].copy_from_slice(&profile);
    let source_digest: [u8; 32] = Sha256::digest(&bmp).into();
    let realm = SharedBudget::new(BYTES_MAX + 4096);
    let prepared = crate::image::bmp_retention::decode(
        &bmp,
        crate::image::DECODE_PIXELS_MAX,
        MetadataAdmission {
            bytes: BYTES_MAX,
            budget: &realm,
        },
        &|| Ok(()),
    )
    .unwrap();
    let expected = [255, 0, 0, 255].repeat(pixels);
    assert_eq!(prepared.image.rgba, expected);
    let source = prepared.attachment.as_ref().unwrap();
    assert_eq!(source.profile(), Some(profile.as_slice()));
    assert_eq!(source.descriptor().source_sha256, source_digest);
    assert_eq!(source.descriptor().source_bytes, bmp.len() as u64);
    assert_eq!(source.descriptor().source_dimensions, (128, 128));
    // This invokes SharedPrepared's real initial save viability admission.
    let doc = prepared
        .document_with_admission(&mut Admission::new(65536), &|| Ok(()))
        .unwrap();
    assert_eq!(
        check_metadata_admission(&doc, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let encoded = write_compact(&doc).unwrap();
    assert_eq!(encoded.len(), DOCUMENT_MAX as usize + 6 + 32);
    let read_realm = SharedBudget::new(BYTES_MAX + 4096);
    let reopened = read_with_metadata_admission_checked(
        &encoded,
        65536,
        65536,
        8,
        MetadataAdmission {
            bytes: BYTES_MAX,
            budget: &read_realm,
        },
        &|| Ok(()),
    )
    .unwrap();
    assert!(reopened.same_semantics(&doc));
    assert_eq!(
        reopened.source_metadata.as_ref().unwrap().profile(),
        Some(profile.as_slice())
    );
    assert_eq!(
        reopened
            .source_metadata
            .as_ref()
            .unwrap()
            .descriptor()
            .source_sha256,
        source_digest
    );
    let raster = &reopened.layers.values().next().unwrap().raster;
    for y in 0..128 {
        for x in 0..128 {
            assert_eq!(raster.get(x, y), Rgba([255, 0, 0, 255]));
        }
    }
    drop(reopened);
    assert_eq!(read_realm.usage().reserved, 0);
    drop(doc);
    drop(prepared);
    assert_eq!(realm.usage().reserved, 0);
}

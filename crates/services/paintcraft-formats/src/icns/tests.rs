use super::*;
use std::cell::Cell;
const MODERN: &[u8] = include_bytes!("../../tests/icns-fixtures/modern.icns");
const DUPLICATES: &[u8] = include_bytes!("../../tests/icns-fixtures/duplicates.icns");
const METADATA: &[u8] = include_bytes!("../../tests/icns-fixtures/metadata.icns");
fn pixels(side: u32, seed: u8) -> Vec<u8> {
    (0..side)
        .flat_map(|y| {
            (0..side).flat_map(move |x| {
                [
                    ((u32::from(seed) * 31 + (x % 4) * 17) % 256) as u8,
                    ((u32::from(seed) * 13 + (y % 4) * 29) % 256) as u8,
                    ((u32::from(seed) * 19 + (x + y) % 5) % 256) as u8,
                    [0, 64, 128, 255][((x + y) % 4) as usize],
                ]
            })
        })
        .collect()
}
fn container(records: &[([u8; 4], &[u8])]) -> Vec<u8> {
    let mut bytes = b"icns\0\0\0\0".to_vec();
    for (tag, payload) in records {
        bytes.extend(tag);
        bytes.extend(((payload.len() + 8) as u32).to_be_bytes());
        bytes.extend(*payload);
    }
    let length = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&length.to_be_bytes());
    bytes
}
fn payload<'a>(bytes: &'a [u8], directory: &Directory, index: usize) -> &'a [u8] {
    let record = &directory.records[usize::from(directory.entries[index].record)];
    &bytes[record.offset as usize + 8..(record.offset + record.length) as usize]
}
#[test]
fn owned_modern_entries_match_independent_pixel_formulas_and_preserve_every_logical_density() {
    let directory = inspect(MODERN, &|| Ok(())).unwrap();
    assert_eq!(directory.entries.len(), 11);
    assert_eq!(directory.records.len(), 12);
    assert_eq!(directory.records[0].tag, *b"TOC ");
    for (index, kind) in IconType::MODERN.into_iter().enumerate() {
        let entry = &directory.entries[index];
        assert_eq!(entry.tag, kind.tag());
        assert_eq!(
            (entry.logical_width, entry.logical_height, entry.scale),
            (kind.logical_size(), kind.logical_size(), kind.scale())
        );
        assert_eq!(
            (entry.width, entry.height),
            (kind.pixel_size(), kind.pixel_size())
        );
        let decoded = decode_entry(MODERN, index as u16, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(decoded.rgba, pixels(kind.pixel_size(), index as u8 + 1));
        assert!(decoded.losses.contains(&"icns-metadata-ignored".into()));
        assert!(
            decoded
                .losses
                .contains(&"icon-other-entries-not-imported".into())
        );
        assert_eq!(
            decoded
                .losses
                .contains(&"icon-density-not-retained-in-flat-document".into()),
            kind.scale() == 2
        );
        assert_eq!(
            decoded.document(&"0".repeat(64)).unwrap().canvas,
            (kind.pixel_size(), kind.pixel_size())
        );
    }
    assert_eq!(directory.entries[1].width, directory.entries[6].width);
    assert_ne!(
        directory.entries[1].logical_width,
        directory.entries[6].logical_width
    );
}
#[test]
fn duplicate_types_remain_ordered_and_legacy_or_jp2_records_never_fall_back_to_another_icon() {
    let directory = inspect(DUPLICATES, &|| Ok(())).unwrap();
    assert_eq!(directory.entries.len(), 2);
    assert_eq!(directory.entries[0].tag, directory.entries[1].tag);
    for (index, seed) in [(0, 2), (1, 8)] {
        let decoded = decode_entry(DUPLICATES, index, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(decoded.rgba, pixels(32, seed));
    }
    for bytes in [
        include_bytes!("../../tests/icns-fixtures/mixed-legacy.icns").as_slice(),
        include_bytes!("../../tests/icns-fixtures/mixed-jp2.icns").as_slice(),
    ] {
        let directory = inspect(bytes, &|| Ok(())).unwrap();
        assert_eq!(directory.entries[0].encoding, Encoding::Png);
        assert_eq!(directory.entries[1].encoding, Encoding::Unqualified);
        assert_eq!(
            decode_entry(bytes, 1, PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            decode_entry(bytes, 0, PIXELS_MAX, &|| Ok(())).unwrap().rgba,
            pixels(16, 3)
        );
    }
    assert_eq!(
        decode_entry(MODERN, u16::MAX, PIXELS_MAX, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        decode_entry(MODERN, 0, 255, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn inert_header_metadata_and_bounded_png_exif_srgb_are_retained_without_weakening_ico_profile() {
    let directory = inspect(METADATA, &|| Ok(())).unwrap();
    assert_eq!(directory.records.last().unwrap().tag, *b"info");
    let decoded = decode_entry(METADATA, 0, PIXELS_MAX, &|| Ok(())).unwrap();
    assert_eq!(decoded.rgba, pixels(16, 3));
    assert!(decoded.losses.contains(&"exif-metadata-ignored".into()));
    let png = payload(METADATA, &directory, 0);
    let mut ico = vec![0, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0];
    ico.extend((png.len() as u32).to_le_bytes());
    ico.extend(22u32.to_le_bytes());
    ico.extend(png);
    assert_eq!(
        crate::icons::inspect(&ico).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn selected_library_fresh_writer_keeps_explicit_types_rgba_and_hidden_rgb_for_independent_readback()
{
    let images: Vec<_> = IconType::MODERN
        .into_iter()
        .enumerate()
        .map(|(index, kind)| pixels(kind.pixel_size(), index as u8 + 1))
        .collect();
    let entries: Vec<_> = IconType::MODERN
        .into_iter()
        .zip(&images)
        .filter(|(kind, _)| kind.writable())
        .map(|(kind, rgba)| WriteEntry { kind, rgba })
        .collect();
    let encoded = encode_entries(&entries, &|| Ok(())).unwrap();
    let directory = inspect(&encoded, &|| Ok(())).unwrap();
    assert_eq!(directory.entries.len(), 8);
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(
            &decode_entry(&encoded, index as u16, PIXELS_MAX, &|| Ok(()))
                .unwrap()
                .rgba,
            entry.rgba
        );
    }
    if let Some(path) = std::env::var_os("PAINTCRAFT_ICNS_ORACLE_DIR") {
        let path = std::path::Path::new(&path);
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(path.join("modern.icns"), &encoded).unwrap();
    }
    let rgba = pixels(256, 4);
    let entries = [
        WriteEntry {
            kind: IconType::Size256,
            rgba: &rgba,
        },
        WriteEntry {
            kind: IconType::Retina128,
            rgba: &rgba,
        },
    ];
    let encoded = encode_entries(&entries, &|| Ok(())).unwrap();
    let a = decode_entry(&encoded, 0, PIXELS_MAX, &|| Ok(())).unwrap();
    let b = decode_entry(&encoded, 1, PIXELS_MAX, &|| Ok(())).unwrap();
    assert_eq!(a.rgba, b.rgba);
    assert_ne!(a.entry.scale, b.entry.scale);
    assert_ne!(
        a.document(&"a".repeat(64)).unwrap().document_id,
        b.document(&"a".repeat(64)).unwrap().document_id
    );
    assert!(encode_entries(&[entries[0], entries[0]], &|| Ok(())).is_err());
    assert!(encode_entries(&[], &|| Ok(())).is_err());
    for kind in [IconType::Size16, IconType::Size32, IconType::Size64] {
        let rgba = pixels(kind.pixel_size(), 1);
        assert_eq!(
            encode_entries(&[WriteEntry { kind, rgba: &rgba }], &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert!(
        encode_entries(
            &[WriteEntry {
                kind: IconType::Retina16,
                rgba: &[0; 4]
            }],
            &|| Ok(())
        )
        .is_err()
    );
}
#[test]
fn complete_source_extents_toc_png_crcs_and_pixel_allowances_fail_before_upstream_work() {
    let directory = inspect(METADATA, &|| Ok(())).unwrap();
    let png = payload(METADATA, &directory, 0);
    for end in 0..METADATA.len() {
        assert!(inspect(&METADATA[..end], &|| Ok(())).is_err());
    }
    let mut trailing = METADATA.to_vec();
    trailing.push(0);
    assert!(inspect(&trailing, &|| Ok(())).is_err());
    for length in [0u32, 1, 7, u32::MAX] {
        let mut bytes = container(&[(*b"icp4", png)]);
        bytes[12..16].copy_from_slice(&length.to_be_bytes());
        assert!(inspect(&bytes, &|| Ok(())).is_err());
    }
    let mut bytes = METADATA.to_vec();
    bytes[20] ^= 1;
    assert!(inspect(&bytes, &|| Ok(())).is_err()); // TOC first length
    let mut bytes = png.to_vec();
    bytes[29] ^= 1;
    assert!(inspect(&container(&[(*b"icp4", &bytes)]), &|| Ok(())).is_err());
    assert!(inspect(&container(&[(*b"icp5", png)]), &|| Ok(())).is_err()); // type/header dimension mismatch
    let fake = b"not a PNG";
    let records = vec![(*b"ic10", fake.as_slice()); 5];
    assert_eq!(
        inspect(&container(&records), &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut records = vec![(*b"info", &[][..]); RECORDS_MAX];
    records.push((*b"icp4", png));
    assert_eq!(
        inspect(&container(&records), &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let opaque = vec![0; OPAQUE_BYTES_MAX + 1];
    assert_eq!(
        inspect(&container(&[(*b"info", &opaque), (*b"icp4", png)]), &|| Ok(
            ()
        ))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let image = vec![0; ENTRY_BYTES_MAX + 1];
    assert_eq!(
        inspect(&container(&[(*b"icp4", &image)]), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut bytes = METADATA.to_vec();
    let size = bytes.len() as u32;
    bytes.extend(b"TOC \0\0\0\x08");
    bytes[4..8].copy_from_slice(&(size + 8).to_be_bytes());
    assert!(inspect(&bytes, &|| Ok(())).is_err());
}
fn cancellation<T: std::fmt::Debug>(run: impl Fn(&dyn Fn() -> Result<()>) -> Result<T>) {
    let calls = Cell::new(0);
    drop(
        run(&|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let result = run(&|| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
}
#[test]
fn every_exposed_stage_cancels_and_never_changes_source_or_returns_a_partial_icon_set() {
    let source = METADATA.to_vec();
    cancellation(|check| inspect(&source, check));
    cancellation(|check| decode_entry(&source, 0, PIXELS_MAX, check));
    let rgba = pixels(32, 3);
    cancellation(|check| {
        encode_entries(
            &[WriteEntry {
                kind: IconType::Retina16,
                rgba: &rgba,
            }],
            check,
        )
    });
    assert_eq!(source, METADATA);
}
#[test]
fn deterministic_bit_mutations_do_not_panic_or_cross_unchecked_ranges() {
    for original in [METADATA, DUPLICATES] {
        for index in (0..original.len() * 8).step_by(3) {
            let mut bytes = original.to_vec();
            bytes[index / 8] ^= 1 << (index % 8);
            assert!(
                std::panic::catch_unwind(|| decode_entry(&bytes, 0, PIXELS_MAX, &|| Ok(())))
                    .is_ok()
            );
        }
    }
}

#[test]
fn legacy_raw_rle_and_exact_masks_preserve_independent_pixels_and_source() {
    for (tag, mask, side) in [(*b"is32", *b"s8mk", 16), (*b"il32", *b"l8mk", 32)] {
        let rgba = pixels(side, 9);
        let raw: Vec<_> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| pixel[..3].iter().copied())
            .collect();
        let alpha: Vec<_> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[3])
            .collect();
        // Independent literal-only channel packet encoding, not selected writer.
        let mut rle = Vec::new();
        for channel in 0..3 {
            let values: Vec<_> = rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| pixel[channel])
                .collect();
            for chunk in values.chunks(128) {
                rle.push((chunk.len() - 1) as u8);
                rle.extend(chunk);
            }
        }
        for (payload, encoding) in [(&raw, Encoding::LegacyRaw), (&rle, Encoding::LegacyRle)] {
            let bytes = container(&[(mask, &alpha), (tag, payload)]);
            let original = bytes.clone();
            let directory = inspect(&bytes, &|| Ok(())).unwrap();
            assert_eq!(directory.entries[0].encoding, encoding);
            assert_eq!(directory.entries[0].scale, 1);
            assert_eq!(
                decode_entry(&bytes, 0, PIXELS_MAX, &|| Ok(()))
                    .unwrap()
                    .rgba,
                rgba
            );
            assert_eq!(bytes, original);
        }
        let kind = if side == 16 {
            IconType::Legacy16
        } else {
            IconType::Legacy32
        };
        let encoded =
            write::encode_legacy_candidate(&[WriteEntry { kind, rgba: &rgba }], &|| Ok(()))
                .unwrap();
        assert_eq!(
            decode_entry(&encoded, 0, PIXELS_MAX, &|| Ok(()))
                .unwrap()
                .rgba,
            rgba
        );
        if let Some(path) = std::env::var_os("PAINTCRAFT_ICNS_ORACLE_DIR") {
            std::fs::write(
                std::path::Path::new(&path).join(format!("legacy-{side}.icns")),
                encoded,
            )
            .unwrap();
            std::fs::write(
                std::path::Path::new(&path).join(format!("raw-{side}.icns")),
                container(&[(tag, &raw), (mask, &alpha)]),
            )
            .unwrap();
        }
    }
}
#[test]
fn legacy_packets_masks_ambiguity_limits_and_cancel_are_fail_closed() {
    let alpha = [255; 256];
    // Every channel: two literal zeros followed by runs 130 and 124. Initial
    // four zero bytes are legal packets; never treat them as a guessed header.
    let mut rle = vec![0, 0, 0, 0, 255, 7, 249, 8];
    for _ in 0..2 {
        rle.extend([255, 9, 251, 10]);
    }
    let good = container(&[(*b"is32", &rle), (*b"s8mk", &alpha)]);
    let decoded = decode_entry(&good, 0, 256, &|| Ok(())).unwrap();
    assert_eq!(&decoded.rgba[..8], &[0, 9, 9, 255, 0, 9, 9, 255]);
    assert_eq!(decoded.rgba[8], 7);
    for badstream in [
        &rle[..rle.len() - 1],
        &[rle.as_slice(), &[0]].concat(),
        &[255, 0, 255, 0][..],
    ] {
        assert!(
            inspect(
                &container(&[(*b"is32", badstream), (*b"s8mk", &alpha)]),
                &|| Ok(())
            )
            .is_err()
        );
    }
    assert!(
        inspect(
            &container(&[(*b"is32", &rle), (*b"s8mk", &alpha[..255])]),
            &|| Ok(())
        )
        .is_err()
    );
    for records in [
        vec![(*b"is32", rle.as_slice())],
        vec![
            (*b"is32", rle.as_slice()),
            (*b"s8mk", alpha.as_slice()),
            (*b"s8mk", alpha.as_slice()),
        ],
        vec![
            (*b"is32", rle.as_slice()),
            (*b"is32", rle.as_slice()),
            (*b"s8mk", alpha.as_slice()),
        ],
    ] {
        let bytes = container(&records);
        assert_eq!(
            inspect(&bytes, &|| Ok(())).unwrap().entries[0].encoding,
            Encoding::Unqualified
        );
        assert!(decode_entry(&bytes, 0, 256, &|| Ok(())).is_err());
    }
    assert_eq!(
        decode_entry(&good, 0, 255, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let calls = Cell::new(0);
    decode_entry(&good, 0, 256, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        let result = decode_entry(&good, 0, 256, &|| {
            count.set(count.get() + 1);
            if count.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "stop"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
}

#[test]
fn independently_authored_legacy_fixtures_and_ambiguous_writer_length() {
    for (bytes, side, encoding) in [
        (
            include_bytes!("../../tests/icns-fixtures/legacy-raw-16.icns").as_slice(),
            16,
            Encoding::LegacyRaw,
        ),
        (
            include_bytes!("../../tests/icns-fixtures/legacy-rle-16.icns").as_slice(),
            16,
            Encoding::LegacyRle,
        ),
        (
            include_bytes!("../../tests/icns-fixtures/legacy-raw-32.icns").as_slice(),
            32,
            Encoding::LegacyRaw,
        ),
        (
            include_bytes!("../../tests/icns-fixtures/legacy-rle-32.icns").as_slice(),
            32,
            Encoding::LegacyRle,
        ),
    ] {
        let directory = inspect(bytes, &|| Ok(())).unwrap();
        assert_eq!(directory.entries[0].encoding, encoding);
        assert_eq!(
            decode_entry(bytes, 0, PIXELS_MAX, &|| Ok(())).unwrap().rgba,
            pixels(side, 9)
        );
    }
    assert_eq!(
        encode_entries(
            &[WriteEntry {
                kind: IconType::Legacy16,
                rgba: &pixels(16, 9)
            }],
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
    // A four-pixel repeat and 252 literals encode to exactly256bytes per
    // channel in the selected encoder: ambiguous with raw3*N. Emit literalRLE.
    let rgba: Vec<_> = (0..256)
        .flat_map(|index| {
            let value = if index < 4 {
                0
            } else {
                ((index - 4) % 251 + 1) as u8
            };
            [value, value, value, 255]
        })
        .collect();
    let bytes = write::encode_legacy_candidate(
        &[WriteEntry {
            kind: IconType::Legacy16,
            rgba: &rgba,
        }],
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        inspect(&bytes, &|| Ok(())).unwrap().entries[0].encoding,
        Encoding::LegacyRle
    );
    assert_eq!(decode_entry(&bytes, 0, 256, &|| Ok(())).unwrap().rgba, rgba);
    cancellation(|check| {
        write::encode_legacy_candidate(
            &[WriteEntry {
                kind: IconType::Legacy16,
                rgba: &rgba,
            }],
            check,
        )
    });
}

#[test]
fn primary_apple_argb_descriptors_do_not_inherit_selected_library_retina_error() {
    let bytes = container(&[(*b"ic04", b"ARGB"), (*b"ic05", b"ARGB")]);
    let directory = inspect(&bytes, &|| Ok(())).unwrap();
    for (entry, side) in directory.entries.iter().zip([16, 32]) {
        assert_eq!(
            (
                entry.width,
                entry.height,
                entry.logical_width,
                entry.logical_height,
                entry.scale
            ),
            (side, side, side, side, 1)
        );
        assert_eq!(entry.encoding, Encoding::Unqualified);
    }
    let modern = inspect(MODERN, &|| Ok(())).unwrap();
    let retina = modern
        .entries
        .iter()
        .find(|entry| entry.tag == *b"ic11")
        .unwrap();
    assert_eq!(
        (retina.width, retina.logical_width, retina.scale),
        (32, 16, 2)
    );
    for index in [0, 1] {
        assert_eq!(
            decode_entry(&bytes, index, PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}

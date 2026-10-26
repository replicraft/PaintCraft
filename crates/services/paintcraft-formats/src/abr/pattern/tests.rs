use super::*;
use std::cell::Cell;
const SOURCE: &[u8] = include_bytes!("../../../tests/abr-fixtures/patterns.abr");

fn source_payload() -> &'static [u8] {
    let mut offset = 4;
    loop {
        let n = u32::from_be_bytes(SOURCE[offset + 8..offset + 12].try_into().unwrap()) as usize;
        if &SOURCE[offset + 4..offset + 8] == b"patt" {
            return &SOURCE[offset + 12..offset + 12 + n];
        }
        offset += 12 + n.next_multiple_of(4);
    }
}
fn file(payload: &[u8]) -> Vec<u8> {
    let mut source = b"\0\x06\0\x01\x38BIMpatt".to_vec();
    source.extend((payload.len() as u32).to_be_bytes());
    source.extend(payload);
    source.resize(source.len().next_multiple_of(4), 0);
    source
}
fn empty_record(name: &str, width: u32, height: u32) -> Vec<u8> {
    let mut body = [1u32.to_be_bytes(), 3u32.to_be_bytes(), [0; 4]].concat();
    let name: Vec<_> = name.encode_utf16().collect();
    body.extend((name.len() as u32).to_be_bytes());
    for unit in name {
        body.extend(unit.to_be_bytes());
    }
    body.extend(b"\x01x");
    body.extend(3u32.to_be_bytes());
    body.extend(40u32.to_be_bytes());
    for value in [0, 0, height, width, 3] {
        body.extend(value.to_be_bytes());
    }
    body.extend([0; 20]);
    let mut record = (body.len() as u32).to_be_bytes().to_vec();
    record.extend(body);
    record.resize(record.len().next_multiple_of(4), 0);
    record
}
fn channel(pattern: &Pattern, index: usize) -> &Channel {
    let Slot::Channel(channel) = &pattern.slots[index] else {
        panic!("expected channel")
    };
    channel
}

#[test]
fn exact_color_channels_alpha_palette_and_converted_previews_are_separate() {
    let library = crate::abr::decode(SOURCE, &|| Ok(())).unwrap();
    let patterns = library.patterns();
    assert_eq!(patterns.len(), 5);
    assert_eq!(library.sections()[1].kind, SectionKind::Patterns);
    assert_eq!(library.sections()[1].payload, source_payload());
    assert_eq!(patterns[0].name, "Pattern 色 🚀");
    assert_eq!(patterns[0].point, [-3, 4]);
    assert_eq!(patterns[0].bounds, [5, 7, 7, 9]);
    assert_eq!(
        channel(&patterns[0], 1).samples,
        Samples::U8(vec![100, 110, 120, 130])
    );
    assert_eq!(
        patterns[0].converted_preview().rgba,
        [
            10, 100, 200, 0, 20, 110, 210, 64, 30, 120, 220, 128, 40, 130, 230, 255
        ]
    );
    assert!(!patterns[0].converted_preview().reduced_precision);
    assert_eq!(
        channel(&patterns[1], 0).samples,
        Samples::U16(vec![0, 255, 32769, 65535])
    );
    assert!(matches!(patterns[1].slots[1], Slot::EmptyWritten));
    assert_eq!(
        patterns[1].converted_preview().rgba,
        [
            0, 0, 0, 0, 1, 1, 1, 0, 128, 128, 128, 128, 255, 255, 255, 255
        ]
    );
    assert!(patterns[1].converted_preview().reduced_precision);
    assert_eq!(patterns[2].palette.as_ref().unwrap()[1], [200, 100, 50]);
    assert_eq!(patterns[2].palette_extra(), Some([0; 4]));
    assert_eq!(
        patterns[2].converted_preview().unavailable,
        Some(PreviewUnavailable::PaletteMetadata)
    );
    assert!(patterns[2].converted_preview().rgba.is_empty());
    assert_eq!(
        channel(&patterns[2], 0).samples(),
        &Samples::U8(vec![0, 1, 1, 0])
    );
    assert_eq!(
        channel(&patterns[2], 2).samples(),
        &Samples::U8(vec![255, 128, 64, 0])
    );
    assert_eq!(
        channel(&patterns[3], 0).samples,
        Samples::F32Bits(vec![0x80000000, 0x3f000000, 0x3fc00000, 0xbe800000])
    );
    assert_eq!(
        patterns[3].converted_preview().rgba,
        [
            0, 64, 255, 255, 128, 191, 0, 128, 255, 255, 32, 64, 0, 0, 223, 0
        ]
    );
    assert!(patterns[3].converted_preview().clamped_float_values);
    assert_eq!(
        patterns[4].converted_preview().rgba,
        [
            0, 10, 100, 255, 1, 20, 110, 0, 2, 30, 120, 64, 0, 40, 130, 255, 3, 50, 140, 128, 4,
            60, 150, 255
        ]
    );
    // The whole-file backend receives no patt sections, so its lossy pattern
    // reader cannot accidentally become the original resource representation.
    let prepared = preflight::inspect(SOURCE, &|| Ok(())).unwrap();
    let normalized = normalize::view(SOURCE, &prepared, &|| Ok(())).unwrap();
    assert!(
        ag_psd::abr::read_abr(&normalized, &Default::default())
            .unwrap()
            .patterns
            .is_empty()
    );
    assert_eq!(library.samples()[0].id, "raw-8");
    assert_eq!(library.brushes()[1].name, "Owned sampled");
}

#[test]
fn optional_palette_words_are_retained_even_when_the_word_is_three() {
    for (source, expected) in [
        (
            include_bytes!("../../../tests/abr-fixtures/indexed-no-extra.abr").as_slice(),
            None,
        ),
        (
            include_bytes!("../../../tests/abr-fixtures/indexed-extra3.abr").as_slice(),
            Some([0, 0, 0, 3]),
        ),
    ] {
        let library = crate::abr::decode(source, &|| Ok(())).unwrap();
        assert_eq!(library.patterns()[0].palette_extra, expected);
        if expected.is_some() {
            assert_eq!(
                library.patterns()[0].converted_preview().unavailable,
                Some(PreviewUnavailable::PaletteMetadata)
            );
            assert!(library.patterns()[0].converted_preview().rgba.is_empty());
        } else {
            assert_eq!(library.patterns()[0].converted_preview().unavailable, None);
            assert_eq!(
                library.patterns()[0].converted_preview().rgba,
                [
                    11, 22, 33, 255, 200, 100, 50, 128, 200, 100, 50, 64, 11, 22, 33, 0
                ]
            );
        }
    }
}

#[test]
fn auxiliary_channels_are_exact_and_explicitly_absent_from_the_thumbnail() {
    let library = crate::abr::decode(
        include_bytes!("../../../tests/abr-fixtures/auxiliary.abr"),
        &|| Ok(()),
    )
    .unwrap();
    let pattern = &library.patterns()[0];
    assert_eq!(
        channel(pattern, 3).samples(),
        &Samples::U16(vec![1, 255, 32769, 65535])
    );
    assert!(pattern.converted_preview().auxiliary_channels_not_rendered);
    assert!(!pattern.converted_preview().reduced_precision);
    assert_eq!(
        pattern.converted_preview().unavailable,
        Some(PreviewUnavailable::AdditionalColorChannels)
    );
    assert!(pattern.converted_preview().rgba.is_empty());
}

#[test]
fn absent_color_planes_and_user_masks_never_shift_other_channel_roles() {
    for (bytes, red) in [
        (
            include_bytes!("../../../tests/abr-fixtures/missing-color.abr").as_slice(),
            Slot::Unwritten,
        ),
        (
            include_bytes!("../../../tests/abr-fixtures/empty-color.abr").as_slice(),
            Slot::EmptyWritten,
        ),
    ] {
        let library = crate::abr::decode(bytes, &|| Ok(())).unwrap();
        let pattern = &library.patterns()[0];
        assert_eq!(pattern.slots()[0], red);
        assert_eq!(
            channel(pattern, 1).samples(),
            &Samples::U8(vec![100, 110, 120, 130])
        );
        assert_eq!(
            pattern.converted_preview().unavailable,
            Some(PreviewUnavailable::MissingColorChannels)
        );
        assert!(pattern.converted_preview().rgba.is_empty());
    }
    let library = crate::abr::decode(
        include_bytes!("../../../tests/abr-fixtures/user-mask.abr"),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        library.patterns()[0].converted_preview().unavailable,
        Some(PreviewUnavailable::UserMask)
    );
    assert!(library.patterns()[0].converted_preview().rgba.is_empty());
    assert_eq!(
        channel(&library.patterns()[0], 4).samples(),
        &Samples::U8(vec![0, 64, 128, 255])
    );
}

#[test]
fn proper_vmal_extents_versions_depths_and_rectangles_are_required() {
    let record = empty_record("x", 2, 2);
    // record prefix4, version4, mode4, point4, name-size4, name2, ID2:
    // VMAL version is at24, length at28, rectangle at32, slots at48.
    for (range, value) in [
        (4..8, 2u32),
        (8..12, 4),
        (8..12, 0x10003),
        (16..20, u32::MAX),
        (24..28, 2),
        (28..32, 0),
        (28..32, u32::MAX),
        (40..44, 0),
        (44..48, u32::MAX),
        (48..52, u32::MAX),
    ] {
        let mut bytes = record.clone();
        bytes[range].copy_from_slice(&value.to_be_bytes());
        assert!(crate::abr::decode(&file(&bytes), &|| Ok(())).is_err());
    }
    let mut malformed = record;
    malformed[52..56].copy_from_slice(&2u32.to_be_bytes());
    assert!(crate::abr::decode(&file(&malformed), &|| Ok(())).is_err());
}

#[test]
fn aggregate_pattern_counts_and_pixels_fail_during_complete_preflight() {
    assert_eq!(
        crate::abr::decode(
            &file(&empty_record("x", 1, 1).repeat(COUNT_MAX + 1)),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        crate::abr::decode(&file(&empty_record("x", 2048, 2048).repeat(5)), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        crate::abr::decode(&file(&empty_record("x", 16384, 16384)), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let max_name = "x".repeat(NAME_UNITS_MAX);
    assert_eq!(
        crate::abr::decode(&file(&empty_record(&max_name, 1, 1)), &|| Ok(()))
            .unwrap()
            .patterns()[0]
            .name,
        max_name
    );
}

#[test]
fn every_non_record_boundary_truncation_and_mutation_remains_safe() {
    let payload = source_payload();
    let mut boundaries = vec![0];
    let mut offset = 0;
    while offset < payload.len() {
        let n = u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4 + n.next_multiple_of(4);
        boundaries.push(offset);
    }
    for end in 0..payload.len() {
        if !boundaries.contains(&end) {
            assert!(
                crate::abr::decode(&file(&payload[..end]), &|| Ok(())).is_err(),
                "prefix {end}"
            );
        }
    }
    for index in 0..payload.len() {
        let mut bytes = payload.to_vec();
        bytes[index] = 255;
        let _ = crate::abr::decode(&file(&bytes), &|| Ok(()));
    }
}

#[test]
fn cancellation_at_every_pattern_checkpoint_never_returns_a_partial_resource() {
    let total = Cell::new(0);
    crate::abr::decode(SOURCE, &|| {
        total.set(total.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 0..total.get() {
        let calls = Cell::new(0);
        let result = crate::abr::decode(SOURCE, &|| {
            let n = calls.get();
            calls.set(n + 1);
            if n == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
}

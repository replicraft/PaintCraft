use super::*;
use std::cell::Cell;
const SMALL: &[u8] = include_bytes!("../../tests/jxr-fixtures/rgb8-4x3.jxr");
const EDGE: &[u8] = include_bytes!("../../tests/jxr-fixtures/rgb8-17x19.jxr");
fn field(bytes: &[u8], tag: u16) -> usize {
    let ifd = dword(bytes, 4).unwrap() as usize;
    (0..usize::from(word(bytes, ifd).unwrap()))
        .map(|i| ifd + 2 + i * 12)
        .find(|pos| word(bytes, *pos).unwrap() == tag)
        .unwrap()
}
fn set(bytes: &mut [u8], pos: usize, value: u32) {
    bytes[pos..pos + 4].copy_from_slice(&value.to_le_bytes());
}
#[test]
fn independently_generated_headers_are_admitted_without_decoding() {
    for (bytes, size) in [(SMALL, (4, 3)), (EDGE, (17, 19))] {
        let header = inspect_checked(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!((header.width, header.height), size);
        assert_eq!(header.image_offset + header.image_bytes, bytes.len());
        assert_eq!(header.container_version, 1);
    }
}
#[test]
fn every_truncation_and_budget_denial_is_rejected() {
    for length in 0..SMALL.len() {
        assert!(
            inspect_checked(&SMALL[..length], PIXELS_MAX, &|| Ok(())).is_err(),
            "length {length}"
        );
    }
    assert_eq!(
        inspect_checked(SMALL, 11, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut huge = SMALL.to_vec();
    let pos = field(&huge, 0xbc80) + 8;
    set(&mut huge, pos, u32::MAX);
    assert_eq!(
        inspect_checked(&huge, PIXELS_MAX, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut huge = SMALL.to_vec();
    set(&mut huge, 4, u32::MAX);
    assert!(inspect_checked(&huge, PIXELS_MAX, &|| Ok(())).is_err());
}
#[test]
fn duplicate_unknown_high_depth_alpha_and_metadata_fields_fail_closed() {
    for tag in [0xbc01u16, 0xbcc2, 0xbcc3, 0x8773, 0x8769, 0xffff] {
        let mut bytes = SMALL.to_vec();
        let pos = field(&bytes, 0xbc82);
        bytes[pos..pos + 2].copy_from_slice(&tag.to_le_bytes());
        assert!(
            inspect_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err(),
            "tag {tag:x}"
        );
    }
    for suffix in [0x0c, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x16, 0x19] {
        let mut bytes = SMALL.to_vec();
        bytes[23] = suffix;
        assert_eq!(
            inspect_checked(&bytes, PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
#[test]
fn contradictory_overlapping_and_unsupported_streams_are_rejected() {
    for tag in [0xbcc0, 0xbcc1, 0xbc02] {
        let mut bytes = SMALL.to_vec();
        let pos = field(&bytes, tag) + 8;
        set(&mut bytes, pos, 1);
        assert!(inspect_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let offset = inspect_checked(SMALL, PIXELS_MAX, &|| Ok(()))
        .unwrap()
        .image_offset;
    for (index, value) in [
        (0, 0),
        (8, 0x22),
        (9, 0x80),
        (10, 0xc1),
        (11, 0x72),
        (13, 0xff),
        (16, 0x80),
    ] {
        let mut bytes = SMALL.to_vec();
        bytes[offset + index] = value;
        assert!(
            inspect_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err(),
            "stream field {index}"
        );
    }
    let mut bytes = SMALL.to_vec();
    bytes.push(0);
    assert!(inspect_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
}
#[test]
fn every_inspection_checkpoint_can_cancel_without_calling_a_codec() {
    let count = Cell::new(0);
    inspect_checked(SMALL, PIXELS_MAX, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for cancel in 1..=count.get() {
        let seen = Cell::new(0);
        let err = inspect_checked(SMALL, PIXELS_MAX, &|| {
            seen.set(seen.get() + 1);
            if seen.get() == cancel {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
    }
}

const LONG_WIDE: &[u8] = include_bytes!("../../tests/jxr-fixtures/white-rgb8-4096x1.jxr");
const LONG_TALL: &[u8] = include_bytes!("../../tests/jxr-fixtures/white-rgb8-1x4096.jxr");
#[test]
fn fresh_long_geometry_does_not_broaden_public_import() {
    for (bytes, size) in [(LONG_WIDE, (4096, 1)), (LONG_TALL, (1, 4096))] {
        let header = inspect_fresh_rgb8_checked(bytes, 1_048_576, &|| Ok(())).unwrap();
        assert_eq!((header.width, header.height), size);
        assert_eq!(
            &bytes[header.image_offset + 8..header.image_offset + 12],
            &[0x11, 0, 0x40, 0x71]
        );
        assert_eq!(
            inspect_checked(bytes, PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    for bytes in [SMALL, EDGE] {
        let old = inspect_checked(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        let fresh = inspect_fresh_rgb8_checked(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(
            (old.width, old.height, old.image_offset, old.image_bytes),
            (
                fresh.width,
                fresh.height,
                fresh.image_offset,
                fresh.image_bytes
            )
        );
    }
}
#[test]
fn long_dimensions_flags_bounds_and_plane_remain_exact() {
    let offset = inspect_fresh_rgb8_checked(LONG_WIDE, PIXELS_MAX, &|| Ok(()))
        .unwrap()
        .image_offset;
    for field_offset in [12, 16] {
        let mut bytes = LONG_WIDE.to_vec();
        bytes[offset + field_offset..offset + field_offset + 4].fill(0xff);
        let error = inspect_fresh_rgb8_checked(&bytes, PIXELS_MAX, &|| Ok(())).unwrap_err();
        assert_eq!(error.code, ErrorCode::CodecFailed);
        assert!(error.message.contains("overflows"));
        bytes[offset + field_offset..offset + field_offset + 4]
            .copy_from_slice(&1u32.to_be_bytes());
        assert!(inspect_fresh_rgb8_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    }
    // Every other single-bit alteration of the fixed flags is refused. Flipping
    // SHORT_HEADER_FLAG alone also cannot reinterpret these 32-bit dimensions.
    for index in 8..12 {
        for bit in 0..8 {
            let mut bytes = LONG_WIDE.to_vec();
            bytes[offset + index] ^= 1 << bit;
            assert!(inspect_fresh_rgb8_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
        }
    }
    let mut bytes = LONG_WIDE.to_vec();
    bytes[offset + 20] ^= 1;
    assert!(inspect_fresh_rgb8_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    assert_eq!(
        inspect_fresh_rgb8_checked(LONG_WIDE, 4095, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut bytes = LONG_WIDE.to_vec();
    let width = field(&bytes, 0xbc80);
    set(&mut bytes, width + 8, 65_537);
    assert_eq!(
        inspect_fresh_rgb8_checked(&bytes, PIXELS_MAX, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn long_header_truncation_and_cancellation_are_checked() {
    let offset = inspect_fresh_rgb8_checked(LONG_WIDE, PIXELS_MAX, &|| Ok(()))
        .unwrap()
        .image_offset;
    for end in 0..LONG_WIDE.len() {
        assert!(inspect_fresh_rgb8_checked(&LONG_WIDE[..end], PIXELS_MAX, &|| Ok(())).is_err());
    }
    // Make the declared payload end match each truncated header, so rejection
    // also exercises the bounded geometry/plane reads rather than only the IFD.
    for length in 0..22 {
        let mut bytes = LONG_WIDE.to_vec();
        let size = field(&bytes, 0xbcc1);
        set(&mut bytes, size + 8, length as u32);
        bytes.truncate(offset + length);
        assert!(inspect_fresh_rgb8_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let error = inspect_fresh_rgb8_checked(LONG_WIDE, PIXELS_MAX, &|| {
        Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}

#[test]
fn private_writer_assessment_is_explicit_and_keeps_density_and_geometry_bounds() {
    for bytes in [SMALL, LONG_WIDE, LONG_TALL] {
        let assessment = assess_writer_import(bytes, 1_048_576, &|| Ok(())).unwrap();
        assert_eq!(assessment.losses(), &["jxr-resolution-not-retained"]);
        assert!(assessment.require_losses(&[]).is_err());
        assert_eq!(assessment.source().resolution_x_bits, Some(0));
        assert_eq!(assessment.source().resolution_y_bits, Some(0));
    }
    assert!(assess_import(LONG_WIDE, PIXELS_MAX, &|| Ok(())).is_err());
    for bits in [(-0f32).to_bits(), 96f32.to_bits()] {
        let mut bytes = SMALL.to_vec();
        let pos = field(&bytes, 0xbc82);
        set(&mut bytes, pos + 8, bits);
        assert!(assess_import(&bytes, PIXELS_MAX, &|| Ok(())).is_ok());
        assert!(assess_writer_import(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let mut bytes = SMALL.to_vec();
    bytes[3] = 0;
    assert!(assess_import(&bytes, PIXELS_MAX, &|| Ok(())).is_ok());
    assert!(assess_writer_import(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    assert_eq!(
        assess_writer_import(LONG_WIDE, 4095, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        assess_writer_import(SMALL, PIXELS_MAX, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}

#[test]
fn native_family_selection_preserves_short_admission_and_selects_long_without_fallback() {
    for bytes in [
        SMALL,
        EDGE,
        include_bytes!("../../tests/jxr-fixtures/rgb8-dpi96x144.jxr").as_slice(),
    ] {
        let selected = assess_native_import(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        assert!(matches!(selected, NativeImportAssessment::Legacy(_)));
        assert_eq!(
            selected.assessment(),
            &assess_import(bytes, PIXELS_MAX, &|| Ok(())).unwrap()
        );
    }
    for bytes in [LONG_WIDE, LONG_TALL] {
        let selected = assess_native_import(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        assert!(selected.is_writer_compatible());
        assert_eq!(
            selected.assessment(),
            &assess_writer_import(bytes, PIXELS_MAX, &|| Ok(())).unwrap()
        );
        let mut changed = bytes.to_vec();
        let pos = field(&changed, 0xbc82);
        set(&mut changed, pos + 8, 96f32.to_bits());
        assert_eq!(
            assess_native_import(&changed, PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}

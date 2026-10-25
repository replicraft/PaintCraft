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

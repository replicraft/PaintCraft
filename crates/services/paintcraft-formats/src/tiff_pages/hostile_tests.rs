use super::*;
use std::{
    cell::Cell,
    io::{Read, Seek, SeekFrom},
};

fn tag(bytes: &[u8], page: usize, wanted: u16) -> usize {
    let count = u16::from_le_bytes(bytes[page..page + 2].try_into().unwrap()) as usize;
    (0..count)
        .map(|i| page + 2 + i * 12)
        .find(|at| u16::from_le_bytes(bytes[*at..*at + 2].try_into().unwrap()) == wanted)
        .unwrap()
}
#[test]
fn whole_source_graph_rejects_cycles_overlaps_ranges_duplicates_and_truncations() {
    let original = tests::fixture(false);
    let changes: Vec<Vec<u8>> = [
        (130usize, 8u32),                         // first next IFD cycles
        (130, 20),                                // next directory overlaps the first
        (256, u32::MAX),                          // second next IFD outside source
        (tag(&original, 8, 273) + 8, u32::MAX),   // unselected payload offset
        (tag(&original, 134, 279) + 8, u32::MAX), // selected payload length
        (tag(&original, 8, 258) + 8, u32::MAX),   // metadata values outside source
    ]
    .into_iter()
    .map(|(at, value)| {
        let mut b = original.clone();
        b[at..at + 4].copy_from_slice(&value.to_le_bytes());
        b
    })
    .collect();
    for bytes in changes {
        assert!(inspect(&bytes, &|| Ok(())).is_err());
    }
    let mut duplicate = original.clone();
    let at = tag(&duplicate, 8, 277);
    duplicate[at..at + 2].copy_from_slice(&256u16.to_le_bytes());
    assert!(inspect(&duplicate, &|| Ok(())).is_err());
    for end in 0..original.len() {
        assert!(
            inspect(&original[..end], &|| Ok(())).is_err(),
            "prefix{end}"
        );
    }
    let mut unsupported = tests::fixture(true);
    let at = tag(&unsupported, 8, 273);
    unsupported[at + 8..at + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(
        inspect(&unsupported, &|| Ok(())).is_err(),
        "unsupported pages do not bypass global range checks"
    );
}
#[test]
fn backwards_acyclic_ifds_and_shared_payloads_remain_valid() {
    let mut bytes = tests::fixture(false);
    bytes[4..8].copy_from_slice(&134u32.to_le_bytes());
    bytes[256..260].copy_from_slice(&8u32.to_le_bytes());
    bytes[130..134].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        decode_page(&bytes, 0, 1, &|| Ok(())).unwrap().rgba,
        [40, 50, 60, 137]
    );
    assert_eq!(
        decode_page(&bytes, 1, 1, &|| Ok(())).unwrap().rgba,
        [17, 29, 91, 0]
    );
    let mut shared = tests::fixture(false);
    let at = tag(&shared, 134, 273);
    shared[at + 8..at + 12].copy_from_slice(&260u32.to_le_bytes());
    assert_eq!(
        decode_page(&shared, 1, 1, &|| Ok(())).unwrap().rgba,
        [17, 29, 91, 0]
    );
}
#[test]
fn projection_is_only_initial_header_intake_and_later_alias_reads_are_original() {
    let source = tests::fixture(false);
    let inventory = inspect(&source, &|| Ok(())).unwrap();
    let check = || Ok(());
    let mut reader = view::Selected::new(&source, inventory.pages()[1].descriptor, &check).unwrap();
    let mut header = [0; 8];
    reader.read_exact(&mut header).unwrap();
    assert_eq!(&header[4..], &134u32.to_le_bytes());
    reader.seek(SeekFrom::Start(0)).unwrap();
    let mut original = Vec::new();
    reader.read_to_end(&mut original).unwrap();
    assert_eq!(original, source);
    assert!(
        reader
            .seek(SeekFrom::Start(source.len() as u64 + 1))
            .is_err()
    );
    assert!(reader.seek(SeekFrom::Current(i64::MIN)).is_err());
}

#[test]
fn gray_alpha_photometric_projection_expires_before_aliased_pixel_bytes_are_read() {
    let mut bytes = b"II*\0\x08\0\0\0".to_vec();
    bytes.extend_from_slice(&10u16.to_le_bytes());
    for (tag, kind, count, value) in [
        (256u16, 4u16, 1u32, 1u32),
        (257, 4, 1, 1),
        (258, 3, 2, 0x0008_0008),
        (259, 3, 1, 1),
        (262, 3, 1, 0),
        (273, 4, 1, 66),
        (277, 3, 1, 2),
        (278, 4, 1, 1),
        (279, 4, 1, 2),
        (338, 3, 1, 2),
    ] {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&0u32.to_le_bytes());
    let original = bytes.clone();
    // The two raw pixel bytes intentionally alias the original WhiteIsZero
    // tag's inline value. A permanent projected value would produce254gray.
    let decoded = decode_page(&bytes, 0, 1, &|| Ok(())).unwrap();
    assert_eq!(decoded.rgba, [255, 255, 255, 0]);
    assert_eq!(decoded.assessment.descriptor().photometric, 0);
    assert_eq!(bytes, original);
}

#[test]
fn incompatible_compression_and_sparse_or_zero_strip_geometry_are_not_decoder_candidates() {
    for compression in [4, 7] {
        let mut bytes = tests::fixture(false);
        let at = tag(&bytes, 8, 259);
        bytes[at + 8..at + 12].copy_from_slice(&(compression as u32).to_le_bytes());
        let inventory = inspect(&bytes, &|| Ok(())).unwrap();
        assert!(inventory.pages()[0].unsupported_reason.is_some());
        assert_eq!(
            decode_page(&bytes, 1, 1, &|| Ok(())).unwrap().rgba,
            [40, 50, 60, 137]
        );
    }
    let mut bytes = tests::fixture(false);
    let at = tag(&bytes, 8, 278);
    bytes[at + 8..at + 12].copy_from_slice(&0u32.to_le_bytes());
    assert!(
        inspect(&bytes, &|| Ok(())).unwrap().pages()[0]
            .unsupported_reason
            .is_some()
    );
    let mut bytes = tests::fixture(false);
    for which in [273, 279] {
        let at = tag(&bytes, 8, which);
        bytes[at + 8..at + 12].copy_from_slice(&0u32.to_le_bytes());
    }
    assert!(
        inspect(&bytes, &|| Ok(())).unwrap().pages()[0]
            .unsupported_reason
            .is_some()
    );
    assert_eq!(
        decode_page(&bytes, 1, 1, &|| Ok(())).unwrap().rgba,
        [40, 50, 60, 137]
    );
}
fn chain(pages: usize, tags: usize, blob: usize) -> Vec<u8> {
    let size = 2 + tags * 12 + 4;
    let data_at = 8 + pages * size;
    let mut out = b"II*\0\x08\0\0\0".to_vec();
    for page in 0..pages {
        out.extend_from_slice(&(tags as u16).to_le_bytes());
        for index in 0..tags {
            let (tag, kind, count, value) = if index == 0 {
                (256u16, 4u16, 1u32, 1u32)
            } else if index == 1 {
                (257, 4, 1, 1)
            } else if blob > 0 {
                (65000, 7, blob as u32, data_at as u32)
            } else {
                ((1000 + index) as u16, 1, 1, 42)
            };
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&kind.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(
            &(if page + 1 < pages {
                (8 + (page + 1) * size) as u32
            } else {
                0
            })
            .to_le_bytes(),
        );
    }
    out.resize(out.len() + blob, 0);
    out
}
#[test]
fn page_tag_metadata_input_and_selected_pixel_budgets_are_checked_before_decode() {
    for bytes in [
        chain(PAGES_MAX + 1, 2, 0),
        chain(17, TAGS_PER_PAGE_MAX, 0),
        chain(5, 3, 900_000),
    ] {
        assert_eq!(
            inspect(&bytes, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    let mut bytes = tests::fixture(false);
    let at = tag(&bytes, 8, 258);
    bytes[at + 4..at + 8].copy_from_slice(&((METADATA_PER_PAGE_MAX as u32) + 1).to_le_bytes());
    assert_eq!(
        inspect(&bytes, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        inspect(&vec![0; BYTES_MAX + 1], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let bytes = tests::fixture(false);
    assert_eq!(
        decode_page(&bytes, 0, 0, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        decode_page(&bytes, 2, 1, &|| Ok(())).unwrap_err().code,
        ErrorCode::InputSchema
    );
    let mut big = bytes.clone();
    big[2] = 43;
    assert_eq!(
        inspect(&big, &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn cancellation_before_inventory_during_reads_and_before_assessment_never_mutates_source() {
    let bytes = tests::fixture(false);
    let original = bytes.clone();
    let calls = Cell::new(0usize);
    decode_page(&bytes, 1, 1, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let total = calls.get();
    assert!(total > 20);
    for stop in [1, total / 2, total - 1] {
        let calls = Cell::new(0usize);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() >= stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel fixture"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            decode_page(&bytes, 1, 1, &check).unwrap_err().code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(bytes, original);
    }
}
#[test]
fn canonical_assessment_binds_full_source_selection_descriptors_losses_and_output() {
    let bytes = tests::fixture(false);
    let result = decode_page(&bytes, 1, 1, &|| Ok(())).unwrap();
    let encoded = result.assessment.encode();
    for index in 0..encoded.len() {
        let mut changed = encoded;
        changed[index] ^= 0x80;
        if let Ok(assessment) = Assessment::decode(&changed) {
            assert_ne!(
                assessment.digest(),
                result.assessment.digest(),
                "report byte{index}"
            );
        }
    }
    let mut rgba = result.rgba.clone();
    rgba[0] ^= 1;
    assert!(
        result
            .assessment
            .validate_result(
                result.assessment.source_sha256(),
                bytes.len() as u64,
                1,
                1,
                1,
                &rgba
            )
            .is_err()
    );
    for (hash, len, index, w, h) in [
        ([0; 32], bytes.len() as u64, 1, 1, 1),
        (
            result.assessment.source_sha256(),
            bytes.len() as u64 + 1,
            1,
            1,
            1,
        ),
        (
            result.assessment.source_sha256(),
            bytes.len() as u64,
            0,
            1,
            1,
        ),
        (
            result.assessment.source_sha256(),
            bytes.len() as u64,
            1,
            2,
            1,
        ),
    ] {
        assert!(
            result
                .assessment
                .validate_result(hash, len, index, w, h, &result.rgba)
                .is_err()
        );
    }
    let mut changed = bytes.clone();
    changed[272] ^= 1;
    let fresh = decode_page(&changed, 1, 1, &|| Ok(())).unwrap();
    assert_ne!(
        fresh.assessment.source_sha256(),
        result.assessment.source_sha256()
    );
    assert!(
        fresh
            .assessment
            .validate_result(
                result.assessment.source_sha256(),
                bytes.len() as u64,
                1,
                1,
                1,
                &fresh.rgba
            )
            .is_err()
    );
}

use super::*;
pub(super) fn fixture(first_unsupported: bool) -> Vec<u8> {
    let mut out = b"II*\0\x08\0\0\0".to_vec();
    for index in 0..2 {
        let at = 8 + index * 126;
        assert_eq!(out.len(), at);
        out.extend_from_slice(&10u16.to_le_bytes());
        for (tag, kind, count, value) in [
            (256u16, 4u16, 1u32, 1u32),
            (257, 4, 1, 1),
            (258, 3, 4, (264 + index * 12) as u32),
            (259, 3, 1, 1),
            (
                262,
                3,
                1,
                if index == 0 && first_unsupported {
                    5
                } else {
                    2
                },
            ),
            (273, 4, 1, (260 + index * 12) as u32),
            (277, 3, 1, 4),
            (278, 4, 1, 1),
            (279, 4, 1, 4),
            (
                if index == 0 && first_unsupported {
                    65000
                } else {
                    338
                },
                3,
                1,
                2,
            ),
        ] {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&kind.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&(if index == 0 { 134u32 } else { 0 }).to_le_bytes());
    }
    // Payloads need not be contiguous with their IFD. All offsets are absolute.
    out.extend_from_slice(&[
        17, 29, 91, 0, 8, 0, 8, 0, 8, 0, 8, 0, 40, 50, 60, 137, 8, 0, 8, 0, 8, 0, 8, 0,
    ]);
    out
}
#[test]
fn selected_page_keeps_absolute_offsets_and_old_open_refuses_multiple_pages() {
    let bytes = fixture(false);
    let original = bytes.clone();
    let inventory = inspect(&bytes, &|| Ok(())).unwrap();
    assert_eq!(inventory.pages().len(), 2);
    for index in 0..2 {
        let decoded = decode_page(&bytes, index, 4, &|| Ok(())).unwrap();
        assert_eq!(
            decoded.rgba,
            if index == 0 {
                vec![17, 29, 91, 0]
            } else {
                vec![40, 50, 60, 137]
            }
        );
        assert_eq!(decoded.assessment.page_index(), index);
        assert_eq!(decoded.assessment.page_count(), 2);
        assert!(decoded.losses.iter().any(|v| v == LOSS_OTHER_PAGES));
        let report = decoded.assessment.encode();
        assert_eq!(Assessment::decode(&report).unwrap(), decoded.assessment);
        assert_eq!(report.len(), 256);
    }
    assert_eq!(bytes, original);
    assert!(crate::image::decode(&bytes).is_err());
}
#[test]
fn unsupported_first_image_does_not_block_independent_later_page() {
    let bytes = fixture(true);
    let inventory = inspect(&bytes, &|| Ok(())).unwrap();
    assert!(inventory.pages()[0].unsupported_reason.is_some());
    assert!(inventory.pages()[1].unsupported_reason.is_none());
    assert!(decode_page(&bytes, 0, 4, &|| Ok(())).is_err());
    assert_eq!(
        decode_page(&bytes, 1, 4, &|| Ok(())).unwrap().rgba,
        [40, 50, 60, 137]
    );
}

#[test]
fn source_pages_get_distinct_deterministic_document_state_and_layer_identities() {
    let bytes = fixture(false);
    let make = |index| {
        let decoded = decode_page(&bytes, index, 4, &|| Ok(())).unwrap();
        let identity = paintcraft_document::hex(&decoded.assessment.document_identity());
        paintcraft_document::import_image(decoded.width, decoded.height, &decoded.rgba, &identity)
            .unwrap()
    };
    let first = make(0);
    let again = make(0);
    let second = make(1);
    assert_eq!(first.document_id, again.document_id);
    assert_eq!(first.state_id, again.state_id);
    assert_eq!(first.roots, again.roots);
    assert_ne!(first.document_id, second.document_id);
    assert_ne!(first.state_id, second.state_id);
    assert_ne!(first.roots, second.roots);
}

//! Authored DCX directory fixtures, with selected PCX byte-layout oracles.
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::dcx;
fn pcx(rgb: [u8; 3]) -> Vec<u8> {
    let mut bytes = vec![0; 128];
    bytes[..4].copy_from_slice(&[10, 5, 1, 8]);
    bytes[65] = 3;
    bytes[66..68].copy_from_slice(&2u16.to_le_bytes());
    for color in rgb {
        if color >= 192 {
            bytes.extend([0xc1, color]);
        } else {
            bytes.push(color);
        }
        bytes.push(0);
    }
    bytes
}
fn container(pages: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![0; 4100];
    bytes[..4].copy_from_slice(&987_654_321u32.to_le_bytes());
    for (index, page) in pages.iter().enumerate() {
        let offset = bytes.len() as u32;
        bytes[4 + index * 4..8 + index * 4].copy_from_slice(&offset.to_le_bytes());
        bytes.extend(page);
    }
    bytes
}
#[test]
fn selected_pages_keep_exact_pixels_and_report_unimported_pages() {
    let bytes = container(&[pcx([10, 20, 30]), pcx([200, 210, 220])]);
    let pages = dcx::inspect(&bytes).unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!((pages[0].width, pages[0].height), (1, 1));
    assert_eq!(pages[0].offset, 4100);
    assert_eq!(pages[1].offset, pages[0].offset + pages[0].length);
    for (page, rgb) in [(0, [10, 20, 30]), (1, [200, 210, 220])] {
        let decoded = dcx::decode_page(&bytes, page, 1).unwrap();
        assert_eq!(decoded.rgba, [rgb[0], rgb[1], rgb[2], 255]);
        assert_eq!(
            decoded.losses,
            ["pcx-metadata-ignored", "dcx-other-pages-not-imported"]
        );
    }
    assert_eq!(
        dcx::decode_page(&bytes, 2, 1).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        dcx::decode_page(&bytes, 0, 0).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        dcx::decode_page(&container(&[pcx([1, 2, 3])]), 0, 1)
            .unwrap()
            .losses,
        ["pcx-metadata-ignored"]
    );
}
#[test]
fn directory_refuses_bad_offsets_termination_overlap_and_invalid_unselected_pages() {
    let valid = container(&[pcx([1, 2, 3]), pcx([4, 5, 6])]);
    for end in 0..4100 {
        assert!(dcx::inspect(&valid[..end]).is_err());
    }
    let mut duplicate = valid.clone();
    duplicate[8..12].copy_from_slice(&4100u32.to_le_bytes());
    assert!(dcx::inspect(&duplicate).is_err());
    let mut after_zero = valid.clone();
    after_zero[8..12].copy_from_slice(&0u32.to_le_bytes());
    after_zero[12..16].copy_from_slice(&4234u32.to_le_bytes());
    assert!(dcx::inspect(&after_zero).is_err());
    let mut out = valid.clone();
    out[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(dcx::inspect(&out).is_err());
    let mut overlap = valid.clone();
    overlap[8..12].copy_from_slice(&4101u32.to_le_bytes());
    assert!(dcx::inspect(&overlap).is_err());
    let mut broken = valid;
    let second = u32::from_le_bytes(broken[8..12].try_into().unwrap()) as usize;
    broken[second] = 0;
    assert!(
        dcx::decode_page(&broken, 0, 1)
            .unwrap_err()
            .message
            .contains("page 1")
    );
    assert!(dcx::inspect(&container(&[])).is_err());
}

use paintcraft_core::types::ErrorCode;
use paintcraft_formats::icons::{self, Encoding, Kind};

fn fixture(cursor: bool) -> Vec<u8> {
    let mut directory = ico::IconDir::new(if cursor {
        ico::ResourceType::Cursor
    } else {
        ico::ResourceType::Icon
    });
    for (size, png) in [(16, false), (32, true)] {
        let mut image = ico::IconImage::from_rgba_data(
            size,
            size,
            [10, 20, 30, 255].repeat((size * size) as usize),
        );
        if cursor {
            image.set_cursor_hotspot(Some((3, 7)));
        }
        directory.add_entry(if png {
            ico::IconDirEntry::encode_as_png(&image).unwrap()
        } else {
            ico::IconDirEntry::encode_as_bmp(&image).unwrap()
        });
    }
    let mut bytes = Vec::new();
    directory.write(&mut bytes).unwrap();
    bytes
}

#[test]
fn mixed_entries_require_explicit_selection_and_retain_hotspots() {
    for cursor in [false, true] {
        let bytes = fixture(cursor);
        let directory = icons::inspect(&bytes).unwrap();
        assert_eq!(
            directory.kind,
            if cursor { Kind::Cursor } else { Kind::Icon }
        );
        assert_eq!(directory.entries.len(), 2);
        assert_eq!(directory.entries[0].encoding, Encoding::Bitmap);
        assert_eq!(directory.entries[1].encoding, Encoding::Png);
        for index in 0..2 {
            let decoded = icons::decode_entry(&bytes, index, 65536).unwrap();
            assert_eq!(decoded.entry.width, if index == 0 { 16 } else { 32 });
            assert_eq!(decoded.entry.hotspot, cursor.then_some((3, 7)));
            assert_eq!(
                decoded.rgba,
                [10, 20, 30, 255].repeat((decoded.entry.width * decoded.entry.height) as usize)
            );
            assert!(
                decoded
                    .losses
                    .iter()
                    .any(|loss| loss == "icon-other-entries-not-imported")
            );
        }
        assert_eq!(
            icons::decode_entry(&bytes, 2, 65536).unwrap_err().code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            icons::decode_entry(&bytes, 1, 1023).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}

#[test]
fn rejects_truncation_overlap_conflicting_geometry_and_bad_hotspots() {
    let bytes = fixture(false);
    for length in 0..bytes.len() {
        assert!(icons::inspect(&bytes[..length]).is_err(), "length {length}");
    }
    for (position, replacement) in [(6, vec![17]), (9, vec![1]), (34, bytes[18..22].to_vec())] {
        let mut broken = bytes.clone();
        broken[position..position + replacement.len()].copy_from_slice(&replacement);
        assert!(icons::inspect(&broken).is_err(), "position {position}");
    }
    let mut cursor = fixture(true);
    cursor[10..12].copy_from_slice(&16u16.to_le_bytes());
    assert!(icons::inspect(&cursor).is_err());
    let mut suffix = bytes;
    suffix.push(0);
    assert!(icons::inspect(&suffix).is_err());
}

#[test]
fn rejects_compressed_bitmap_and_png_crc_before_decode() {
    let mut bytes = fixture(false);
    let entries = icons::inspect(&bytes).unwrap().entries;
    let dib = entries[0].offset as usize;
    bytes[dib + 16] = 1;
    assert_eq!(
        icons::inspect(&bytes).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    bytes[dib + 16] = 0;
    let png = entries[1].offset as usize;
    bytes[png + 29] ^= 1;
    assert!(icons::inspect(&bytes).is_err());
}

// Construct bytes directly, independently of ico's writer. Two bottom-up
// 24-bit BGR pixels and an AND mask expose row order, channel order and alpha.
fn raw_bitmap() -> Vec<u8> {
    let mut bytes = vec![0; 22 + 40 + 8 + 8];
    bytes[2..4].copy_from_slice(&2u16.to_le_bytes());
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
    bytes[6] = 1;
    bytes[7] = 2;
    bytes[12..14].copy_from_slice(&1u16.to_le_bytes()); // hotspot y
    bytes[14..18].copy_from_slice(&56u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&22u32.to_le_bytes());
    let dib = &mut bytes[22..];
    dib[..4].copy_from_slice(&40u32.to_le_bytes());
    dib[4..8].copy_from_slice(&1u32.to_le_bytes());
    dib[8..12].copy_from_slice(&4u32.to_le_bytes()); // doubled height
    dib[12..14].copy_from_slice(&1u16.to_le_bytes());
    dib[14..16].copy_from_slice(&24u16.to_le_bytes());
    dib[40..43].copy_from_slice(&[30, 20, 10]); // bottom
    dib[44..47].copy_from_slice(&[60, 50, 40]); // top
    dib[48] = 0x80; // bottom transparent
    bytes
}

#[test]
fn independently_constructed_cursor_preserves_orientation_channels_and_mask() {
    let decoded = icons::decode_entry(&raw_bitmap(), 0, 2).unwrap();
    assert_eq!(decoded.rgba, [40, 50, 60, 255, 10, 20, 30, 0]);
    assert_eq!(decoded.entry.hotspot, Some((0, 1)));
    assert_eq!(
        decoded.losses,
        ["cursor-hotspot-not-retained-in-flat-document"]
    );
}

#[test]
fn refuses_ambiguous_legacy_alpha_instead_of_turning_it_transparent() {
    let mut bytes = raw_bitmap();
    bytes[22 + 14..22 + 16].copy_from_slice(&32u16.to_le_bytes());
    assert_eq!(
        icons::decode_entry(&bytes, 0, 2).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    // Nonzero alpha with a meaningful AND-mask bit is also not qualified.
    bytes[22 + 43] = 255;
    assert_eq!(
        icons::decode_entry(&bytes, 0, 2).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    // Unambiguous alpha and no meaningful mask bits retain exact samples.
    bytes[22 + 48] = 0;
    let decoded = icons::decode_entry(&bytes, 0, 2).unwrap();
    assert_eq!(decoded.rgba, [40, 50, 60, 0, 10, 20, 30, 255]);
}

#[test]
fn directory_order_is_independent_of_payload_order() {
    let mut bytes = fixture(false);
    let first = bytes[6..22].to_vec();
    let second = bytes[22..38].to_vec();
    bytes[6..22].copy_from_slice(&second);
    bytes[22..38].copy_from_slice(&first);
    assert_eq!(
        icons::decode_entry(&bytes, 0, 1024).unwrap().entry.width,
        32
    );
    assert_eq!(icons::decode_entry(&bytes, 1, 256).unwrap().entry.width, 16);
}

#[test]
fn independent_fixtures_match_pillow_pixel_hashes_and_directory_metadata() {
    use sha2::{Digest, Sha256};
    let expected = [
        (
            17,
            1,
            "aa9775f00a4b994d9c0447ad60897a711adfd624a4bac5184fe1665ba93db65a",
        ),
        (
            19,
            4,
            "f0e464a4bbe06fd6e3e9c485bd56d77cf4807a49ca30017c6f3c1e560726188e",
        ),
        (
            21,
            8,
            "4bc5673638811c2f3378a77a6e3bf007367e0e8f08b41aaf2c301be9985c8651",
        ),
        (
            23,
            24,
            "6a9bd8b9457260ad4f889b78a6d173898eef5c2dfd9d943812f981cffe32424b",
        ),
        (
            25,
            32,
            "d8e88fece8aff8e001b897f1cc282e0afebf954b582de604800787a3a032a8d5",
        ),
        (
            256,
            32,
            "4e6383d5d7945e7dc4169d662c4dffc74169f601af0cba5f0551c4c082e5e2b5",
        ),
    ];
    for (bytes, cursor) in [
        (&include_bytes!("icon-fixtures/mixed.ico")[..], false),
        (&include_bytes!("icon-fixtures/mixed.cur")[..], true),
    ] {
        let directory = icons::inspect(bytes).unwrap();
        assert_eq!(directory.entries.len(), expected.len());
        for (index, &(size, depth, hash)) in expected.iter().enumerate() {
            let decoded = icons::decode_entry(bytes, index as u16, 65536).unwrap();
            assert_eq!((decoded.entry.width, decoded.entry.height), (size, size));
            assert_eq!(decoded.entry.bits_per_pixel, depth);
            assert_eq!(
                Sha256::digest(&decoded.rgba)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
                hash
            );
            assert_eq!(
                decoded.entry.hotspot,
                cursor.then_some((index as u16, index as u16 + 1))
            );
        }
    }
}

#[test]
fn selected_entries_have_distinct_deterministic_document_identity() {
    let bytes = fixture(false);
    let first = icons::decode_entry(&bytes, 0, 65536).unwrap();
    let second = icons::decode_entry(&bytes, 1, 65536).unwrap();
    let a = first.document("same-source-hash").unwrap();
    let b = second.document("same-source-hash").unwrap();
    assert_ne!(a.document_id, b.document_id);
    assert_ne!(a.state_id, b.state_id);
    assert_ne!(a.roots, b.roots);
    assert_eq!(
        a.document_id,
        first.document("same-source-hash").unwrap().document_id
    );
}

#[test]
fn png_palette_expansion_requires_the_same_loss_as_bitmap_palettes() {
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 2, 1);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(vec![10, 20, 30, 40, 50, 60]);
        encoder.set_trns(vec![0, 127]);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[0, 1])
            .unwrap();
    }
    let mut bytes = vec![0; 22];
    bytes[2..4].copy_from_slice(&1u16.to_le_bytes());
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
    bytes[6..9].copy_from_slice(&[2, 1, 2]);
    bytes[10..12].copy_from_slice(&1u16.to_le_bytes());
    bytes[12..14].copy_from_slice(&8u16.to_le_bytes());
    bytes[14..18].copy_from_slice(&(png.len() as u32).to_le_bytes());
    bytes[18..22].copy_from_slice(&22u32.to_le_bytes());
    bytes.extend(png);
    let decoded = icons::decode_entry(&bytes, 0, 2).unwrap();
    assert_eq!(decoded.rgba, [10, 20, 30, 0, 40, 50, 60, 127]);
    assert_eq!(decoded.losses, ["icon-palette-expanded"]);
}

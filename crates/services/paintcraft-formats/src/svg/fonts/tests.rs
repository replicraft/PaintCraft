use super::*;

#[test]
fn collection_face_inventory_is_bounded_before_upstream_reservation() {
    for count in [0u32, 65, u32::MAX] {
        let mut bytes = b"ttcf\0\x01\0\0".to_vec();
        bytes.extend(count.to_be_bytes());
        assert_eq!(
            Fonts::from_data(&[&bytes], "Selawik", &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    let mut truncated = b"ttcf\0\x01\0\0".to_vec();
    truncated.extend(1u32.to_be_bytes());
    assert_eq!(
        Fonts::from_data(&[&truncated], "Selawik", &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    truncated.extend(u32::MAX.to_be_bytes());
    assert_eq!(
        Fonts::from_data(&[&truncated], "Selawik", &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
}
#[test]
fn bounded_collection_faces_render_the_same_glyphs_as_the_original_font() {
    let font = BUNDLED[0];
    let mut collection = b"ttcf\0\x01\0\0".to_vec();
    collection.extend(1u32.to_be_bytes());
    collection.extend(16u32.to_be_bytes());
    collection.extend(font);
    let tables = usize::from(u16::from_be_bytes(font[4..6].try_into().unwrap()));
    for index in 0..tables {
        let at = 16 + 12 + index * 16 + 8;
        let offset = u32::from_be_bytes(collection[at..at + 4].try_into().unwrap());
        collection[at..at + 4].copy_from_slice(&(offset + 16).to_be_bytes());
    }
    let fonts = Fonts::from_data(&[&collection], "Selawik", &|| Ok(())).unwrap();
    let original = Fonts::from_data(&[font], "Selawik", &|| Ok(())).unwrap();
    let bytes=b"<svg xmlns='http://www.w3.org/2000/svg' width='64' height='32'><text y='24' font-family='Selawik' font-size='22'>Paint</text></svg>";
    let options = Options {
        width: 64,
        height: 32,
        dpi: 96.0,
        fit: Fit::Contain,
    };
    let render = |fonts| {
        prepare(bytes, options, fonts, PIXELS_MAX, &|| Ok(()))
            .unwrap()
            .render(&|| Ok(()))
            .unwrap()
    };
    assert_eq!(render(&fonts), render(&original));
}
#[test]
fn font_embedded_renderers_and_oversized_metadata_are_refused_before_parsing() {
    let mut legacy = BUNDLED[0].to_vec();
    legacy[..4].copy_from_slice(b"typ1");
    assert_eq!(
        Fonts::from_data(&[&legacy], "Selawik", &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    for tag in [
        *b"SVG ", *b"sbix", *b"CBDT", *b"CBLC", *b"EBDT", *b"EBLC", *b"COLR", *b"CPAL",
    ] {
        let mut bytes = BUNDLED[0].to_vec();
        bytes[12..16].copy_from_slice(&tag);
        assert_eq!(
            Fonts::from_data(&[&bytes], "Selawik", &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let mut bytes = BUNDLED[0].to_vec();
    bytes[4..6].copy_from_slice(&513u16.to_be_bytes());
    assert_eq!(
        Fonts::from_data(&[&bytes], "Selawik", &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut bytes = BUNDLED[0].to_vec();
    let tables = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
    let table = bytes[12..12 + tables * 16]
        .as_chunks::<16>()
        .0
        .iter()
        .find(|table| &table[..4] == b"name")
        .unwrap();
    let offset = u32::from_be_bytes(table[8..12].try_into().unwrap()) as usize;
    bytes[offset + 2..offset + 4].copy_from_slice(&1025u16.to_be_bytes());
    assert_eq!(
        Fonts::from_data(&[&bytes], "Selawik", &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}

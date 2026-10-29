//! Existing strict BMP admission is independent of the backend's padded palette.
use ::image::ImageDecoder;
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::image;
use std::{io::Cursor, path::Path};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/bmp-indexed-fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn existing_facade_keeps_independent_packed_rows_and_reserved_bytes_opaque() {
    for depth in [1, 4, 8] {
        for direction in ["top", "bottom"] {
            let name = format!("rgb{depth}-{direction}.bmp");
            let bytes = fixture(&name);
            let original = bytes.clone();
            let decoded = image::decode_with_pixel_limit(&bytes, 39).unwrap();
            assert_eq!((decoded.width, decoded.height), (13, 3));
            assert_eq!(
                decoded.rgba,
                fixture(&name.replace(".bmp", ".rgba")),
                "{name}"
            );
            assert_eq!(decoded.losses, ["palette-indices-expanded"]);
            assert_eq!(original, bytes);
            assert_eq!(
                image::decode_with_pixel_limit(&bytes, 38).unwrap_err().code,
                ErrorCode::ResourceLimit
            );
        }
    }
}

#[test]
fn existing_facade_refuses_undeclared_indices_which_backend_paints_black() {
    for depth in [1, 4, 8] {
        let bytes = fixture(&format!("rgb{depth}-bottom-undeclared.bmp"));
        let decoder = ::image::codecs::bmp::BmpDecoder::new(Cursor::new(&bytes)).unwrap();
        let mut rgb = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut rgb).unwrap();
        let index = if depth == 1 { 1 } else { 3 };
        assert_eq!(rgb[index * 3..index * 3 + 3], [0, 0, 0]);
        let error = image::decode(&bytes).unwrap_err();
        assert_eq!(error.code, ErrorCode::CodecFailed);
        assert!(error.message.contains("palette index"));
    }
}

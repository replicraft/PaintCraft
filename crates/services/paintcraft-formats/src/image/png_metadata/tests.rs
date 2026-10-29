use super::*;
use paintcraft_core::types::{Error, ErrorCode, json};
use std::{cell::Cell, io::Write};
const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/raster-metadata-fixtures"
);
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).unwrap()
}
fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = (data.len() as u32).to_be_bytes().to_vec();
    bytes.extend(kind);
    bytes.extend(data);
    bytes.extend(crc32fast::hash(&bytes[4..]).to_be_bytes());
    bytes
}
fn inserted(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = fixture("rgba16-high-byte.png");
    bytes.splice(33..33, chunk(kind, data));
    bytes
}
#[test]
fn independent_png_raw_samples_and_hidden_rgb_survive_current_pixel_conversion() {
    let oracle = json::parse(&fixture("oracle.json")).unwrap();
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("file").unwrap().as_str().unwrap();
        if !name.ends_with(".png") || name == "invalid-index.png" {
            continue;
        }
        let bytes = fixture(name);
        let descriptor = inspect(&bytes, 1_048_576, &|| Ok(())).unwrap();
        let decoded = super::super::decode_with_pixel_limit(&bytes, 1_048_576).unwrap();
        let expected: Vec<u8> = row
            .get("expected_rgba8")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as u8)
            .collect();
        assert_eq!(decoded.rgba, expected, "{name}");
        assert_eq!(
            (descriptor.width, descriptor.height),
            (decoded.width, decoded.height)
        );
        assert_eq!(
            paintcraft_core::types::policy::hex(&descriptor.source.sha256),
            row.get("sha256").unwrap().as_str().unwrap()
        );
    }
}
#[test]
fn palette_transparency_and_raw_precedence_resolution_are_described_without_management() {
    for depth in [1, 2, 4, 8] {
        let descriptor = inspect(
            &fixture(&format!("indexed{depth}-trns.png")),
            1_048_576,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(descriptor.bit_depth, depth);
        assert_eq!(descriptor.color_type, 3);
        assert_eq!(descriptor.palette.rgb[0], [13, 71, 201]);
        assert_eq!(descriptor.palette.rgb[1], [13, 71, 201]);
        assert_eq!(descriptor.palette.alpha[0], 0);
        assert_eq!(descriptor.losses(), ["palette-indices-expanded"]);
        assert_eq!(
            descriptor.packed_row_bytes(),
            Some((3 * u64::from(depth)).div_ceil(8))
        );
    }
    let srgb = inspect(&fixture("rgba8-srgb-phys-text.png"), 1_048_576, &|| Ok(())).unwrap();
    let interlaced = inspect(&fixture("indexed2-adam7.png"), 1_048_576, &|| Ok(())).unwrap();
    assert!(interlaced.interlaced);
    assert_eq!((interlaced.width, interlaced.height), (5, 5));
    assert_eq!(interlaced.packed_row_bytes(), None);
    assert_eq!(srgb.color_declaration(), ColorDeclaration::Srgb);
    assert_eq!(srgb.gamma_scaled, Some(45455));
    assert_eq!(srgb.resolution.unwrap().x, 3780);
    assert_eq!(srgb.resolution.unwrap().y, 7560);
    assert!(srgb.other_metadata);
    assert!(srgb.losses().contains(&"resolution-metadata-ignored"));
    let linear = inspect(&fixture("rgba8-aspect-gamma.png"), 1_048_576, &|| Ok(())).unwrap();
    assert_eq!(linear.gamma_scaled, Some(100000));
    assert_eq!(
        linear.resolution.unwrap().unit,
        facts::ResolutionUnit::Unknown
    );
    let icc = inspect(&fixture("rgba8-icc-precedence.png"), 1_048_576, &|| Ok(())).unwrap();
    assert_eq!(icc.color_declaration(), ColorDeclaration::Icc);
    assert_eq!(icc.gamma_scaled, Some(100000));
    assert_eq!(icc.icc.as_ref().unwrap().color_space, *b"RGB ");
    assert!(icc.losses().contains(&"icc-profile-ignored"));
    let cicp = inspect(&fixture("rgba8-cicp-precedence.png"), 1_048_576, &|| Ok(())).unwrap();
    assert_eq!(cicp.color_declaration(), ColorDeclaration::Cicp);
    assert_eq!(cicp.srgb_intent, Some(1));
}
#[test]
fn png_metadata_does_not_decode_indices_and_pinned_expand_is_not_an_index_validator() {
    let bytes = fixture("invalid-index.png");
    let descriptor = inspect(&bytes, 1_048_576, &|| Ok(())).unwrap();
    assert_eq!(descriptor.palette.rgb.len(), 1);
    // Explicit backend baseline: png0.18.1 EXPAND supplies black for undeclared
    // entries. The facade must separately validate indices during actual decode.
    let decoded = ::image::load_from_memory_with_format(&bytes, ::image::ImageFormat::Png).unwrap();
    assert_eq!(decoded.to_rgba8().into_raw(), [0, 0, 0, 255]);
}
#[test]
fn exif_before_or_after_idat_has_one_specific_loss_without_generic_metadata_duplication() {
    let bytes = fixture("rgba8-exif-orientation.png");
    let descriptor = inspect(&bytes, 1_048_576, &|| Ok(())).unwrap();
    let exif = descriptor.exif.as_ref().unwrap();
    assert_eq!(exif.bytes, 26);
    assert_eq!(descriptor.losses(), ["exif-metadata-ignored"]);
    assert!(!descriptor.other_metadata);
    // Move the original chunk after IDAT without changing its payload or CRC.
    let length = u32::from_be_bytes(bytes[33..37].try_into().unwrap()) as usize + 12;
    assert_eq!(&bytes[37..41], b"eXIf");
    let mut moved = bytes.clone();
    let original: Vec<u8> = moved.drain(33..33 + length).collect();
    let at = moved.len() - 12;
    moved.splice(at..at, original.clone());
    let later = inspect(&moved, 1_048_576, &|| Ok(())).unwrap();
    assert_eq!(later.exif, descriptor.exif);
    assert_eq!(later.losses(), ["exif-metadata-ignored"]);
    assert_eq!(
        later.omitted_ancillary_sha256,
        descriptor.omitted_ancillary_sha256
    );
    let at = moved.len() - 12;
    moved.splice(at..at, original);
    assert!(inspect(&moved, 1_048_576, &|| Ok(())).is_err());
    // A large omitted EXIF payload is streamed, never copied into the descriptor.
    let mut large = bytes.clone();
    large.splice(33..33 + length, chunk(b"eXIf", &vec![17; 1_048_577]));
    let descriptor = inspect(&large, 1_048_576, &|| Ok(())).unwrap();
    assert_eq!(descriptor.exif.as_ref().unwrap().bytes, 1_048_577);
    assert_eq!(descriptor.losses(), ["exif-metadata-ignored"]);
}
#[test]
fn omitted_payloads_stream_without_new_chunk_count_or_one_mib_defaults() {
    let mut bytes = fixture("rgba16-high-byte.png");
    let insert_at = bytes.len() - 12;
    let mut metadata = Vec::new();
    for _ in 0..4100 {
        metadata.extend(chunk(b"vpAg", &[]));
    }
    metadata.extend(chunk(b"vpAg", &vec![137; 1_048_577]));
    bytes.splice(insert_at..insert_at, metadata);
    let descriptor = inspect(&bytes, 1_048_576, &|| Ok(())).unwrap();
    assert!(descriptor.omitted_ancillary_count > 4096);
    assert!(descriptor.omitted_ancillary_bytes > 1_048_576);
    assert!(descriptor.losses().contains(&"png-metadata-ignored"));
    let calls = Cell::new(0);
    let result = inspect(&bytes, 1_048_576, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 5 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel streaming source hash",
            ))
        } else {
            Ok(())
        }
    });
    assert!(matches!(result, Err(e) if e.code == ErrorCode::OperationCancelled));
}
#[test]
fn malformed_duplicates_crc_order_color_profile_and_explicit_limits_refuse() {
    let gamma = 100000u32.to_be_bytes();
    let mut duplicate = inserted(b"gAMA", &gamma);
    duplicate.splice(33..33, chunk(b"gAMA", &gamma));
    assert!(inspect(&duplicate, 1_048_576, &|| Ok(())).is_err());
    assert!(inspect(&inserted(b"gAMA", &[0; 4]), 1_048_576, &|| Ok(())).is_err());
    let mut crc = fixture("rgba16-high-byte.png");
    crc[40] ^= 1;
    assert!(inspect(&crc, 1_048_576, &|| Ok(())).is_err());
    let mut trailing = fixture("rgba16-high-byte.png");
    trailing.extend(chunk(b"tEXt", b"k\0v"));
    assert!(inspect(&trailing, 1_048_576, &|| Ok(())).is_err());
    let mut huge = vec![0u8; 131072];
    huge[..4].copy_from_slice(&131072u32.to_be_bytes());
    huge[16..20].copy_from_slice(b"RGB ");
    huge[36..40].copy_from_slice(b"acsp");
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&huge).unwrap();
    let mut compressed = b"Bounded\0\0".to_vec();
    compressed.extend(z.finish().unwrap());
    let bomb = inserted(b"iCCP", &compressed);
    assert!(
        matches!(inspect_with_metadata_limit(&bomb,1_048_576,1024,&||Ok(())),Err(e) if e.code==ErrorCode::ResourceLimit)
    );
    assert_eq!(
        inspect(&bomb, 1_048_576, &|| Ok(()))
            .unwrap()
            .icc
            .unwrap()
            .payload
            .bytes,
        131072
    );
    assert!(
        inspect(&fixture("rgba8-icc-precedence.png"), 1_048_576, &|| Err(
            Error::new(ErrorCode::OperationCancelled, "stop")
        ))
        .is_err()
    );
    let calls = Cell::new(0);
    let source = fixture("rgba8-icc-precedence.png");
    let result = inspect(&source, 1_048_576, &|| {
        calls.set(calls.get() + 1);
        if calls.get() > 10 {
            Err(Error::new(ErrorCode::OperationCancelled, "stop chunk walk"))
        } else {
            Ok(())
        }
    });
    assert!(matches!(result,Err(e) if e.code==ErrorCode::OperationCancelled));
}

//! Fixtures were encoded independently by libjxl, never by PaintCraft's writer.
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::jxl::{self, Samples};

#[test]
fn independent_libjxl_rgba8_container_preserves_alpha_and_hidden_color() {
    let decoded =
        jxl::decode_with_pixel_limit(include_bytes!("jxl-fixtures/rgba8.jxl"), 6).unwrap();
    assert_eq!((decoded.width, decoded.height), (3, 2));
    assert_eq!(
        decoded.samples,
        Samples::Rgba8(vec![
            10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254,
            160, 170, 180, 255
        ])
    );
}
#[test]
fn independent_libjxl_rgba16_preserves_partial_alpha_and_hidden_color() {
    let decoded =
        jxl::decode_with_pixel_limit(include_bytes!("jxl-fixtures/rgba16.jxl"), 4).unwrap();
    assert_eq!(
        decoded.samples,
        Samples::Rgba16(vec![
            0, 1, 65535, 256, 257, 32767, 32768, 0, 65535, 1024, 2048, 32768, 4096, 8192, 16384,
            65535
        ])
    );
}
#[test]
fn independent_single_pixel_decodes_even_though_zune_writer_refuses_it() {
    let decoded =
        jxl::decode_with_pixel_limit(include_bytes!("jxl-fixtures/single.jxl"), 1).unwrap();
    assert_eq!((decoded.width, decoded.height), (1, 1));
    assert_eq!(decoded.samples, Samples::Rgba8(vec![17, 29, 91, 137]));
}
#[test]
fn linear_profile_requires_explicit_conversion() {
    assert_eq!(
        jxl::decode_with_pixel_limit(include_bytes!("jxl-fixtures/linear.jxl"), 4)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn external_container_metadata_is_reported_and_lengths_are_checked() {
    let mut source = include_bytes!("jxl-fixtures/rgba8.jxl").to_vec();
    source.extend_from_slice(&12u32.to_be_bytes());
    source.extend_from_slice(b"Exif");
    source.extend_from_slice(&[0, 0, 0, 0]);
    let info = jxl::inspect_checked(&source, 6, &|| Ok(())).unwrap();
    assert!(info.metadata_boxes);
    let decoded = jxl::decode_with_pixel_limit(&source, 6).unwrap();
    assert_eq!(decoded.losses, ["jxl-container-metadata-ignored"]);
    for len in [0usize, 1, 7, 8, 11] {
        let mut truncated = include_bytes!("jxl-fixtures/rgba8.jxl").to_vec();
        truncated.extend_from_slice(&source[source.len() - 12..source.len() - 12 + len]);
        if len > 0 {
            assert!(jxl::decode_with_pixel_limit(&truncated, 6).is_err());
        }
    }
    let mut overflow = include_bytes!("jxl-fixtures/rgba8.jxl").to_vec();
    overflow.extend_from_slice(&1u32.to_be_bytes());
    overflow.extend_from_slice(b"Exif");
    overflow.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(jxl::decode_with_pixel_limit(&overflow, 6).is_err());
}
/// Optional test-only oracle binaries are never linked into PaintCraft. When
/// unset this test remains explicitly ignored; the independent fixtures above
/// remain mandatory. Exact raw pixel assertions exercise both encoder depths.
#[test]
#[ignore = "requires an external independent libjxl djxl binary in PAINTCRAFT_DJXL"]
fn external_libjxl_decodes_zune_writer_pixels_when_oracle_is_supplied() {
    let executable = std::env::var_os("PAINTCRAFT_DJXL")
        .expect("supply a qualified independent djxl test executable");
    let base = std::env::temp_dir().join(format!("paintcraft-jxl-oracle-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    for sixteen in [false, true] {
        let source8 = vec![
            10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254,
            160, 170, 180, 255,
        ];
        let source16: Vec<u16> = (0..6)
            .flat_map(|i| [i * 257, i * 5001, 65535 - i * 997, 65535])
            .collect();
        let jxl = if sixteen {
            jxl::encode_rgba16(3, 2, &source16)
        } else {
            paintcraft_formats::image::encode(
                paintcraft_formats::image::ImageFormat::Jxl,
                3,
                2,
                &source8,
                100,
            )
        }
        .unwrap();
        let input = base.join("input.jxl");
        let output = base.join("output.png");
        std::fs::write(&input, jxl).unwrap();
        let status = std::process::Command::new(&executable)
            .arg(&input)
            .arg(&output)
            .arg("--bits_per_sample")
            .arg(if sixteen { "16" } else { "8" })
            .status()
            .unwrap();
        assert!(status.success());
        let decoded = image::load_from_memory(&std::fs::read(&output).unwrap()).unwrap();
        if sixteen {
            assert_eq!(decoded.to_rgba16().into_raw(), source16);
        } else {
            assert_eq!(decoded.to_rgba8().into_raw(), source8);
        }
    }
    std::fs::remove_dir_all(base).unwrap();
}
#[test]
fn independent_animation_is_inspected_and_requires_a_frame_workflow() {
    let bytes = include_bytes!("jxl-fixtures/animation.jxl");
    let info = jxl::inspect_checked(bytes, 2, &|| Ok(())).unwrap();
    assert!(info.animation);
    assert_eq!(
        jxl::decode_with_pixel_limit(bytes, 2).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn independent_icc_is_inspected_and_not_silently_ignored() {
    let bytes = include_bytes!("jxl-fixtures/icc.jxl");
    let info = jxl::inspect_checked(bytes, 6, &|| Ok(())).unwrap();
    assert!(info.custom_icc);
    assert_eq!(
        jxl::decode_with_pixel_limit(bytes, 6).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn every_external_container_prefix_is_rejected() {
    let bytes = include_bytes!("jxl-fixtures/rgba16.jxl");
    for end in 0..bytes.len() {
        assert!(
            jxl::decode_with_pixel_limit(&bytes[..end], 4).is_err(),
            "accepted incomplete container prefix {end}"
        );
    }
}
#[test]
fn container_metadata_and_box_count_admission_precede_pixel_decode() {
    let fixture = include_bytes!("jxl-fixtures/rgba8.jxl");
    let mut boxes = fixture.to_vec();
    for _ in 0..4097 {
        boxes.extend_from_slice(&8u32.to_be_bytes());
        boxes.extend_from_slice(b"free");
    }
    assert_eq!(
        jxl::decode_with_pixel_limit(&boxes, 6).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut metadata = fixture.to_vec();
    metadata.extend_from_slice(&((jxl::METADATA_BYTES_MAX + 9) as u32).to_be_bytes());
    metadata.extend_from_slice(b"Exif");
    metadata.resize(metadata.len() + jxl::METADATA_BYTES_MAX + 1, 0);
    assert_eq!(
        jxl::decode_with_pixel_limit(&metadata, 6).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}

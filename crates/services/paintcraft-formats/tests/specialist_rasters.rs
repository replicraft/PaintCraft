//! Real facade consumers of bounded specialist writers and qualified DDS input.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    avif, dds, export,
    image::{self, ImageFormat},
    jxl, pcnd,
};
use std::cell::Cell;

fn samples() -> Vec<u8> {
    vec![
        10, 20, 30, 0, 40, 50, 60, 128, 70, 80, 90, 255, 100, 110, 120, 64, 130, 140, 150, 255,
        160, 170, 180, 0,
    ]
}
#[test]
fn facade_writers_preserve_inputs_and_keep_lossless_or_lossy_profiles_explicit() {
    let pixels = samples();
    let before = pixels.clone();
    for format in [ImageFormat::Jxl, ImageFormat::Dds, ImageFormat::Avif] {
        let bytes = image::encode(format, 3, 2, &pixels, 100).unwrap();
        assert_eq!(image::ImageFormat::sniff(&bytes), Some(format));
        match format {
            ImageFormat::Jxl => {
                let result = jxl::decode_with_pixel_limit(&bytes, 6).unwrap();
                assert_eq!(result.samples, jxl::Samples::Rgba8(pixels.clone()));
                assert!(format.export_losses_for_rgba(&pixels).is_empty());
                assert_eq!(
                    image::decode(&bytes).unwrap_err().code,
                    ErrorCode::CapabilityUnsupported,
                    "writer does not qualify the unconfined reader"
                );
            }
            ImageFormat::Dds => {
                assert_eq!(&bytes[..4], b"DDS ");
                assert_eq!(
                    u32::from_le_bytes(bytes[128..132].try_into().unwrap()),
                    29,
                    "explicit DXGI RGBA8 sRGB"
                );
                assert_eq!(
                    u32::from_le_bytes(bytes[144..148].try_into().unwrap()),
                    1,
                    "explicit straight alpha"
                );
                assert_eq!(&bytes[148..], pixels);
                assert_eq!(
                    image::decode_with_pixel_limit(&bytes, 6).unwrap().rgba,
                    pixels
                );
                assert!(format.export_losses_for_rgba(&pixels).is_empty());
            }
            ImageFormat::Avif => {
                assert_eq!(
                    format.export_losses_for_rgba(&pixels),
                    ["lossy-avif", "alpha-lossy", "transparent-rgb-discarded"]
                );
                assert_eq!(
                    image::decode(&bytes).unwrap_err().code,
                    ErrorCode::CapabilityUnsupported,
                    "AVIF encoder does not provide a decoder"
                );
            }
            _ => unreachable!(),
        }
        assert_eq!(pixels, before);
    }
}
#[test]
fn dds_facade_rejects_linear_unknown_associated_alpha_and_multisurface_profiles() {
    let pixels = samples();
    let original = image::encode(ImageFormat::Dds, 3, 2, &pixels, 90).unwrap();
    for (offset, value) in [(128, 28u32), (144, 0), (144, 2), (140, 2), (136, 4)] {
        let mut bytes = original.clone();
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert_eq!(
            image::decode(&bytes).unwrap_err().code,
            ErrorCode::CapabilityUnsupported,
            "offset {offset}, value {value}"
        );
    }
    let mut opaque = original.clone();
    opaque[144..148].copy_from_slice(&3u32.to_le_bytes());
    let mut expected = pixels.clone();
    for pixel in expected.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    assert_eq!(image::decode(&opaque).unwrap().rgba, expected);
    let mut legacy = original[..128].to_vec();
    legacy.extend(&original[148..]);
    for (offset, value) in [
        (80, 0x41u32),
        (84, 0),
        (88, 32),
        (92, 0xff),
        (96, 0xff00),
        (100, 0xff0000),
        (104, 0xff000000),
    ] {
        legacy[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(
        dds::inspect_checked(&legacy, 6, &|| Ok(()))
            .unwrap()
            .transfer,
        dds::Transfer::Unspecified
    );
    assert_eq!(
        image::decode(&legacy).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        image::decode_with_pixel_limit(&original, 5)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn dds_srgb_bc1_surface_uses_the_same_explicit_transfer_and_alpha_gate() {
    let mut bytes = image::encode(ImageFormat::Dds, 4, 4, &[255; 64], 90).unwrap();
    bytes.truncate(148);
    for (offset, value) in [(8, 0x81007u32), (20, 8), (128, 72)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes.extend([0, 0xf8, 0xe0, 7, 0, 0, 0, 0]);
    assert_eq!(
        image::decode(&bytes).unwrap().rgba,
        [255, 0, 0, 255].repeat(16)
    );
}
#[test]
fn avif_export_preflights_exact_losses_before_encoding_and_never_changes_the_source() {
    let pixels = samples();
    let document = paintcraft_document::import_image(3, 2, &pixels, &"0".repeat(64)).unwrap();
    let original = pcnd::write(&document).unwrap();
    for allowed in [
        vec![],
        vec!["lossy-avif".into()],
        vec![
            "hidden-rgb".into(),
            "lossy-avif".into(),
            "transparent-rgb-discarded".into(),
        ],
    ] {
        assert_eq!(
            export::export_flat(&document, ImageFormat::Avif, 100, &allowed)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(pcnd::write(&document).unwrap(), original);
    }
    let allowed = ["hidden-rgb", "lossy-avif", "alpha-lossy"].map(str::to_owned);
    let exported = export::export_flat(&document, ImageFormat::Avif, 100, &allowed).unwrap();
    assert_eq!(exported.losses, allowed);
    assert_eq!(
        image::ImageFormat::sniff(&exported.bytes),
        Some(ImageFormat::Avif)
    );
    assert_eq!(pcnd::write(&document).unwrap(), original);
    for quality in [1, 50, 100] {
        let direct = avif::encode_rgba8(3, 2, &pixels, quality, &|| Ok(())).unwrap();
        assert_eq!(
            direct.losses,
            ImageFormat::Avif.export_losses_for_rgba(&pixels)
        );
    }
    assert_eq!(
        ImageFormat::Avif.export_losses_for_rgba(&[1, 2, 3, 255]),
        ["lossy-avif"]
    );
    assert_eq!(
        ImageFormat::Avif.export_losses_for_rgba(&[0, 0, 0, 0]),
        ["lossy-avif", "alpha-lossy"]
    );
    assert_eq!(
        ImageFormat::Avif.export_losses(true),
        ["lossy-avif", "alpha-lossy", "transparent-rgb-discarded"],
        "legacy boolean preflight stays conservative"
    );
}
#[test]
fn checked_facade_cancellation_never_returns_encoded_bytes_and_adapter_limits_stay_in_force() {
    let pixels = samples();
    for format in [ImageFormat::Jxl, ImageFormat::Dds, ImageFormat::Avif] {
        let calls = Cell::new(0usize);
        image::encode_checked(format, 3, 2, &pixels, 90, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=calls.get() {
            let seen = Cell::new(0usize);
            let check = || -> Result<()> {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "injected encode cancellation",
                    ))
                } else {
                    Ok(())
                }
            };
            assert_eq!(
                image::encode_checked(format, 3, 2, &pixels, 90, &check)
                    .unwrap_err()
                    .code,
                ErrorCode::OperationCancelled,
                "{format:?} checkpoint {stop}"
            );
        }
    }
    assert_eq!(
        image::encode(ImageFormat::Jxl, 1, 2, &[255; 8], 90)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        image::encode(ImageFormat::Avif, 3, 2, &pixels, 0)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        image::encode(ImageFormat::Avif, 3, 2, &pixels, 101)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    for format in [ImageFormat::Jxl, ImageFormat::Dds, ImageFormat::Avif] {
        assert!(image::encode(format, 3, 2, &pixels[..20], 90).is_err());
    }
}

#[test]
#[ignore = "requires an independent Pillow installation via PAINTCRAFT_PILLOW_PYTHON"]
fn pillow_independently_reopens_facade_dds_rgba_pixels() {
    use std::process::{Command, Stdio};
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let folder = Scratch(std::env::temp_dir().join(format!(
        "paintcraft-dds-facade-oracle-{}",
        std::process::id()
    )));
    std::fs::create_dir_all(&folder.0).unwrap();
    let pixels = samples();
    let bytes = image::encode(ImageFormat::Dds, 3, 2, &pixels, 90).unwrap();
    let path = folder.0.join("sample.dds");
    std::fs::write(&path, &bytes).unwrap();
    let python = std::env::var_os("PAINTCRAFT_PILLOW_PYTHON")
        .expect("set PAINTCRAFT_PILLOW_PYTHON to an independent Pillow test interpreter");
    let result=Command::new(&python).args(["-c","from PIL import Image; import sys; im=Image.open(sys.argv[1]); im.load(); sys.stdout.buffer.write(im.convert('RGBA').tobytes())"]).arg(&path)
        .stdin(Stdio::null()).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, pixels);
    if let Some(output) = std::env::var_os("PAINTCRAFT_DDS_ORACLE_DIR") {
        let output = std::path::PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(output.join("rgba8-srgb-straight.dds"), &bytes).unwrap();
        std::fs::write(output.join("source.rgba"), &pixels).unwrap();
        std::fs::write(output.join("pillow.rgba"), &result.stdout).unwrap();
        let version = Command::new(python)
            .args(["-c", "import PIL; print(PIL.__version__)"])
            .output()
            .unwrap();
        assert!(version.status.success());
        std::fs::write(output.join("pillow-version.txt"), version.stdout).unwrap();
    }
}

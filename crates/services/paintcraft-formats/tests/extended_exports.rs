//! Bounded extended writers preserve RGBA8; Netpbm output is explicitly PAM P7.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    export,
    image::{self, ImageFormat},
    pcnd,
};
use std::cell::Cell;
fn pixels() -> Vec<u8> {
    vec![
        10, 20, 30, 0, 40, 50, 60, 128, 70, 80, 90, 255, 100, 110, 120, 64, 130, 140, 150, 255,
        160, 170, 180, 0,
    ]
}
#[test]
fn lossless_extended_writers_keep_pixels_alpha_hidden_rgb_and_source_bytes() {
    for (width, height) in [(3, 2), (1, 6), (6, 1)] {
        let rgba = pixels();
        let before = rgba.clone();
        for format in [ImageFormat::Qoi, ImageFormat::Pnm, ImageFormat::Farbfeld] {
            let bytes = image::encode(format, width, height, &rgba, 90).unwrap();
            let decoded = image::decode(&bytes).unwrap();
            assert_eq!(decoded.format, format);
            assert_eq!((decoded.width, decoded.height), (width, height));
            assert_eq!(decoded.rgba, rgba, "{format:?}");
            assert_eq!(rgba, before);
            assert!(format.export_losses_for_rgba(&rgba).is_empty());
            match format {
                ImageFormat::Qoi => {
                    assert_eq!(&bytes[..4], b"qoif");
                    assert_eq!(&bytes[12..14], &[4, 0]);
                    assert!(decoded.losses.is_empty());
                }
                ImageFormat::Pnm => {
                    assert!(bytes.starts_with(b"P7\n"));
                    assert!(
                        bytes
                            .windows(b"TUPLTYPE RGB_ALPHA".len())
                            .any(|v| v == b"TUPLTYPE RGB_ALPHA")
                    );
                    assert!(
                        bytes
                            .windows(b"MAXVAL 255".len())
                            .any(|v| v == b"MAXVAL 255")
                    );
                    assert!(decoded.losses.is_empty());
                }
                ImageFormat::Farbfeld => {
                    assert_eq!(&bytes[..8], b"farbfeld");
                    assert_eq!(bytes.len(), 16 + rgba.len() * 2);
                    for (value, pair) in rgba.iter().zip(bytes[16..].as_chunks::<2>().0) {
                        assert_eq!(*pair, [*value, *value]);
                    }
                    assert_eq!(
                        decoded.losses,
                        ["depth-16-reduced-to-8"],
                        "read precision semantics remain explicit despite exact widening"
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}
#[test]
fn every_observed_writer_cancellation_discards_private_output_and_limits_are_admitted() {
    let rgba = pixels();
    for format in [ImageFormat::Qoi, ImageFormat::Pnm, ImageFormat::Farbfeld] {
        let calls = Cell::new(0usize);
        image::encode_checked(format, 3, 2, &rgba, 90, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=calls.get() {
            let seen = Cell::new(0usize);
            let error = image::encode_checked(format, 3, 2, &rgba, 90, &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "injected writer cancellation",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(
                error.code,
                ErrorCode::OperationCancelled,
                "{format:?} checkpoint {stop}"
            );
        }
        assert!(image::encode(format, 3, 2, &rgba[..20], 90).is_err());
    }
    let oversized = vec![0u8; 2049 * 2048 * 4];
    const { assert!(2049 * 2048 > image::EXTENDED_ENCODE_PIXELS_MAX) };
    for format in [ImageFormat::Qoi, ImageFormat::Pnm, ImageFormat::Farbfeld] {
        assert_eq!(
            image::encode(format, 2049, 2048, &oversized, 90)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    assert_eq!(
        image::encode(ImageFormat::Pcx, 3, 2, &rgba, 90)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn document_export_still_requires_hidden_rgb_and_flatten_loss_consent() {
    let document = paintcraft_document::import_image(3, 2, &pixels(), &"0".repeat(64)).unwrap();
    let original = pcnd::write(&document).unwrap();
    for format in [ImageFormat::Qoi, ImageFormat::Pnm, ImageFormat::Farbfeld] {
        assert_eq!(
            export::export_flat(&document, format, 90, &[])
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        let flat = export::export_flat(&document, format, 90, &["hidden-rgb".into()]).unwrap();
        assert_eq!(flat.losses, ["hidden-rgb"]);
        assert_eq!(
            image::decode(&flat.bytes).unwrap().rgba,
            paintcraft_render::render_flat(&document).unwrap()
        );
        assert_eq!(pcnd::write(&document).unwrap(), original);
        let mut layered = document.clone();
        let mut extra = layered.layers[&layered.roots[0]].clone();
        extra.id = uuid::Uuid::from_u128(1);
        extra.visible = false;
        layered.roots.push(extra.id);
        layered.layers.insert(extra.id, extra);
        let layered_before = pcnd::write(&layered).unwrap();
        assert_eq!(
            export::export_flat(&layered, format, 90, &["hidden-rgb".into()])
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        let flat = export::export_flat(
            &layered,
            format,
            90,
            &["hidden-rgb".into(), "flatten".into()],
        )
        .unwrap();
        assert_eq!(flat.losses, ["flatten", "hidden-rgb"]);
        assert_eq!(pcnd::write(&layered).unwrap(), layered_before);
    }
}
#[test]
#[ignore = "requires FFmpeg and the official farbfeld ff2pam reference decoder via PAINTCRAFT_FF2PAM"]
fn independent_ffmpeg_decodes_qoi_pam_and_farbfeld_16bit_samples() {
    use std::process::{Command, Stdio};
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let folder = Scratch(std::env::temp_dir().join(format!(
        "paintcraft-extended-export-oracle-{}",
        std::process::id()
    )));
    std::fs::create_dir_all(&folder.0).unwrap();
    let rgba = pixels();
    for (format, extension) in [
        (ImageFormat::Qoi, "qoi"),
        (ImageFormat::Pnm, "pam"),
        (ImageFormat::Farbfeld, "ff"),
    ] {
        let bytes = image::encode(format, 3, 2, &rgba, 100).unwrap();
        let input = folder.0.join(format!("sample.{extension}"));
        std::fs::write(&input, &bytes).unwrap();
        let executable = std::env::var_os("PAINTCRAFT_FFMPEG").unwrap_or_else(|| "ffmpeg".into());
        let mut command = Command::new(executable);
        command.args(["-hide_banner", "-loglevel", "error"]);
        let mut decode_input = input.clone();
        let mut reference = None;
        let (pixel_format, expected) = if format == ImageFormat::Farbfeld {
            // Use the official C reference to parse the complete farbfeld file,
            // then independently decode its 16-bit PAM output with FFmpeg. Keep
            // all 16 bits: an 8-bit conversion can introduce decoder dithering.
            let ff2pam = std::env::var_os("PAINTCRAFT_FF2PAM")
                .expect("set PAINTCRAFT_FF2PAM to the official farbfeld reference decoder");
            let result = Command::new(ff2pam)
                .stdin(Stdio::from(std::fs::File::open(&input).unwrap()))
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            decode_input = folder.0.join("reference.pam");
            std::fs::write(&decode_input, &result.stdout).unwrap();
            reference = Some(result.stdout);
            (
                "rgba64be",
                rgba.iter().flat_map(|v| [*v, *v]).collect::<Vec<_>>(),
            )
        } else {
            ("rgba", rgba.clone())
        };
        let output = command
            .arg("-i")
            .arg(&decode_input)
            .args([
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                pixel_format,
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{format:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout, expected,
            "independent {format:?} sample decoder"
        );
        if let Some(path) = std::env::var_os("PAINTCRAFT_EXTENDED_EXPORT_ORACLE_DIR") {
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join(format!("sample.{extension}")), &bytes).unwrap();
            if let Some(reference) = reference {
                std::fs::write(path.join("farbfeld-reference.pam"), reference).unwrap();
            }
            std::fs::write(path.join(format!("{extension}.source.rgba")), &rgba).unwrap();
            std::fs::write(
                path.join(format!("{extension}.decoded.{pixel_format}")),
                &output.stdout,
            )
            .unwrap();
        }
    }
}

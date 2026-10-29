use super::*;

#[test]
fn rgb_scratch_matches_independent_integer_rounding_for_every_alpha_and_keeps_hidden_source_immutable()
 {
    for source in [0u8, 1, 37, 128, 254, 255] {
        for backdrop in [0u8, 1, 64, 127, 128, 254, 255] {
            let color = [source, 255 - source, source ^ 0x55];
            let matte = [backdrop, backdrop ^ 0xaa, 255 - backdrop];
            let rgba: Vec<u8> = (0..=255)
                .flat_map(|alpha| [color[0], color[1], color[2], alpha])
                .collect();
            let before = rgba.clone();
            let rgb = prepare(256, 1, &rgba, JpegMatte::new(matte), 768, &|| Ok(())).unwrap();
            for (alpha, pixel) in rgb.as_chunks::<3>().0.iter().enumerate() {
                for channel in 0..3 {
                    let numerator = u64::from(color[channel]) * alpha as u64
                        + u64::from(matte[channel]) * (255 - alpha) as u64;
                    let quotient = numerator / 255;
                    let remainder = numerator % 255;
                    assert_eq!(
                        u64::from(pixel[channel]),
                        quotient + u64::from(remainder * 2 > 255)
                    );
                }
            }
            assert_eq!(&rgb[..3], &matte);
            assert_eq!(&rgb[rgb.len() - 3..], &color);
            assert_eq!(rgba, before);
        }
    }
}

#[test]
fn legacy_white_and_explicit_white_produce_identical_jpeg_bytes_and_custom_matte_reaches_encoder() {
    let rgba = [
        11, 22, 33, 0, 44, 55, 66, 255, 77, 88, 99, 128, 255, 0, 128, 1,
    ];
    for quality in [1, 37, 90, 100] {
        let expected_rgb = [255, 255, 255, 44, 55, 66, 166, 171, 177, 255, 254, 255];
        let mut baseline = Vec::new();
        ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut baseline, quality)
            .encode(&expected_rgb, 2, 2, ::image::ExtendedColorType::Rgb8)
            .unwrap();
        let old = super::super::encode(ImageFormat::Jpeg, 2, 2, &rgba, quality).unwrap();
        assert_eq!(old, baseline);
        let explicit = encode_checked(2, 2, &rgba, quality, JpegMatte::WHITE, &|| Ok(())).unwrap();
        assert_eq!(old, explicit);
        let black =
            encode_checked(2, 2, &rgba, quality, JpegMatte::new([0; 3]), &|| Ok(())).unwrap();
        assert_ne!(black, old);
    }
    // Fully transparent source samples become the chosen backdrop before lossy
    // JPEG encoding; a constant plane remains close under independent decoding.
    let hidden: Vec<u8> = (0..64).flat_map(|_| [17, 92, 231, 0]).collect();
    let bytes = encode_checked(8, 8, &hidden, 100, JpegMatte::new([32, 160, 224]), &|| {
        Ok(())
    })
    .unwrap();
    let decoded = ::image::load_from_memory_with_format(&bytes, ::image::ImageFormat::Jpeg)
        .unwrap()
        .into_rgb8();
    assert!(decoded.pixels().all(|pixel| {
        pixel
            .0
            .into_iter()
            .zip([32u8, 160, 224])
            .all(|(a, b)| a.abs_diff(b) <= 2)
    }));
}

#[test]
fn scratch_limits_invalid_layout_and_every_observed_cancellation_refuse_private_output() {
    let rgba = vec![37; 8193 * 4];
    assert_eq!(
        prepare(8193, 1, &rgba, JpegMatte::WHITE, 8193 * 3 - 1, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(
        prepare(
            8193,
            1,
            &rgba[..4],
            JpegMatte::WHITE,
            usize::MAX,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(prepare(0, 1, &[], JpegMatte::WHITE, usize::MAX, &|| Ok(())).is_err());
    assert!(prepare(65536, 1, &[], JpegMatte::WHITE, usize::MAX, &|| Ok(())).is_err());
    let calls = std::cell::Cell::new(0);
    prepare(8193, 1, &rgba, JpegMatte::WHITE, usize::MAX, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(calls.get() >= 5);
    for stop in 1..=calls.get() {
        let seen = std::cell::Cell::new(0);
        let result = prepare(8193, 1, &rgba, JpegMatte::WHITE, usize::MAX, &|| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel matte"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
}

#[test]
fn typed_rgb_matte_accepts_only_exact_hex_rgb() {
    assert_eq!("aAbB00".parse::<JpegMatte>().unwrap().rgb(), [170, 187, 0]);
    assert_eq!("#ffffff".parse::<JpegMatte>().unwrap(), JpegMatte::WHITE);
    for invalid in [
        "",
        "fff",
        "0xffffff",
        "#GGGGGG",
        "１２３",
        "ffffff00",
        " ffffff",
    ] {
        assert_eq!(
            invalid.parse::<JpegMatte>().unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
}

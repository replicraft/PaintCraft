use super::*;
type Fixture = (&'static [u8], &'static [u8], u32, bool, bool);
const FILES: [Fixture; 10] = [
    (
        include_bytes!("../../../../tests/heif-color-fixtures/identity444.heic"),
        include_bytes!("../../../../tests/heif-color-fixtures/identity444.libheif-bilinear-rgb"),
        3,
        true,
        false,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/identity444-alpha.heic"),
        include_bytes!(
            "../../../../tests/heif-color-fixtures/identity444-alpha.libheif-bilinear-rgba"
        ),
        3,
        true,
        true,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-full.heic"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-full.libheif-bilinear-rgb"),
        3,
        true,
        false,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-full-alpha.heic"),
        include_bytes!(
            "../../../../tests/heif-color-fixtures/yuv444-full-alpha.libheif-bilinear-rgba"
        ),
        3,
        true,
        true,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-limited.heic"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-limited.libheif-bilinear-rgb"),
        3,
        false,
        false,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-limited-alpha.heic"),
        include_bytes!(
            "../../../../tests/heif-color-fixtures/yuv444-limited-alpha.libheif-bilinear-rgba"
        ),
        3,
        false,
        true,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-full.heic"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-full.libheif-bilinear-rgb"),
        1,
        true,
        false,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-full-alpha.heic"),
        include_bytes!(
            "../../../../tests/heif-color-fixtures/yuv420-full-alpha.libheif-bilinear-rgba"
        ),
        1,
        true,
        true,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-limited.heic"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-limited.libheif-bilinear-rgb"),
        1,
        false,
        false,
    ),
    (
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-limited-alpha.heic"),
        include_bytes!(
            "../../../../tests/heif-color-fixtures/yuv420-limited-alpha.libheif-bilinear-rgba"
        ),
        1,
        false,
        true,
    ),
];
#[test]
fn explicit_color_container_and_native_planes_match_independent_encoder() {
    for (source, _, chroma, full_range, alpha) in FILES {
        let admitted = admit_srgb8(source, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(admitted.size(), (32, 24));
        assert_eq!(admitted.chroma(), chroma);
        assert_eq!(admitted.alpha(), alpha);
        assert_eq!(admitted.encoding().primaries, 1);
        assert_eq!(admitted.encoding().transfer, 13);
        assert_eq!(admitted.encoding().full_range, full_range);
        assert_eq!(admitted.source().as_ptr(), source.as_ptr());
        let image = heic_decoder::decode_with(
            source,
            &heic_decoder::DecodeOptions {
                apply_transforms: false,
            },
        )
        .unwrap();
        for (channel, plane) in [(1, &image.cb), (2, &image.cr)] {
            for y in 0..image.cb_height {
                for x in 0..image.cb_width {
                    let expected = if admitted.encoding().matrix == 0 {
                        (x * 7 + y * 11 + channel * 61) % 256
                    } else {
                        96 + ((x * 5 + y * 3 + channel * 11) % 64)
                    };
                    assert_eq!(
                        plane[y * image.cb_width + x],
                        expected as i32,
                        "chroma={chroma} channel={channel} x={x} y={y}"
                    );
                }
            }
        }
        for y in 0..24 {
            for x in 0..32 {
                let expected = if admitted.encoding().matrix == 0 {
                    (x * 7 + y * 11) % 256
                } else {
                    32 + ((x * 3 + y * 5) % 176)
                };
                assert_eq!(image.y[y * 32 + x], expected as i32);
                if let Some(alpha) = &image.alpha {
                    assert_eq!(
                        alpha.samples[y * 32 + x],
                        [0, 1, 127, 128, 254, 255][(x + y) % 6]
                    );
                }
            }
        }
    }
}
#[test]
fn selected_rgb_conversion_is_compared_to_independent_libheif() {
    for (source, reference, _, _, alpha) in FILES {
        let admitted = admit_srgb8(source, PIXELS_MAX, &|| Ok(())).unwrap();
        let image = heic_decoder::decode_with(
            source,
            &heic_decoder::DecodeOptions {
                apply_transforms: false,
            },
        )
        .unwrap();
        let rgb = image.to_rgba8().unwrap();
        let channels = if alpha { 4 } else { 3 };
        assert_eq!(rgb.data.len(), 32 * 24 * 4);
        assert_eq!(reference.len(), 32 * 24 * channels);
        let mut max_error = 0;
        for (pixel, expected) in rgb
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .zip(reference.chunks_exact(channels))
        {
            for c in 0..3 {
                max_error = max_error.max(pixel[c].abs_diff(expected[c]));
            }
            assert_eq!(pixel[3], if alpha { expected[3] } else { 255 });
        }
        eprintln!(
            "chroma={} matrix={} full={} alpha={} maximum RGB error={}",
            admitted.chroma(),
            admitted.encoding().matrix,
            admitted.encoding().full_range,
            alpha,
            max_error
        );
        if admitted.chroma() == 3 {
            assert!(
                max_error <= 1,
                "independent 4:4:4 RGB oracle mismatch {max_error}"
            );
        } else {
            // Preserve the incompatible libheif bilinear output as negative
            // evidence, not as the selected PaintCraft reconstruction policy.
            assert_eq!(
                max_error,
                if admitted.encoding().full_range {
                    98
                } else {
                    113
                }
            );
        }
    }
}
#[test]
fn color_conversion_matches_explicit_independent_ffmpeg_bilinear_policy() {
    let references: [&[u8]; 5] = [
        include_bytes!("../../../../tests/heif-color-fixtures/identity444.ffmpeg-rgb"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-full.ffmpeg-rgb"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-limited.ffmpeg-rgb"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-full.ffmpeg-rgb"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-limited.ffmpeg-rgb"),
    ];
    for (index, (source, _, _, _, _)) in FILES.iter().enumerate() {
        let image = heic_decoder::decode_with(
            source,
            &heic_decoder::DecodeOptions {
                apply_transforms: false,
            },
        )
        .unwrap();
        let converted = image.to_rgba8().unwrap();
        let max_error = converted
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .zip(references[index / 2].as_chunks::<3>().0.iter())
            .flat_map(|(actual, expected)| (0..3).map(move |c| actual[c].abs_diff(expected[c])))
            .max()
            .unwrap();
        assert!(
            max_error <= 1,
            "fixture{index}: explicit FFmpeg policy RGB error{max_error}"
        );
    }
}
#[test]
fn odd_444_geometry_and_alpha_are_checked_at_every_boundary_pixel() {
    let cases: [(&[u8], &[u8]); 3] = [
        (
            include_bytes!("../../../../tests/heif-color-fixtures/odd/identity444-alpha.heic"),
            include_bytes!("../../../../tests/heif-color-fixtures/odd/identity444.ffmpeg-rgb"),
        ),
        (
            include_bytes!("../../../../tests/heif-color-fixtures/odd/yuv444-full-alpha.heic"),
            include_bytes!("../../../../tests/heif-color-fixtures/odd/yuv444-full.ffmpeg-rgb"),
        ),
        (
            include_bytes!("../../../../tests/heif-color-fixtures/odd/yuv444-limited-alpha.heic"),
            include_bytes!("../../../../tests/heif-color-fixtures/odd/yuv444-limited.ffmpeg-rgb"),
        ),
    ];
    for (source, reference) in cases {
        let admitted = admit_srgb8(source, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(admitted.size(), (33, 25));
        let image = heic_decoder::decode_with(
            source,
            &heic_decoder::DecodeOptions {
                apply_transforms: false,
            },
        )
        .unwrap();
        let rgba = image.to_rgba8().unwrap();
        assert_eq!(rgba.data.len(), 33 * 25 * 4);
        assert_eq!(reference.len(), 33 * 25 * 3);
        for y in 0..25 {
            for x in 0..33 {
                let i = y * 33 + x;
                for c in 0..3 {
                    assert!(
                        rgba.data[i * 4 + c].abs_diff(reference[i * 3 + c]) <= 1,
                        "pixel{x},{y},channel{c}"
                    );
                }
                assert_eq!(
                    rgba.data[i * 4 + 3],
                    [0, 1, 127, 128, 254, 255][(x + y) % 6]
                );
            }
        }
    }
}
#[test]
fn malformed_extents_limits_cancellation_and_unqualified_color_fail_closed() {
    use std::cell::Cell;
    let source = FILES[0].0;
    assert_eq!(
        admit_srgb8(source, 767, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    for length in [0, 4, 8, 23, 100, source.len() - 1] {
        assert!(admit_srgb8(&source[..length], PIXELS_MAX, &|| Ok(())).is_err());
    }
    for until in 0..16 {
        let checks = Cell::new(0);
        assert_eq!(
            admit_srgb8(source, PIXELS_MAX, &|| {
                let count = checks.get();
                checks.set(count + 1);
                if count >= until {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
    let value = inspect_checked(source, PIXELS_MAX, &|| Ok(())).unwrap();
    let primary = value.primary().unwrap();
    let base = Encoding {
        primaries: 1,
        transfer: 13,
        matrix: 0,
        full_range: true,
    };
    for declaration in [
        Encoding {
            primaries: 2,
            ..base
        },
        Encoding {
            transfer: 2,
            ..base
        },
        Encoding {
            transfer: 16,
            ..base
        },
        Encoding { matrix: 2, ..base },
        Encoding {
            full_range: false,
            ..base
        },
    ] {
        let mut item = primary.clone();
        for association in &mut item.properties {
            if matches!(association.property, Property::Color(_)) {
                association.property = Property::Color(Color::Nclx {
                    primaries: declaration.primaries,
                    transfer: declaration.transfer,
                    matrix: declaration.matrix,
                    full_range: declaration.full_range,
                });
            }
        }
        assert!(profile(&item, Some(declaration)).is_err());
        assert!(profile(&item, Some(base)).is_err());
    }
    assert!(profile(primary, None).is_err());
}

#[test]
fn selected_rgb_matches_independent_exact_rational_policy() {
    let references: [&[u8]; 5] = [
        include_bytes!("../../../../tests/heif-color-fixtures/identity444.rational-rgb"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-full.rational-rgb"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv444-limited.rational-rgb"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-full.rational-rgb"),
        include_bytes!("../../../../tests/heif-color-fixtures/yuv420-limited.rational-rgb"),
    ];
    for (index, (source, _, _, _, _)) in FILES.iter().enumerate() {
        let image = heic_decoder::decode_with(
            source,
            &heic_decoder::DecodeOptions {
                apply_transforms: false,
            },
        )
        .unwrap();
        let converted = image.to_rgba8().unwrap();
        assert_eq!(converted.data.len(), 32 * 24 * 4);
        assert_eq!(references[index / 2].len(), 32 * 24 * 3);
        for (actual, expected) in converted
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .zip(references[index / 2].as_chunks::<3>().0.iter())
        {
            for channel in 0..3 {
                assert!(
                    actual[channel].abs_diff(expected[channel]) <= 1,
                    "fixture{index} channel{channel}"
                );
            }
        }
    }
}

#[test]
fn odd_420_is_refused_until_its_chroma_extent_has_independent_qualification() {
    for size in [(33, 24), (32, 25), (33, 25)] {
        assert_eq!(
            geometry(size, 1).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        geometry(size, 3).unwrap();
    }
    geometry((32, 24), 1).unwrap();
}

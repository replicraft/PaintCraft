use super::*;
use ::image::{DynamicImage, ImageDecoder};
use paintcraft_core::types::json;
use std::{cell::Cell, fs, path::PathBuf};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/jpeg-fixtures")
            .join(name),
    )
    .unwrap()
}
fn raw(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut decoder = ::image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(bytes)).unwrap();
    let dimensions = decoder.dimensions();
    // Reading metadata reports a transform; the decoder does not apply it.
    let _ = decoder.orientation().unwrap();
    let image = DynamicImage::from_decoder(decoder).unwrap().into_rgba8();
    (dimensions.0, dimensions.1, image.into_raw())
}
fn exif(value: u16, little: bool) -> Vec<u8> {
    let mut bytes = b"Exif\0\0".to_vec();
    bytes.extend(if little { *b"II" } else { *b"MM" });
    let word = |v: u16| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    let long = |v: u32| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    bytes.extend(word(42));
    bytes.extend(long(8));
    bytes.extend(word(1));
    bytes.extend(word(0x112));
    bytes.extend(word(3));
    bytes.extend(long(1));
    bytes.extend(word(value));
    bytes.extend(word(0));
    bytes.extend(long(0));
    bytes
}
fn insert(base: &[u8], marker: u8, data: &[u8]) -> Vec<u8> {
    let mut output = base[..2].to_vec();
    output.extend([255, marker]);
    output.extend(u16::try_from(data.len() + 2).unwrap().to_be_bytes());
    output.extend(data);
    output.extend(&base[2..]);
    output
}
#[test]
fn independent_libjpeg_samples_all_orientation_profiles_and_source_models() {
    let oracle = json::parse(&fixture("oracle.json")).unwrap();
    let cases = oracle.get("cases").unwrap().as_array().unwrap();
    assert_eq!(cases.len(), 54);
    for case in cases {
        let name = case.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        assert_eq!(
            hex(&Sha256::digest(&bytes)),
            case.get("sha256").unwrap().as_str().unwrap(),
            "{name}"
        );
        let source = inspect(&bytes, super::super::DECODE_PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(source.precision(), 8);
        assert_eq!(
            source.orientation(),
            case.get("orientation").unwrap().as_i64().unwrap() as u8,
            "{name}"
        );
        assert_eq!(source.source_bytes(), bytes.len() as u64);
        assert_eq!(
            source.source_sha256(),
            <[u8; 32]>::from(Sha256::digest(&bytes))
        );
        let expected_model = match case
            .get("original_jpeg_color_space")
            .unwrap()
            .as_i64()
            .unwrap()
        {
            1 => Model::Gray,
            2 => Model::Rgb,
            3 => Model::YCbCr,
            4 => Model::Cmyk {
                adobe_inverted: case.get("adobe").unwrap().as_bool().unwrap(),
            },
            5 => Model::Ycck,
            other => panic!("Unexpected independent component model {other}"),
        };
        assert_eq!(source.model(), expected_model, "{name}");
        let result = decode(&bytes, super::super::DECODE_PIXELS_MAX, &|| Ok(()))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(result.source, source);
        let assumed = matches!(
            source.model(),
            Model::Cmyk {
                adobe_inverted: false
            }
        );
        assert_eq!(
            result.losses.iter().any(|v| v == LOSS_MODEL_ASSUMED),
            assumed
        );
        assert_eq!(
            result.losses.iter().any(|v| v == LOSS_CMYK),
            matches!(source.model(), Model::Cmyk { .. } | Model::Ycck)
        );
        assert_eq!(
            (result.width, result.height),
            (
                case.get("expected_width").unwrap().as_i64().unwrap() as u32,
                case.get("expected_height").unwrap().as_i64().unwrap() as u32
            ),
            "{name}"
        );
        let expected = fixture(case.get("rgba_file").unwrap().as_str().unwrap());
        assert_eq!(
            hex(&Sha256::digest(&expected)),
            case.get("rgba_sha256").unwrap().as_str().unwrap()
        );
        if result.rgba != expected {
            let at = result
                .rgba
                .iter()
                .zip(&expected)
                .position(|(a, b)| a != b)
                .unwrap();
            panic!(
                "{name}: independent raw component mismatch at byte {at}: actual {}, expected {}; actualSHA={}, expectedSHA={}",
                result.rgba[at],
                expected[at],
                hex(&Sha256::digest(&result.rgba)),
                hex(&Sha256::digest(&expected))
            );
        }
        assert_eq!(result.normalized_orientation, 1);
        assert_eq!(result.identity_profile, IDENTITY_PROFILE);
        assert_eq!(fixture(name), bytes, "source remains immutable");
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}

#[test]
fn pinned_facade_baseline_does_not_encode_the_unmarked_cmyk_policy() {
    let bytes = fixture("unmarked-cmyk.jpg");
    let (_, _, legacy) = raw(&bytes);
    let expected = fixture("unmarked-cmyk.rgba");
    assert_ne!(
        legacy, expected,
        "requalify the adapter if the pinned facade changes"
    );
    assert_eq!(decode(&bytes, 384, &|| Ok(())).unwrap().rgba, expected);
}

#[test]
fn ancillary_projection_keeps_original_offsets_bytes_and_omission_facts() {
    let base = fixture("gray.jpg");
    let mut xmp = b"http://ns.adobe.com/xmp/extension/\0".to_vec();
    xmp.extend([b'0'; 32]);
    xmp.extend(u32::MAX.to_be_bytes());
    xmp.extend(0u32.to_be_bytes());
    xmp.extend(b"incomplete payload with \xff\xe1 fake marker");
    let mut icc = b"ICC_PROFILE\0".to_vec();
    icc.extend([1, 1, 8, 3, 1]);
    let original = insert(
        &insert(
            &insert(&insert(&base, 0xe1, &exif(6, true)), 0xe2, &icc),
            0xed,
            b"Photoshop 3.0\0\xff\xe1 payload",
        ),
        0xe1,
        &xmp,
    );
    let before = original.clone();
    let (source, projection) =
        inspect::projected_copy(&original, Limits::for_pixels(384), &|| Ok(())).unwrap();
    assert_eq!(projection.len(), original.len());
    let changed: Vec<_> = original
        .iter()
        .zip(&projection)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .collect();
    assert_eq!(changed.len(), 4);
    for (at, (from, to)) in changed {
        assert!(matches!(*from, 0xe1 | 0xe2 | 0xed));
        assert_eq!(*to, 0xfe);
        assert_eq!(original[at - 1], 255);
    }
    assert_eq!(
        source.source_sha256(),
        <[u8; 32]>::from(Sha256::digest(&original))
    );
    assert_eq!(source.orientation(), 6);
    assert_eq!(source.metadata().icc_segments, 1);
    assert!(source.metadata().xmp_bytes > 0);
    let result = decode(&original, 384, &|| Ok(())).unwrap();
    assert_eq!((result.width, result.height), (16, 24));
    assert_eq!(result.source, source);
    for code in [
        "icc-profile-ignored",
        "exif-metadata-ignored",
        LOSS_METADATA,
    ] {
        assert!(result.losses.iter().any(|v| v == code));
    }
    assert_eq!(original, before);
}

#[test]
fn all_eight_transforms_pin_non_square_exact_rgba_including_hidden_color() {
    let admitted = inspect(&fixture("rgb.jpg"), 384, &|| Ok(())).unwrap();
    let labels = *b"ABCDEF";
    let pixels: Vec<_> = labels
        .into_iter()
        .zip([0, 1, 127, 128, 254, 255])
        .flat_map(|(v, a)| [v, v ^ 91, v ^ 173, a])
        .collect();
    for (index, expected) in [
        "ABCDEF", "CBAFED", "FEDCBA", "DEFABC", "ADBECF", "DAEBFC", "FCEBDA", "CFBEAD",
    ]
    .into_iter()
    .enumerate()
    {
        let mut source = admitted.clone();
        // Private fixture seam isolates the mathematical pixel permutation from
        // the independent real JPEG decoder/metadata corpus above.
        source.width = 3;
        source.height = 2;
        source.orientation = index as u8 + 1;
        let output = materialize_orientation(&source, 3, 2, pixels.clone(), &|| Ok(())).unwrap();
        let expected: Vec<_> = expected
            .bytes()
            .flat_map(|label| {
                let at = usize::from(label - b'A') * 4;
                pixels[at..at + 4].iter().copied()
            })
            .collect();
        assert_eq!(output.rgba, expected);
        assert_eq!(
            (output.width, output.height),
            if index >= 4 { (2, 3) } else { (3, 2) }
        );
        assert_eq!(output.original_orientation, index as u8 + 1);
        assert_eq!(output.normalized_orientation, 1);
    }
}
#[test]
fn public_orientation_rejects_hostile_geometry_without_overflow_or_allocation() {
    let source = inspect(&fixture("rgb.jpg"), 384, &|| Ok(())).unwrap();
    for (width, height) in [(u32::MAX, u32::MAX), (u32::MAX, 1), (0, 0), (16, 24)] {
        assert!(materialize_orientation(&source, width, height, Vec::new(), &|| Ok(())).is_err());
    }
    assert!(materialize_orientation(&source, 24, 16, vec![0; 1535], &|| Ok(())).is_err());
}
#[test]
fn malformed_exif_and_conflicting_color_models_refuse_before_decoding() {
    let base = fixture("rgb.jpg");
    for little in [true, false] {
        for value in [0, 9, u16::MAX] {
            assert!(inspect(&insert(&base, 0xe1, &exif(value, little)), 384, &|| Ok(())).is_err());
        }
        let one = insert(&base, 0xe1, &exif(6, little));
        assert!(inspect(&insert(&one, 0xe1, &exif(6, little)), 384, &|| Ok(())).is_err());
    }
    let mut outside = exif(1, true);
    outside[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(inspect(&insert(&base, 0xe1, &outside), 384, &|| Ok(())).is_err());
    let mut adobe = b"Adobe\0d\0\0\0\0\x01".to_vec();
    // RGB component IDs disagree with Adobe YCbCr; arbitrary two-channel and
    // transform-2/three-channel tolerance must not fabricate a source model.
    for value in [1, 2, 3] {
        adobe[11] = value;
        assert!(inspect(&insert(&base, 0xee, &adobe), 384, &|| Ok(())).is_err());
    }
    for end in 0..base.len() {
        assert!(
            inspect(&base[..end], 384, &|| Ok(())).is_err(),
            "truncated at{end}"
        );
    }
}
#[test]
fn caller_limits_icc_sequence_metadata_omissions_and_cancel_are_explicit() {
    let base = fixture("gray.jpg");
    let limits = Limits {
        input_bytes: base.len() as u64 - 1,
        pixels: 384,
        metadata_bytes: INPUT_MAX,
    };
    assert_eq!(
        inspect_with_limits(&base, limits, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        inspect(&base, 383, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let comment = insert(&base, 0xfe, b"authored comment");
    let source = inspect(&comment, 384, &|| Ok(())).unwrap();
    assert_eq!(source.losses(), [LOSS_METADATA]);
    assert_eq!(source.metadata().comment_segments, 1);
    let limits = Limits {
        input_bytes: INPUT_MAX,
        pixels: 384,
        metadata_bytes: 0,
    };
    assert!(inspect_with_limits(&comment, limits, &|| Ok(())).is_err());
    let chunk = |index, count| {
        let mut bytes = b"ICC_PROFILE\0".to_vec();
        bytes.extend([index, count]);
        bytes.extend([8, 3, 1]);
        bytes
    };
    let incomplete = insert(&base, 0xe2, &chunk(1, 2));
    assert!(inspect(&incomplete, 384, &|| Ok(())).is_err());
    assert!(inspect(&insert(&incomplete, 0xe2, &chunk(1, 2)), 384, &|| Ok(())).is_err());
    let complete = insert(&incomplete, 0xe2, &chunk(2, 2));
    let source = inspect(&complete, 384, &|| Ok(())).unwrap();
    assert_eq!(source.metadata().icc_segments, 2);
    assert_eq!(source.metadata().icc_bytes, 6);
    assert!(source.losses().contains(&"icc-profile-ignored".to_owned()));
    let before = complete.clone();
    let calls = Cell::new(0);
    inspect(&complete, 384, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let result = inspect(&complete, 384, &|| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "inspect cancel"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
    assert_eq!(complete, before);
    let mut source = inspect(&base, 384, &|| Ok(())).unwrap();
    source.orientation = 6;
    let input = vec![191; 24 * 16 * 4];
    let before = input.clone();
    let calls = Cell::new(0);
    materialize_orientation(&source, 24, 16, input.clone(), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let result = materialize_orientation(&source, 24, 16, input.clone(), &|| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "orientation cancel",
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
    assert_eq!(input, before);
}

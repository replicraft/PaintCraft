use super::*;
use std::cell::Cell;

pub(super) const FIXTURES: [[&[u8]; 8]; 2] = [
    [
        include_bytes!("../../tests/psd-fixtures/layers-0.psd"),
        include_bytes!("../../tests/psd-fixtures/layers-1.psd"),
        include_bytes!("../../tests/psd-fixtures/layers-2.psd"),
        include_bytes!("../../tests/psd-fixtures/layers-3.psd"),
        include_bytes!("../../tests/psd-fixtures/composite-0.psd"),
        include_bytes!("../../tests/psd-fixtures/composite-1.psd"),
        include_bytes!("../../tests/psd-fixtures/composite-2.psd"),
        include_bytes!("../../tests/psd-fixtures/composite-3.psd"),
    ],
    [
        include_bytes!("../../tests/psd-fixtures/layers-0.psb"),
        include_bytes!("../../tests/psd-fixtures/layers-1.psb"),
        include_bytes!("../../tests/psd-fixtures/layers-2.psb"),
        include_bytes!("../../tests/psd-fixtures/layers-3.psb"),
        include_bytes!("../../tests/psd-fixtures/composite-0.psb"),
        include_bytes!("../../tests/psd-fixtures/composite-1.psb"),
        include_bytes!("../../tests/psd-fixtures/composite-2.psb"),
        include_bytes!("../../tests/psd-fixtures/composite-3.psb"),
    ],
];

#[test]
fn selected_backend_pixels_match_the_independent_oracle() {
    for fixtures in FIXTURES {
        for (index, bytes) in fixtures.into_iter().enumerate() {
            let decoded = decode(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
            assert_eq!((decoded.width, decoded.height), (5, 3));
            if index < 4 {
                assert_eq!(decoded.layers.len(), 2);
                for (layer, seed) in decoded.layers.iter().zip([9usize, 51]) {
                    let count = layer.info.width as usize * layer.info.height as usize * 4;
                    let expected: Vec<_> = (0..count)
                        .map(|i| ((seed + i * 29 + (i / 4) * 7) % 256) as u8)
                        .collect();
                    assert_eq!(layer.rgba, expected);
                }
                assert_eq!(decoded.layers[0].info.origin, (-1, 1));
                assert_eq!(
                    (
                        decoded.layers[0].info.opacity,
                        decoded.layers[0].info.visible
                    ),
                    (127, false)
                );
            } else {
                assert_eq!(decoded.layers.len(), 1);
                let expected: Vec<_> = (0..15)
                    .flat_map(|pixel| {
                        (0..4).map(move |channel| {
                            if channel == 3 {
                                255
                            } else {
                                ((channel * 15 + pixel) * 13 + 17) as u8
                            }
                        })
                    })
                    .collect();
                assert_eq!(decoded.layers[0].rgba, expected);
            }
        }
    }
}

#[test]
fn independently_verified_raw_rle_zip_and_prediction_psd_and_psb() {
    for (version, fixtures) in FIXTURES.iter().enumerate() {
        for (index, bytes) in fixtures.iter().enumerate() {
            let info = inspect(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
            assert_eq!(
                info.version,
                if version == 0 {
                    Version::Psd
                } else {
                    Version::Psb
                }
            );
            assert_eq!((info.width, info.height), (5, 3));
            assert_eq!(info.losses, ["psd-metadata-ignored"]);
            assert_eq!(info.layers.len(), if index < 4 { 2 } else { 0 });
            if index < 4 {
                let first = &info.layers[0];
                assert_eq!(
                    (first.name.as_str(), first.origin, first.width, first.height),
                    ("日本語 duplicate", (-1, 1), 3, 2)
                );
                assert_eq!(
                    (first.opacity, first.visible, first.source_id),
                    (127, false, Some(71))
                );
                assert_eq!(
                    (
                        info.layers[1].opacity,
                        info.layers[1].visible,
                        info.layers[1].source_id
                    ),
                    (255, true, Some(72))
                );
            }
        }
    }
}

#[test]
fn every_truncated_prefix_and_undeclared_composite_suffix_is_refused() {
    for fixtures in FIXTURES {
        for bytes in fixtures {
            for end in 0..bytes.len() {
                assert!(
                    inspect(&bytes[..end], PIXELS_MAX, &|| Ok(())).is_err(),
                    "accepted truncation {end}"
                );
            }
            let mut tailed = bytes.to_vec();
            tailed.push(0);
            assert!(inspect(&tailed, PIXELS_MAX, &|| Ok(())).is_err());
        }
    }
}

#[test]
fn exact_expansion_limits_cover_large_streams_and_zip_bombs() {
    use std::io::Write;
    for size in [0usize, 1, 65536, 65537, 131073] {
        let mut writer =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        writer.write_all(&vec![17; size]).unwrap();
        let mut payload = vec![0, 2];
        payload.extend(writer.finish().unwrap());
        channels::admit(&payload, size as u32, 1, false, &|| Ok(())).unwrap();
        if size > 0 {
            assert!(channels::admit(&payload, (size - 1) as u32, 1, false, &|| Ok(())).is_err());
        }
        assert!(channels::admit(&payload, (size + 1) as u32, 1, false, &|| Ok(())).is_err());
        payload.push(0);
        assert!(channels::admit(&payload, size as u32, 1, false, &|| Ok(())).is_err());
    }
}

#[test]
fn packbits_rows_cannot_spill_or_borrow_from_the_next_row() {
    // 128 no-op, -3 repeats, two literals: exactly six samples.
    let data = [0, 1, 0, 6, 128, 253, 19, 1, 11, 12];
    channels::admit(&data, 6, 1, false, &|| Ok(())).unwrap();
    assert!(channels::admit(&data, 5, 1, false, &|| Ok(())).is_err());
    assert!(channels::admit(&data, 7, 1, false, &|| Ok(())).is_err());
    let mut broken = data;
    broken[3] = 5;
    assert!(channels::admit(&broken, 6, 1, false, &|| Ok(())).is_err());
}

#[test]
fn budgets_and_cancellation_apply_before_decode() {
    let bytes = FIXTURES[0][0];
    assert!(inspect(bytes, 14, &|| Ok(())).is_err()); // Canvas exceeds budget.
    assert!(inspect(bytes, 20, &|| Ok(())).is_err()); // 21 aggregate layer pixels.
    let calls = Cell::new(0);
    let error = inspect(bytes, PIXELS_MAX, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 5 {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel test"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(calls.get(), 5);
}

#[test]
fn unrepresentable_or_ambiguous_layer_semantics_are_refused() {
    let original = FIXTURES[0][0];
    for (needle, replacement) in [
        (b"8BIMnorm".as_slice(), b"8BIMmul ".as_slice()),
        (b"luni", b"TySh"),
        (b"lyid", b"lsct"),
    ] {
        let mut bytes = original.to_vec();
        let at = bytes
            .windows(needle.len())
            .position(|part| part == needle)
            .unwrap();
        bytes[at..at + needle.len()].copy_from_slice(replacement);
        assert!(inspect(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let mut bytes = original.to_vec();
    let at = bytes.windows(4).position(|part| part == b"iOpa").unwrap();
    bytes[at + 8] = 127;
    assert!(inspect(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    let mut bytes = original.to_vec();
    let at = bytes.windows(4).position(|part| part == b"luni").unwrap();
    bytes[at + 8..at + 12].copy_from_slice(&1025u32.to_be_bytes());
    assert!(inspect(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
    let mut bytes = original.to_vec();
    bytes[22..24].copy_from_slice(&16u16.to_be_bytes());
    assert!(inspect(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
}

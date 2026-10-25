use paintcraft_core::types::ErrorCode;
use paintcraft_formats::ora::{Limits, decode_layers};
use sha2::{Digest, Sha256};
use std::cell::Cell;

const STORED: &[u8] = include_bytes!("ora-fixtures/stored.ora");
const DEFLATED: &[u8] = include_bytes!("ora-fixtures/deflated.ora");

#[test]
fn independent_zip_png_layers_keep_order_names_offsets_visibility_and_hidden_rgb() {
    for bytes in [STORED, DEFLATED] {
        let decoded = decode_layers(bytes, &Limits::default(), &|| Ok(())).unwrap();
        assert_eq!((decoded.width, decoded.height), (3, 2));
        assert_eq!(decoded.resolution, Some((144, 144)));
        assert_eq!(decoded.losses, ["ora-resolution-ignored"]);
        assert_eq!(decoded.layers.len(), 3);
        let expected = [
            (
                "Background",
                (0, 0),
                true,
                "673f579fd4558390ed2fbc35f6674e76f43488794a0571a97e9278d62295dda4",
            ),
            (
                "日本語 & paint",
                (1, 1),
                true,
                "fe2b84ccfb2a708b7f78ff0fe4c860042e12f4189feb69b36070fad8a0d49737",
            ),
            (
                "Hidden",
                (-1, 0),
                false,
                "7a7bf454c5f3cb1b9d9a20f81417f98d976fe3b3dd52c1b9968f02e89e7e8a2f",
            ),
        ];
        for (layer, (name, origin, visible, hash)) in decoded.layers.iter().zip(expected) {
            assert_eq!(layer.name, name);
            assert_eq!(layer.origin, origin);
            assert_eq!(layer.visible, visible);
            assert_eq!(layer.opacity, 255);
            assert_eq!(paintcraft_document::hex(&Sha256::digest(&layer.rgba)), hash);
        }
    }
}

#[test]
fn source_entry_aggregate_xml_and_pixel_limits_are_enforced() {
    let cases = [
        Limits {
            archive_bytes: 1,
            ..Limits::default()
        },
        Limits {
            entries: 2,
            ..Limits::default()
        },
        Limits {
            inflated_bytes: 32,
            ..Limits::default()
        },
        Limits {
            entry_bytes: 32,
            ..Limits::default()
        },
        Limits {
            xml_bytes: 8,
            ..Limits::default()
        },
        Limits {
            metadata_bytes: 8,
            ..Limits::default()
        },
        Limits {
            attribute_bytes: 2,
            ..Limits::default()
        },
        Limits {
            attributes: 1,
            ..Limits::default()
        },
        Limits {
            depth: 2,
            ..Limits::default()
        },
        Limits {
            path_bytes: 2,
            ..Limits::default()
        },
        Limits {
            layers: 2,
            ..Limits::default()
        },
        Limits {
            dimension: 2,
            ..Limits::default()
        },
        Limits {
            pixels: 7,
            ..Limits::default()
        },
        Limits {
            decoded_bytes: 28,
            ..Limits::default()
        },
    ];
    for limits in cases {
        for source in [STORED, DEFLATED] {
            assert!(
                decode_layers(source, &limits, &|| Ok(())).is_err(),
                "{limits:?}"
            );
        }
    }
}

#[test]
fn every_truncated_prefix_is_refused_and_cancellation_returns_no_partial_stack() {
    for source in [STORED, DEFLATED] {
        for end in 0..source.len() {
            assert!(decode_layers(&source[..end], &Limits::default(), &|| Ok(())).is_err());
        }
        let calls = Cell::new(0);
        decode_layers(source, &Limits::default(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=calls.get() {
            let current = Cell::new(0);
            let error = decode_layers(source, &Limits::default(), &|| {
                current.set(current.get() + 1);
                if current.get() == stop {
                    Err(paintcraft_core::types::Error::new(
                        ErrorCode::OperationCancelled,
                        "test cancellation",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
        }
    }
}

#[test]
fn corrupt_crc_local_central_disagreement_traversal_and_symlinks_are_refused() {
    let mut crc = STORED.to_vec();
    let png = crc
        .windows(8)
        .position(|w| w == b"\x89PNG\r\n\x1a\n")
        .unwrap();
    crc[png + 12] ^= 1;
    assert!(decode_layers(&crc, &Limits::default(), &|| Ok(())).is_err());
    let mut mismatch = STORED.to_vec();
    mismatch[8] = 8; // Local compression method disagrees with directory.
    assert!(decode_layers(&mismatch, &Limits::default(), &|| Ok(())).is_err());
    let mut traversal = STORED.to_vec();
    for p in 0..traversal.len() - 12 {
        if &traversal[p..p + 12] == b"data/top.png" {
            traversal[p..p + 12].copy_from_slice(b"../a/top.png");
        }
    }
    assert!(decode_layers(&traversal, &Limits::default(), &|| Ok(())).is_err());
    let mut symlink = STORED.to_vec();
    let central = symlink.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    symlink[central + 38..central + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
    assert!(decode_layers(&symlink, &Limits::default(), &|| Ok(())).is_err());
}

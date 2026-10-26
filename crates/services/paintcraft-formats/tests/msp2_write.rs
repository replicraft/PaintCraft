use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::msp;
use std::{cell::Cell, io::Write};
fn vectors() -> Vec<(&'static str, u32, u32, Vec<u8>)> {
    let mut runs = vec![0; 256];
    runs.extend([255; 256]);
    runs.extend(0u8..=255);
    vec![
        ("tail-1", 1, 2, vec![0x80, 0x7f]),
        ("tail-9", 9, 3, vec![0x80, 0x81, 0xff, 0xfe, 0x55, 0xaa]),
        ("runs-2048", 2048, 3, runs),
        (
            "wide-65535",
            65535,
            1,
            (0..8192).map(|i| (i % 251) as u8).collect(),
        ),
    ]
}
#[test]
fn writer_matches_independent_oracle_vectors_and_optional_output_is_create_new() {
    let mut qualified = Vec::new();
    for (name, width, height, packed) in vectors() {
        let encoded = msp::encode_v2(width, height, &packed, &|| Ok(())).unwrap();
        let plan = msp::preflight_v2(width, height, &packed, &|| Ok(())).unwrap();
        assert_eq!(encoded.len(), plan.encoded_bytes);
        assert_eq!(plan.transient_bytes, packed.len() + encoded.len());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/msp2-writer-fixtures")
            .join(format!("{name}.msp"));
        if let Some(folder) = std::env::var_os("PAINTCRAFT_MSP2_WRITER_OUTPUT") {
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(std::path::Path::new(&folder).join(format!("{name}.msp")))
                .unwrap();
            file.write_all(&encoded).unwrap();
        }
        qualified.push((name, path, encoded.clone()));
        assert_eq!(&encoded[..4], b"LinS");
        assert_eq!(msp::decode(&encoded, &|| Ok(())).unwrap().packed(), packed);
        for row in encoded[32..32 + height as usize * 2].as_chunks::<2>().0 {
            assert_ne!(u16::from_le_bytes(*row), 0);
        }
        for end in [0, 31, 32, 32 + height as usize * 2, encoded.len() - 1] {
            assert!(msp::decode(&encoded[..end], &|| Ok(())).is_err());
        }
    }
    for (name, path, encoded) in qualified {
        assert_eq!(
            encoded,
            std::fs::read(path).unwrap(),
            "qualified {name} drift"
        );
    }
}
#[test]
fn repeat_literal_boundaries_and_extreme_tall_map_preserve_every_packed_byte() {
    for len in [1, 2, 3, 254, 255, 256, 257, 510, 511] {
        for value in [0, 255, 17] {
            let packed = vec![value; len];
            let bytes = msp::encode_v2((len * 8) as u32, 1, &packed, &|| Ok(())).unwrap();
            assert_eq!(msp::decode(&bytes, &|| Ok(())).unwrap().packed(), packed);
            if len == 255 {
                assert_eq!(&bytes[34..], &[0, 255, value]);
            }
            if len == 256 {
                assert_eq!(&bytes[34..], &[0, 255, value, 1, value]);
            }
        }
    }
    let packed = vec![0x80; 65535];
    let bytes = msp::encode_v2(1, 65535, &packed, &|| Ok(())).unwrap();
    assert_eq!(bytes.len(), 32 + 65535 * 4);
    assert_eq!(msp::decode(&bytes, &|| Ok(())).unwrap().packed(), packed);
}
#[test]
fn malformed_geometry_rgba_and_cancellation_never_mutate_source_or_return_output() {
    for (w, h) in [(0, 1), (1, 0), (65536, 1), (1, 65536), (65535, 65535)] {
        assert!(msp::encode_v2(w, h, &[], &|| Ok(())).is_err());
    }
    assert!(msp::encode_v2(9, 1, &[0], &|| Ok(())).is_err());
    for pixel in [
        [17, 17, 17, 255],
        [0, 0, 0, 0],
        [255, 255, 255, 254],
        [0, 1, 0, 255],
    ] {
        assert_eq!(
            msp::encode_v2_rgba_exact(1, 1, &pixel, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let rgba = [0, 0, 0, 255, 255, 255, 255, 255];
    let exact = msp::encode_v2_rgba_exact(2, 1, &rgba, &|| Ok(())).unwrap();
    assert_eq!(msp::decode(&exact, &|| Ok(())).unwrap().packed(), [0x40]);
    let packed: Vec<_> = (0..8192).map(|i| (i % 251) as u8).collect();
    let original = packed.clone();
    let count = Cell::new(0);
    msp::encode_v2(65535, 1, &packed, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let result = msp::encode_v2(65535, 1, &packed, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(packed, original);
    }
    let rgba = [0, 0, 0, 255, 255, 255, 255, 255].repeat(257);
    let count = Cell::new(0);
    msp::encode_v2_rgba_exact(514, 1, &rgba, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let result = msp::encode_v2_rgba_exact(514, 1, &rgba, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancelled RGBA"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
}

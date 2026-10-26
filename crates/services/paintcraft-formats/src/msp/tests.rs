use super::*;
use sha2::{Digest, Sha256};
use std::cell::Cell;
const RAW: &[u8] = include_bytes!("../../tests/msp-fixtures/raw-9.msp");
const RLE: &[u8] = include_bytes!("../../tests/msp-fixtures/rle-9.msp");
fn fixtures() -> [(&'static str, &'static [u8], u32, u32, &'static str); 7] {
    [
        (
            "raw-1.msp",
            include_bytes!("../../tests/msp-fixtures/raw-1.msp"),
            1,
            2,
            "c9fa5b8296e4465091d7112f785f5909b7201afda9b98c3fffae87244919f114",
        ),
        (
            "raw-7.msp",
            include_bytes!("../../tests/msp-fixtures/raw-7.msp"),
            7,
            3,
            "b00ebb09731d1b9215b630d467896aab4604d3d6c1647f8ff91d9ea06e209590",
        ),
        (
            "raw-9.msp",
            RAW,
            9,
            3,
            "bbcd6d33a2b5739de948fdb8185fdbd1fb1f85f0ced33db3d85e2f3094c1066f",
        ),
        (
            "raw-257.msp",
            include_bytes!("../../tests/msp-fixtures/raw-257.msp"),
            257,
            2,
            "0e124490f73c165b86334a50ffbf455bbc4f115b2e7302c32468ded74c7f5074",
        ),
        (
            "rle-9.msp",
            RLE,
            9,
            3,
            "bbcd6d33a2b5739de948fdb8185fdbd1fb1f85f0ced33db3d85e2f3094c1066f",
        ),
        (
            "rle-2048.msp",
            include_bytes!("../../tests/msp-fixtures/rle-2048.msp"),
            2048,
            2,
            "696c0599f3365581513272e542c4bc366cd4b01013ec4ef8d44991b563430d67",
        ),
        (
            "pillow-7.msp",
            include_bytes!("../../tests/msp-fixtures/pillow-7.msp"),
            7,
            3,
            "b00ebb09731d1b9215b630d467896aab4604d3d6c1647f8ff91d9ea06e209590",
        ),
    ]
}
fn checksum(bytes: &mut [u8]) {
    bytes[24..26].fill(0);
    let sum = bytes[..32]
        .as_chunks::<2>()
        .0
        .iter()
        .fold(0u16, |sum, word| sum ^ u16::from_le_bytes(*word));
    bytes[24..26].copy_from_slice(&sum.to_le_bytes());
}
#[test]
fn independent_specimens_preserve_packed_rows_tail_bits_metadata_and_opaque_pixel_polarity() {
    for (_, bytes, width, height, digest) in fixtures() {
        let bitmap = decode(bytes, &|| Ok(())).unwrap();
        assert_eq!(
            (bitmap.header().width(), bitmap.header().height()),
            (width, height)
        );
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(bitmap.rgba(&|| Ok(())).unwrap())),
            digest
        );
    }
    let bitmap = decode(RAW, &|| Ok(())).unwrap();
    assert_eq!(bitmap.packed(), [0x80, 0x80, 0xff, 0xff, 0x55, 0x55]);
    assert_eq!(
        &bitmap.header().original_words()[4..12],
        &[2, 3, 5, 7, 640, 480, 0x1234, 0x5678]
    );
    assert_eq!(bitmap.header().version(), Version::UncompressedV1);
    let rle = decode(RLE, &|| Ok(())).unwrap();
    assert_eq!(rle.packed(), bitmap.packed());
    assert_eq!(rle.header().version(), Version::RowRleV2);
    let expected = ["100000001", "111111111", "010101010"]
        .concat()
        .bytes()
        .flat_map(|bit| {
            if bit == b'1' {
                [255; 4]
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(bitmap.rgba(&|| Ok(())).unwrap(), expected);
}
#[test]
fn fresh_v1_writers_keep_exact_pixels_and_expose_bytes_to_an_independent_optional_reader() {
    for (name, bytes, width, height, _) in fixtures() {
        let bitmap = decode(bytes, &|| Ok(())).unwrap();
        let original = bitmap.clone();
        let encoded = encode_v1(width, height, bitmap.packed(), &|| Ok(())).unwrap();
        assert_eq!(&encoded[..4], b"DanM");
        assert_eq!(&encoded[32..], bitmap.packed());
        let reread = decode(&encoded, &|| Ok(())).unwrap();
        assert_eq!(reread.packed(), bitmap.packed());
        assert_eq!(
            &reread.header().original_words()[4..10],
            &[1, 1, 1, 1, width as u16, height as u16]
        );
        let rgba = bitmap.rgba(&|| Ok(())).unwrap();
        assert_eq!(
            decode(
                &encode_v1_rgba_exact(width, height, &rgba, &|| Ok(())).unwrap(),
                &|| Ok(())
            )
            .unwrap()
            .rgba(&|| Ok(()))
            .unwrap(),
            rgba
        );
        assert_eq!(bitmap, original);
        if let Some(path) = std::env::var_os("PAINTCRAFT_MSP_ORACLE_DIR") {
            let path = std::path::Path::new(&path);
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(path.join(name), encoded).unwrap();
        }
    }
}
#[test]
fn complete_extents_checksums_geometry_and_encoded_allowances_are_checked_before_decode() {
    for bytes in [RAW, RLE] {
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], &|| Ok(())).is_err(), "prefix {end}");
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(decode(&trailing, &|| Ok(())).is_err());
    }
    let mut bad = RAW.to_vec();
    bad[12] ^= 1;
    assert!(inspect(&bad, &|| Ok(())).is_err());
    for (width, height, code) in [
        (0, 3, ErrorCode::CodecFailed),
        (9, 0, ErrorCode::CodecFailed),
        (65535, 65535, ErrorCode::ResourceLimit),
    ] {
        let mut bad = RAW.to_vec();
        bad[4..6].copy_from_slice(&(width as u16).to_le_bytes());
        bad[6..8].copy_from_slice(&(height as u16).to_le_bytes());
        checksum(&mut bad);
        assert_eq!(inspect(&bad, &|| Ok(())).unwrap_err().code, code);
    }
    let mut huge = vec![0; BYTES_MAX + 1];
    huge[..RAW.len()].copy_from_slice(RAW);
    assert_eq!(
        inspect(&huge, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn rle_rows_never_read_past_map_packets_or_exact_output_and_ambiguous_empty_rows_are_refused() {
    fn file(map: &[u16], payload: &[u8]) -> Vec<u8> {
        let mut result = RLE[..32].to_vec();
        result[4..6].copy_from_slice(&9u16.to_le_bytes());
        result[6..8].copy_from_slice(&(map.len() as u16).to_le_bytes());
        checksum(&mut result);
        for word in map {
            result.extend(word.to_le_bytes());
        }
        result.extend(payload);
        result
    }
    for (map, payload) in [
        (&[1][..], &[0][..]),
        (&[2], &[0, 1]),
        (&[3], &[0, 3, 255]),
        (&[3], &[0, 1, 255]),
        (&[3], &[0, 0, 255]),
        (&[2], &[2, 255]),
        (&[1], &[3]),
        (&[65535], &[2, 0, 0]),
        (&[3, 3], &[2, 0, 0]),
    ] {
        assert!(
            decode(&file(map, payload), &|| Ok(())).is_err(),
            "{map:?} {payload:?}"
        );
    }
    assert_eq!(
        decode(&file(&[0], &[]), &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    // Independent lengths consume consecutive rows, not unchecked offsets.
    assert_eq!(
        decode(&file(&[3, 3], &[2, 0, 255, 0, 2, 128]), &|| Ok(()))
            .unwrap()
            .packed(),
        [0, 255, 128, 128]
    );
}
#[test]
fn no_implicit_color_threshold_alpha_matte_or_extent_truncation_is_permitted() {
    for pixel in [
        [255, 255, 255, 254],
        [0, 0, 0, 0],
        [255, 0, 0, 255],
        [128, 128, 128, 255],
    ] {
        assert_eq!(
            encode_v1_rgba_exact(1, 1, &pixel, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert!(encode_v1(9, 1, &[0], &|| Ok(())).is_err());
    assert!(encode_v1_rgba_exact(1, 1, &[0; 3], &|| Ok(())).is_err());
    assert!(encode_v1(65536, 1, &[], &|| Ok(())).is_err());
    assert_eq!(
        &encode_v1_rgba_exact(1, 2, &[255, 255, 255, 255, 0, 0, 0, 255], &|| Ok(())).unwrap()[32..],
        &[128, 0]
    );
}
fn cancellation<T: std::fmt::Debug>(run: impl Fn(&dyn Fn() -> Result<()>) -> Result<T>) {
    let calls = Cell::new(0);
    drop(
        run(&|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let result = run(&|| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
}
#[test]
fn every_exposed_reader_expansion_and_writer_checkpoint_cancels_without_a_partial_result() {
    for source in [RAW, RLE] {
        let original = source.to_vec();
        cancellation(|check| inspect(source, check));
        cancellation(|check| decode(source, check));
        let bitmap = decode(source, &|| Ok(())).unwrap();
        cancellation(|check| bitmap.rgba(check));
        cancellation(|check| {
            encode_v1(
                bitmap.header().width(),
                bitmap.header().height(),
                bitmap.packed(),
                check,
            )
        });
        let rgba = bitmap.rgba(&|| Ok(())).unwrap();
        cancellation(|check| {
            encode_v1_rgba_exact(
                bitmap.header().width(),
                bitmap.header().height(),
                &rgba,
                check,
            )
        });
        assert_eq!(source, original);
    }
}
#[test]
fn deterministic_mutations_of_both_versions_do_not_panic() {
    for original in [RAW, RLE] {
        for index in 0..original.len() * 8 {
            let mut bytes = original.to_vec();
            bytes[index / 8] ^= 1 << (index % 8);
            let result = std::panic::catch_unwind(|| decode(&bytes, &|| Ok(())));
            assert!(result.is_ok());
        }
    }
}

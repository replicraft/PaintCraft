use super::*;
use std::{cell::Cell, fs, path::PathBuf};
fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/jpeg-fixtures")
            .join(name),
    )
    .unwrap()
}
#[test]
fn whole_decode_cancels_at_every_observed_checkpoint_and_preserves_source() {
    for name in [
        "rgb-orientation-6-II.jpg",
        "unmarked-cmyk-orientation-6-II.jpg",
        "progressive-orientation-7-MM.jpg",
    ] {
        let bytes = fixture(name);
        let before = bytes.clone();
        let calls = Cell::new(0);
        let decoded = decode(&bytes, 384, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            decoded.source.source_sha256(),
            <[u8; 32]>::from(Sha256::digest(&bytes))
        );
        for stop in 1..=calls.get() {
            let seen = Cell::new(0);
            let output = decode(&bytes, 384, &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "whole JPEG cancel",
                    ))
                } else {
                    Ok(())
                }
            });
            assert_eq!(
                output.unwrap_err().code,
                ErrorCode::OperationCancelled,
                "{name}, checkpoint{stop}"
            );
            assert_eq!(bytes, before);
        }
    }
}
#[test]
fn whole_decode_obeys_caller_budgets_before_constructing_the_decoder() {
    let bytes = fixture("rgb.jpg");
    for limits in [
        Limits {
            input_bytes: bytes.len() as u64 - 1,
            pixels: 384,
            metadata_bytes: INPUT_MAX,
        },
        Limits {
            input_bytes: INPUT_MAX,
            pixels: 383,
            metadata_bytes: INPUT_MAX,
        },
        Limits {
            input_bytes: INPUT_MAX,
            pixels: 384,
            metadata_bytes: 0,
        },
    ] {
        assert_eq!(
            decode_with_limits(&bytes, limits, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    let mut impossible = bytes.clone();
    let at = impossible
        .windows(2)
        .position(|v| v == [255, 0xc0])
        .unwrap();
    impossible[at + 5..at + 9].copy_from_slice(&[255, 255, 255, 255]);
    assert_eq!(
        decode(&impossible, super::super::DECODE_PIXELS_MAX, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn fresh_export_has_normalized_geometry_and_no_second_exif_rotation() {
    let decoded = decode(&fixture("rgb-orientation-6-II.jpg"), 384, &|| Ok(())).unwrap();
    assert_eq!((decoded.width, decoded.height), (16, 24));
    let encoded = super::super::encode(
        super::super::ImageFormat::Jpeg,
        decoded.width,
        decoded.height,
        &decoded.rgba,
        100,
    )
    .unwrap();
    let reopened = decode(&encoded, 384, &|| Ok(())).unwrap();
    assert_eq!((reopened.width, reopened.height), (16, 24));
    assert_eq!(reopened.source.orientation(), 1);
    assert_eq!(reopened.source.metadata().exif_bytes, 0);
    assert_eq!(reopened.normalized_orientation, 1);
}
#[test]
fn fill_marker_and_stuffed_entropy_walk_have_bounded_cancel_checkpoints() {
    let base = fixture("gray.jpg");
    let mut padded = base[..2].to_vec();
    padded.extend(std::iter::repeat_n(255, 196_608));
    padded.extend(&base[2..]);
    let seen = Cell::new(0);
    let result = inspect(&padded, 384, &|| {
        seen.set(seen.get() + 1);
        if seen.get() == 3 {
            Err(Error::new(ErrorCode::OperationCancelled, "padding cancel"))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    assert_eq!(seen.get(), 3);
    let at = base.windows(2).position(|v| v == [255, 0xda]).unwrap();
    let end = at + 2 + usize::from(u16::from_be_bytes([base[at + 2], base[at + 3]]));
    let mut stuffed = base[..end].to_vec();
    let fake = [255, 0, 0xe1, 0, 2, 255, 0xd0, 17, 255, 0, 0xed, 0, 2];
    stuffed.extend(fake);
    stuffed.extend(&base[end..]);
    // The structural projection must not mistake stuffed/restart bytes for
    // ancillary markers. Entropy validity is separately checked by the codec.
    let (_, projected) =
        inspect::projected_copy(&stuffed, Limits::for_pixels(384), &|| Ok(())).unwrap();
    assert_eq!(&projected[end..end + fake.len()], &fake);
}

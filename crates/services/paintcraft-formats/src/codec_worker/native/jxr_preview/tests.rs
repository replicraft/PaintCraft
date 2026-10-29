//! Pure validation controls; positive runtime decoding is a separate installed gate.
use super::*;
use std::cell::Cell;
fn frame(rgba: &[u8], width: u32, height: u32) -> Frame<'_> {
    Frame {
        width,
        height,
        pixels: Pixels::SrgbRgba8(rgba),
        heap_peak: 0,
        float_report: None,
        jxl_report: None,
    }
}
fn rgb_hash(rgba: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in rgba.as_chunks::<4>().0 {
        h.update(&p[..3]);
    }
    h.finalize().into()
}
#[test]
fn exact_opaque_pixels_are_borrowed_and_wrong_geometry_alpha_model_and_seal_refuse() {
    let rgba = [7, 31, 211, 255, 91, 17, 3, 255];
    let layout = Layout::new(2, 1, 4096).unwrap();
    let expected = rgb_hash(&rgba);
    assert_eq!(
        validated_pixels(&frame(&rgba, 2, 1), layout, expected, &|| Ok(()))
            .unwrap()
            .as_ptr(),
        rgba.as_ptr()
    );
    for (w, h) in [(1, 2), (u32::MAX, u32::MAX)] {
        assert!(validated_pixels(&frame(&rgba, w, h), layout, expected, &|| Ok(())).is_err());
    }
    assert!(validated_pixels(&frame(&rgba[..4], 2, 1), layout, expected, &|| Ok(())).is_err());
    assert!(validated_pixels(&frame(&rgba, 2, 1), layout, [0; 32], &|| Ok(())).is_err());
    let mut transparent = rgba;
    transparent[3] = 254;
    assert!(validated_pixels(&frame(&transparent, 2, 1), layout, expected, &|| Ok(())).is_err());
    let luma = Frame {
        width: 2,
        height: 1,
        pixels: Pixels::NativeGray8 {
            gray: &[7, 31],
            alpha: None,
            encoding: NativeEncoding {
                primaries: 2,
                transfer: 2,
                matrix: 2,
                full_range: true,
            },
        },
        heap_peak: 0,
        float_report: None,
        jxl_report: None,
    };
    assert!(validated_pixels(&luma, layout, expected, &|| Ok(())).is_err());
}
#[test]
fn every_bounded_hash_checkpoint_cancels_without_modifying_samples() {
    let rgba = vec![255; 256 * 256 * 4];
    let original = Sha256::digest(&rgba);
    let layout = Layout::new(256, 256, 4096).unwrap();
    let expected = rgb_hash(&rgba);
    let calls = Cell::new(0);
    validated_pixels(&frame(&rgba, 256, 256), layout, expected, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(calls.get() > 4);
    for stop in 0..calls.get() {
        let count = Cell::new(0);
        let error = validated_pixels(&frame(&rgba, 256, 256), layout, expected, &|| {
            let at = count.get();
            count.set(at + 1);
            if at == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel seal loop",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    assert_eq!(Sha256::digest(&rgba), original);
}
#[test]
fn preview_quote_preserves_full_existing_encoder_boundary_and_prices_actual_scratch() {
    let maximum = Layout::new(
        65536,
        16,
        super::super::super::jxr_encode::ENCODED_BYTES_MAX,
    )
    .unwrap();
    let bytes = jxr_preview_reservation_bytes(maximum).unwrap();
    assert_eq!(
        bytes,
        2 * maximum.encoded_cap()
            + HEAP_BYTES
            + OUTPUT_BYTES_MAX as u64
            + u64::from(JXR_PREVIEW_SIDE).pow(2) * 4 * 3
            + RGB_SCRATCH as u64
            + 65536
    );
    assert!(bytes > HEAP_BYTES + maximum.encoded_cap());
    assert!(Layout::new(65537, 1, 4096).is_err());
    assert!(Layout::new(65536, 17, 4096).is_err());
}

use super::*;
fn png() -> Vec<u8> {
    image::encode(
        image::ImageFormat::Png,
        2,
        1,
        &[11, 22, 33, 0, 44, 55, 66, 127],
        90,
    )
    .unwrap()
}
#[test]
fn bounded_png_decode_keeps_exact_pixels_until_last_reader_drops() {
    let budget = SharedBudget::new(MAX_PIXELS * 4);
    for frontend in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
    ] {
        let decoded = decode(&png(), frontend, &budget).unwrap();
        assert_eq!(decoded.image.rgba(), &[11, 22, 33, 0, 44, 55, 66, 127]);
        assert_eq!((decoded.image.width(), decoded.image.height()), (2, 1));
        assert!(decoded.losses.is_empty());
        let held = decoded.image.clone();
        drop(decoded);
        assert!(budget.usage().reserved >= 8);
        drop(held);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn malformed_mislabeled_and_unadmitted_pngs_return_no_pixels_or_credit() {
    let budget = SharedBudget::new(MAX_PIXELS * 4);
    for bytes in [b"not png".as_slice(), b"\x89PNG\r\n\x1a\n".as_slice()] {
        assert!(decode(bytes, Frontend::ClassicV11, &budget).is_err());
        assert_eq!(budget.usage().reserved, 0);
    }
    let denied = SharedBudget::new(8);
    assert_eq!(
        decode(&png(), Frontend::ClassicV11, &denied)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(denied.usage().reserved, 0);
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_test_host_never_claims_browser_read_support() {
    assert!(!read_supported());
    assert_eq!(
        read(
            &eframe::egui::Context::default(),
            Frontend::ClassicV11,
            SharedBudget::new(64 << 20)
        )
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
}

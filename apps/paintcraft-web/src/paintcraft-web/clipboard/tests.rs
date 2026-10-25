use super::*;
use paintcraft_core::types::resources::SharedBudget;
fn image(width: u32, height: u32, rgba: Vec<u8>) -> Image {
    Image::from_rgba(
        width,
        height,
        rgba,
        &SharedBudget::new(80 << 20),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn png_payload_preserves_straight_rgba_including_hidden_rgb() {
    let pixels = vec![
        255, 0, 17, 0, 21, 230, 91, 1, 11, 55, 99, 127, 200, 100, 50, 255,
    ];
    let source = image(2, 2, pixels.clone());
    let png = encode_png(&source).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = paintcraft_formats::image::decode(&png).unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 2));
    assert_eq!(decoded.rgba, pixels);
    assert_eq!(source.rgba(), pixels);
}
#[test]
fn browser_pixel_admission_precedes_encoding_and_submission() {
    // Valid core payload, beyond the smaller browser-specific allowance.
    let too_large = image(4097, 1024, vec![0; 4097 * 1024 * 4]);
    assert_eq!(
        encode_png(&too_large).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        submit(&eframe::egui::Context::default(), too_large)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let maximum = image(2048, 2048, vec![0; 2048 * 2048 * 4]);
    validate(&maximum).unwrap();
}
#[test]
fn completion_is_pending_until_acknowledged_and_delivered_only_once() {
    let (mut job, complete) = channel();
    assert!(job.try_result().is_none());
    complete.complete(Ok(()));
    assert!(job.try_result().unwrap().is_ok());
    assert!(job.try_result().is_none());
    let mut job: Job = Job::completed(Err(Error::new(
        ErrorCode::StorageFailed,
        "permission denied in fake adapter",
    )));
    let error = job.try_result().unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::StorageFailed);
    assert!(job.try_result().is_none());
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn headless_host_never_claims_a_browser_clipboard_write() {
    let source = image(1, 1, vec![20, 40, 60, 128]);
    let error = submit(&eframe::egui::Context::default(), source).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
}

#[test]
fn dropped_receiver_retires_late_image_and_its_capacity() {
    let budget = SharedBudget::new(64);
    let image = Image::from_rgba(1, 1, vec![1, 2, 3, 4], &budget, &|| Ok(())).unwrap();
    let (job, complete) = channel::<Image>();
    assert!(complete.live());
    drop(job);
    assert!(!complete.live());
    complete.complete(Ok(image));
    assert_eq!(budget.usage().reserved, 0);
}

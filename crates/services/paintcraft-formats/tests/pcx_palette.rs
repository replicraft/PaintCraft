use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    export::{self, FlatOptions},
    image::{self, ImageFormat},
    pcx::{self, Mode},
};
use std::cell::Cell;

fn gradient() -> Vec<u8> {
    (0..400u32)
        .flat_map(|i| [i as u8, (i >> 8) as u8, i.wrapping_mul(53) as u8, 255])
        .collect()
}

#[test]
fn exact_palette_and_quantization_share_the_encoded_preview_without_source_changes() {
    let pixels = gradient();
    let before = pixels.clone();
    assert_eq!(
        pcx::prepare_rgba(40, 10, &pixels, Mode::IndexedExact, &|| Ok(()))
            .err()
            .unwrap()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let prepared = pcx::prepare_rgba(40, 10, &pixels, Mode::Quantized256, &|| Ok(())).unwrap();
    assert!(prepared.is_quantized());
    let preview = prepared.preview(40, &|| Ok(())).unwrap();
    assert_eq!((preview.width, preview.height), (40, 10));
    let encoded = prepared.encode(&|| Ok(())).unwrap();
    assert_eq!(encoded[65], 1);
    assert_eq!(image::decode(&encoded).unwrap().rgba, preview.rgba);
    assert_ne!(preview.rgba, pixels);
    assert_eq!(
        encoded,
        pcx::prepare_rgba(40, 10, &pixels, Mode::Quantized256, &|| Ok(()))
            .unwrap()
            .encode(&|| Ok(()))
            .unwrap()
    );
    assert_eq!(pixels, before);
    assert_eq!(
        prepared.preview(257, &|| Ok(())).unwrap_err().code,
        ErrorCode::InputSchema
    );
}

#[test]
fn small_palettes_are_exact_even_when_quantization_is_requested_and_matte_is_explicit() {
    let pixels = [17, 51, 93, 255, 17, 51, 93, 128, 17, 51, 93, 0];
    for mode in [Mode::Rgb24, Mode::IndexedExact, Mode::Quantized256] {
        let prepared = pcx::prepare_rgba(3, 1, &pixels, mode, &|| Ok(())).unwrap();
        assert!(!prepared.is_quantized());
        let preview = prepared.preview(256, &|| Ok(())).unwrap();
        assert_eq!((preview.width, preview.height), (3, 1));
        assert_eq!(
            preview.rgba,
            [17, 51, 93, 255, 136, 153, 174, 255, 255, 255, 255, 255]
        );
        assert_eq!(
            image::decode(&prepared.encode(&|| Ok(())).unwrap())
                .unwrap()
                .rgba,
            preview.rgba
        );
    }
}

#[test]
fn quantization_loss_is_required_only_when_colors_change_and_wrong_format_options_fail_early() {
    let pixels = gradient();
    let doc = paintcraft_document::import_image(40, 10, &pixels, "pcx-palette-fixture").unwrap();
    let before = doc.clone();
    let mut options = FlatOptions::new(ImageFormat::Pcx, 90);
    options.pcx_mode = Mode::Quantized256;
    assert_eq!(
        export::export_flat_options(&doc, options, &[])
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let flat = export::export_flat_options(&doc, options, &["palette-quantized".into()]).unwrap();
    assert_eq!(flat.losses, ["palette-quantized"]);
    assert!(doc.same_semantics(&before));
    let bad = FlatOptions {
        format: ImageFormat::Png,
        ..options
    };
    assert_eq!(
        export::export_flat_options_with(&doc, bad, &[], |_| panic!(
            "must reject options before rendering"
        ))
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
}

#[test]
fn every_observed_preparation_and_preview_cancellation_discards_the_private_result() {
    let pixels = gradient();
    for mode in [Mode::Rgb24, Mode::Quantized256] {
        let count = Cell::new(0);
        let prepared = pcx::prepare_rgba(40, 10, &pixels, mode, &|| {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=count.get() {
            let calls = Cell::new(0);
            let result = pcx::prepare_rgba(40, 10, &pixels, mode, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            });
            assert_eq!(result.err().unwrap().code, ErrorCode::OperationCancelled);
        }
        let count = Cell::new(0);
        prepared
            .preview(40, &|| {
                count.set(count.get() + 1);
                Ok(())
            })
            .unwrap();
        for stop in 1..=count.get() {
            let calls = Cell::new(0);
            assert_eq!(
                prepared
                    .preview(40, &|| {
                        calls.set(calls.get() + 1);
                        if calls.get() == stop {
                            Err(Error::new(ErrorCode::OperationCancelled, "test"))
                        } else {
                            Ok(())
                        }
                    })
                    .unwrap_err()
                    .code,
                ErrorCode::OperationCancelled
            );
        }
    }
}

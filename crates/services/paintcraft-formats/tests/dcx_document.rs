use paintcraft_core::types::{Error, ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::{dcx, export, image::ImageFormat, pcx};
use std::cell::Cell;

#[test]
fn one_composite_has_exact_existing_pcx_bytes_and_explicit_palette_policy() {
    let doc = new_document(7, 3, Some(Rgba([17, 51, 93, 255])), 0, POLICY).unwrap();
    for mode in [
        pcx::Mode::Rgb24,
        pcx::Mode::IndexedExact,
        pcx::Mode::Quantized256,
    ] {
        let mut options = export::FlatOptions::new(ImageFormat::Pcx, 100);
        options.pcx_mode = mode;
        let old = export::export_flat_options(&doc, options, &[]).unwrap();
        let prepared =
            dcx::prepare_document(&doc, mode, &[], Default::default(), &|| Ok(())).unwrap();
        assert_eq!(prepared.mode(), mode);
        assert_eq!(prepared.losses(), old.losses);
        let pages = dcx::inspect(prepared.bytes()).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(&prepared.bytes()[4100..], old.bytes);
        assert_eq!(
            dcx::decode_page(prepared.bytes(), 0, 21).unwrap().rgba,
            [17, 51, 93, 255].repeat(21)
        );
        assert_eq!(prepared.into_export().losses, old.losses);
    }
}
#[test]
fn inherited_alpha_hidden_rgb_and_flatten_losses_are_not_silently_accepted() {
    let mut doc = new_document(2, 2, Some(Rgba([12, 34, 56, 0])), 0, POLICY).unwrap();
    let error = dcx::prepare_document(&doc, pcx::Mode::Rgb24, &[], Default::default(), &|| Ok(()))
        .unwrap_err();
    assert!(error.message.contains("alpha-discarded"));
    assert!(error.message.contains("hidden-rgb"));
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = uuid::Uuid::from_u128(1);
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
    let allowed = ["flatten", "hidden-rgb", "alpha-discarded"].map(str::to_owned);
    let prepared = dcx::prepare_document(
        &doc,
        pcx::Mode::IndexedExact,
        &allowed,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(prepared.losses(), allowed);
    assert_eq!(dcx::inspect(prepared.bytes()).unwrap().len(), 1);
    assert_eq!(
        dcx::decode_page(prepared.bytes(), 0, 4).unwrap().rgba,
        [255; 16]
    );
}
#[test]
fn all_limits_precede_rendering_and_renderer_extent_is_verified() {
    let doc = new_document(2, 2, Some(Rgba([1, 2, 3, 255])), 0, POLICY).unwrap();
    for case in 0..3 {
        let mut limits = dcx::DocumentLimits::default();
        match case {
            0 => limits.pixels = 3,
            1 => limits.encoded_bytes = 4100,
            _ => limits.transient_bytes = 1,
        }
        let error =
            dcx::prepare_document_with(&doc, pcx::Mode::Rgb24, &[], limits, &|| Ok(()), |_| {
                panic!("no render before admission")
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
    }
    let error = dcx::prepare_document_with(
        &doc,
        pcx::Mode::Rgb24,
        &[],
        Default::default(),
        &|| Ok(()),
        |_| Ok(vec![0; 4]),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvariantViolation);
}
#[test]
fn cancellation_at_every_stage_returns_no_prepared_output_and_preserves_document() {
    let doc = new_document(65, 3, Some(Rgba([12, 34, 56, 255])), 0, POLICY).unwrap();
    let state = doc.state_id;
    let count = Cell::new(0);
    dcx::prepare_document(
        &doc,
        pcx::Mode::IndexedExact,
        &[],
        Default::default(),
        &|| {
            count.set(count.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let error = dcx::prepare_document(
            &doc,
            pcx::Mode::IndexedExact,
            &[],
            Default::default(),
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(doc.state_id, state);
    }
}

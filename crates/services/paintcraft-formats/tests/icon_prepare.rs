use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    icons::{self, Encoding, EntrySpec, Filter, Kind, WriteLimits},
    pcnd,
};
use std::cell::Cell;

fn spec(width: u32, height: u32) -> EntrySpec {
    EntrySpec {
        width,
        height,
        encoding: Encoding::Png,
        hotspot: None,
    }
}
fn allow() -> Vec<String> {
    icons::KNOWN_EXPORT_LOSSES
        .iter()
        .map(|code| (*code).into())
        .collect()
}

#[test]
fn nearest_fit_is_centered_and_encoded_pixels_match_the_private_preview() {
    let doc = paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255], "source")
        .unwrap();
    let before = pcnd::write_compact(&doc).unwrap();
    let prepared = icons::prepare_document(
        &doc,
        Kind::Icon,
        &[spec(4, 4), spec(2, 1)],
        Filter::Nearest,
        WriteLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(prepared.losses(), ["icon-resized"]);
    assert!(
        prepared
            .encode(&[], &|| Ok(()))
            .unwrap_err()
            .message
            .contains("icon-resized")
    );
    let mut expected = vec![0; 16];
    for _ in 0..2 {
        expected.extend([
            255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255,
        ]);
    }
    expected.extend([0; 16]);
    assert_eq!(prepared.entries()[0].rgba(), expected);
    let bytes = prepared.encode(&allow(), &|| Ok(())).unwrap();
    for (index, entry) in prepared.entries().iter().enumerate() {
        let decoded = icons::decode_entry(&bytes, index as u16, 65536).unwrap();
        assert_eq!(decoded.rgba, entry.rgba());
        assert_eq!(
            (decoded.entry.width, decoded.entry.height),
            (entry.spec.width, entry.spec.height)
        );
    }
    assert_eq!(pcnd::write_compact(&doc).unwrap(), before);
    assert_eq!(
        prepared
            .encode(&["lossy-jpeg".into()], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}

#[test]
fn lanczos_filters_premultiplied_color_and_each_size_comes_from_the_source() {
    let doc =
        paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 255, 0, 0], "edge").unwrap();
    let specs = [spec(16, 8), spec(8, 4)];
    let a = icons::prepare_document(
        &doc,
        Kind::Icon,
        &specs,
        Filter::Lanczos3,
        WriteLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let b = icons::prepare_document(
        &doc,
        Kind::Icon,
        &[specs[1], specs[0]],
        Filter::Lanczos3,
        WriteLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(a.entries()[0].rgba(), b.entries()[1].rgba());
    assert_eq!(a.entries()[1].rgba(), b.entries()[0].rgba());
    assert!(a.losses().iter().any(|code| code == "hidden-rgb"));
    let mut partial = false;
    for pixel in a.entries()[0].rgba().as_chunks::<4>().0 {
        if pixel[3] > 0 {
            assert_eq!(&pixel[..3], &[255, 0, 0]);
            partial |= pixel[3] != 255;
        } else {
            assert_eq!(*pixel, [0; 4]);
        }
    }
    assert!(partial);
}

#[test]
fn layout_and_source_budgets_are_checked_before_rendering() {
    let mut doc =
        paintcraft_document::new_document(1, 1, None, 0, paintcraft_document::POLICY).unwrap();
    for specs in [
        vec![],
        vec![spec(0, 1)],
        vec![spec(257, 1)],
        vec![spec(1, 1); 257],
    ] {
        let calls = Cell::new(0);
        assert!(
            icons::prepare_document(
                &doc,
                Kind::Icon,
                &specs,
                Filter::Nearest,
                WriteLimits::default(),
                &|| {
                    calls.set(calls.get() + 1);
                    Ok(())
                }
            )
            .is_err()
        );
        assert_eq!(calls.get(), 0);
    }
    doc.canvas = (4_194_305, 1);
    assert_eq!(
        icons::prepare_document(
            &doc,
            Kind::Icon,
            &[spec(1, 1)],
            Filter::Nearest,
            WriteLimits::default(),
            &|| panic!("must not start rendering")
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn preparation_cancellation_preserves_the_original_error_at_every_checkpoint() {
    let doc =
        paintcraft_document::new_document(2, 2, None, 0, paintcraft_document::POLICY).unwrap();
    let calls = Cell::new(0);
    icons::prepare_document(
        &doc,
        Kind::Icon,
        &[spec(4, 4)],
        Filter::Lanczos3,
        WriteLimits::default(),
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    for stop in 0..calls.get() {
        let visits = Cell::new(0);
        let error = icons::prepare_document(
            &doc,
            Kind::Icon,
            &[spec(4, 4)],
            Filter::Lanczos3,
            WriteLimits::default(),
            &|| {
                let index = visits.get();
                visits.set(index + 1);
                if index == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "stop icon preparation",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "stop icon preparation");
    }
}

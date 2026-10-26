use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    icns::{self, IconType},
    icons::{self, Filter},
    pcnd,
};
use std::cell::Cell;
fn allow() -> Vec<String> {
    icons::KNOWN_EXPORT_LOSSES
        .iter()
        .map(|code| (*code).into())
        .collect()
}
#[test]
fn typed_preview_and_encoded_pixels_match_independent_centered_nearest_oracle() {
    let doc = paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255], "source")
        .unwrap();
    let before = pcnd::write_compact(&doc).unwrap();
    let prepared = icns::prepare_document(
        &doc,
        &[IconType::Retina16, IconType::Size128],
        Filter::Nearest,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(prepared.losses(), ["icon-resized"]);
    assert_eq!(prepared.entries()[0].kind, IconType::Retina16);
    let expected: Vec<_> = (0..32)
        .flat_map(|y| {
            (0..32).flat_map(move |x| {
                if !(8..24).contains(&y) {
                    [0; 4]
                } else if x < 16 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                }
            })
        })
        .collect();
    assert_eq!(prepared.entries()[0].rgba(), expected);
    assert!(prepared.encode(&[], &|| Ok(())).is_err());
    assert_eq!(
        prepared
            .encode(&["unknown".into()], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let bytes = prepared.encode(&allow(), &|| Ok(())).unwrap();
    for (index, entry) in prepared.entries().iter().enumerate() {
        let decoded =
            icns::decode_entry(&bytes, index as u16, icns::PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(decoded.rgba, entry.rgba());
        assert_eq!(decoded.entry.tag, entry.kind.tag());
        assert_eq!(decoded.entry.logical_width, entry.kind.logical_size());
        assert_eq!(decoded.entry.scale, entry.kind.scale());
    }
    assert_eq!(pcnd::write_compact(&doc).unwrap(), before);
}
#[test]
fn qualified_1024_type_and_independent_source_fit_do_not_inherit_ico_256_limit() {
    let doc = paintcraft_document::import_image(1, 1, &[20, 40, 60, 255], "source").unwrap();
    let prepared = icns::prepare_document(
        &doc,
        &[IconType::Retina512, IconType::Retina16],
        Filter::Nearest,
        &|| Ok(()),
    )
    .unwrap();
    for entry in prepared.entries() {
        assert_eq!(
            entry.rgba().len(),
            (entry.kind.pixel_size() * entry.kind.pixel_size() * 4) as usize
        );
        assert!(
            entry
                .rgba()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [20, 40, 60, 255])
        );
    }
    let swapped =
        icns::prepare_document(&doc, &[IconType::Retina16], Filter::Nearest, &|| Ok(())).unwrap();
    assert_eq!(prepared.entries()[1].rgba(), swapped.entries()[0].rgba());
}
#[test]
fn unqualified_types_duplicates_source_limits_and_every_cancel_preserve_original_state() {
    let mut doc =
        paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 255, 0, 0], "edge").unwrap();
    let before = pcnd::write_compact(&doc).unwrap();
    for kinds in [
        vec![],
        vec![IconType::Legacy16],
        vec![IconType::Size16],
        vec![IconType::Size64],
        vec![IconType::Retina16; 2],
    ] {
        assert!(icns::prepare_document(&doc, &kinds, Filter::Nearest, &|| Ok(())).is_err());
    }
    let calls = Cell::new(0);
    let prepared = icns::prepare_document(&doc, &[IconType::Retina16], Filter::Lanczos3, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(prepared.losses().iter().any(|code| code == "hidden-rgb"));
    for stop in 1..=calls.get() {
        let visits = Cell::new(0);
        let error = icns::prepare_document(&doc, &[IconType::Retina16], Filter::Lanczos3, &|| {
            visits.set(visits.get() + 1);
            if visits.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "stop"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    assert_eq!(pcnd::write_compact(&doc).unwrap(), before);
    doc.canvas = (icns::PIXELS_MAX as u32 + 1, 1);
    assert_eq!(
        icns::prepare_document(&doc, &[IconType::Retina16], Filter::Nearest, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}

//! Native PAINT preparation: raw geometry/pixel and explicit semantic-policy oracles.
use paintcraft_core::types::{Error, ErrorCode, Rgba};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_formats::{paint, pcnd};
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::Cell;
use uuid::Uuid;
const FIRST: [u8; 24] = [
    10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254, 160,
    170, 180, 255,
];
const SECOND: [u8; 24] = [
    9, 8, 7, 255, 6, 5, 4, 254, 3, 2, 1, 128, 200, 180, 160, 127, 140, 120, 100, 1, 80, 60, 40, 0,
];
fn source() -> Document {
    let mut doc = new_document(9, 7, None, 7, POLICY).unwrap();
    let mut admission = Admission::new(131072);
    for (id, name, size, position, rgba) in [
        (1, "UI 日本語", (3, 2), (1, -2), &FIRST),
        (2, "second UI", (2, 3), (-3, 4), &SECOND),
    ] {
        let id = Uuid::from_u128(id);
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: name.into(),
                origin: position,
                visible: true,
                opacity: 255,
                raster: RasterLayer::from_rgba(size.0, size.1, Rgba([0; 4]), rgba, &mut admission)
                    .unwrap(),
            },
        );
    }
    doc
}
fn options() -> paint::Options {
    paint::Options {
        canvas_fill: [0; 4],
        layer_names: paint::LayerNames::Omit,
    }
}
#[test]
fn exact_individual_geometry_hidden_rgb_and_fill16_are_frozen_without_clipping() {
    let mut doc = source();
    let before = pcnd::write(&doc).unwrap();
    let allowed = vec![paint::NAMES_LOSS.into()];
    let prepared =
        paint::prepare_document(&doc, options(), &Default::default(), &allowed, &|| Ok(()))
            .unwrap();
    assert_eq!(prepared.plan().source_pixels, 12);
    assert_eq!(prepared.plan().prepared_pixels, 12);
    assert_eq!(prepared.plan().omitted_names, 2);
    for (index, (size, position, rgba)) in [((3, 2), (1, -2), FIRST), ((2, 3), (-3, 4), SECOND)]
        .into_iter()
        .enumerate()
    {
        let layer = &prepared.layers()[index];
        assert_eq!((layer.width(), layer.height()), size);
        assert_eq!(layer.origin(), position);
        assert_eq!(layer.rgba(), rgba);
    }
    let encoded = prepared.encode(&allowed, &|| Ok(())).unwrap();
    let decoded =
        paintcraft_paint::decode_layers(&encoded, &Default::default(), &|| Ok(())).unwrap();
    assert_eq!(decoded.canvas_fill, [0; 4]);
    assert_eq!(decoded.layers[0].rgba, FIRST);
    assert_eq!(decoded.layers[1].rgba, SECOND);
    assert!(
        decoded
            .layers
            .iter()
            .all(|layer| layer.item_name.is_empty())
    );
    assert_eq!(pcnd::write(&doc).unwrap(), before);
    let first = doc.layers.get_mut(&doc.roots[0]).unwrap();
    first.origin = (123, 456);
    first.opacity = 0;
    first.name.clear();
    assert_eq!(prepared.encode(&allowed, &|| Ok(())).unwrap(), encoded);
    let fill = [0x1234, 0xabcd, 0x7f00, 0xffff];
    let other = paint::Options {
        canvas_fill: fill,
        ..options()
    };
    let source = source();
    let allowed = vec![paint::NAMES_LOSS.into(), paint::FILL_LOSS.into()];
    let prepared =
        paint::prepare_document(&source, other, &Default::default(), &allowed, &|| Ok(())).unwrap();
    assert_eq!(
        paintcraft_paint::decode_layers(
            &prepared.encode(&allowed, &|| Ok(())).unwrap(),
            &Default::default(),
            &|| Ok(())
        )
        .unwrap()
        .canvas_fill,
        fill
    );
    assert_eq!(prepared.plan().options, other);
}
#[test]
fn clear_fill_and_empty_names_need_no_fabricated_loss_but_real_changes_require_consent() {
    let mut doc = source();
    for layer in doc.layers.values_mut() {
        layer.name.clear();
    }
    let exact = paint::Options {
        layer_names: paint::LayerNames::RequireEmpty,
        ..options()
    };
    let prepared =
        paint::prepare_document(&doc, exact, &Default::default(), &[], &|| Ok(())).unwrap();
    assert!(prepared.plan().losses.is_empty());
    doc.title = Some("actual source title".into());
    doc.layers.get_mut(&doc.roots[0]).unwrap().name = "actual UI name".into();
    assert!(paint::preflight(&doc, exact, &Default::default(), &|| Ok(())).is_err());
    let selected = paint::Options {
        canvas_fill: [0, 0, 0, 65535],
        ..options()
    };
    let plan = paint::preflight(&doc, selected, &Default::default(), &|| Ok(())).unwrap();
    assert_eq!(
        plan.losses,
        [paint::TITLE_LOSS, paint::NAMES_LOSS, paint::FILL_LOSS]
    );
    for allowed in [
        vec![],
        vec![paint::NAMES_LOSS.into()],
        vec!["flatten".into()],
    ] {
        assert!(
            paint::prepare_document(&doc, selected, &Default::default(), &allowed, &|| Ok(()))
                .is_err()
        );
    }
}
#[test]
fn unsupported_flags_overflow_and_separate_source_prepared_budgets_refuse_before_storage() {
    let doc = source();
    let before = pcnd::write(&doc).unwrap();
    let allowed = vec![paint::NAMES_LOSS.into()];
    for case in 0..4 {
        let mut bad = doc.clone();
        let layer = bad.layers.get_mut(&bad.roots[0]).unwrap();
        match case {
            0 => layer.visible = false,
            1 => layer.opacity = 254,
            2 => layer.origin = (i64::MAX, 0),
            _ => layer.origin = (0, i64::MIN),
        }
        assert_eq!(
            paint::prepare_document(&bad, options(), &Default::default(), &allowed, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    for case in 0..5 {
        let mut limits = paint::Limits::default();
        match case {
            0 => limits.source_pixels = 11,
            1 => limits.prepared_pixels = 11,
            2 => limits.source_resident_bytes = 0,
            3 => limits.transient_bytes = 1,
            _ => limits.writer.metadata_bytes = 1,
        }
        assert_eq!(
            paint::prepare_document(&doc, options(), &limits, &allowed, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    let limits = paint::Limits {
        prepared_pixels: 12,
        ..Default::default()
    };
    assert!(paint::prepare_document(&doc, options(), &limits, &allowed, &|| Ok(())).is_ok());
    let mut bad = doc.clone();
    bad.roots[1] = bad.roots[0];
    assert!(paint::preflight(&bad, options(), &Default::default(), &|| Ok(())).is_err());
    assert_eq!(pcnd::write(&doc).unwrap(), before);
}
#[test]
fn all_preparation_and_encoding_cancellation_checkpoints_leave_source_unchanged() {
    let doc = source();
    let before = pcnd::write(&doc).unwrap();
    let allowed = vec![paint::NAMES_LOSS.into()];
    let count = Cell::new(0);
    paint::prepare_document(&doc, options(), &Default::default(), &allowed, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let result =
            paint::prepare_document(&doc, options(), &Default::default(), &allowed, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
                } else {
                    Ok(())
                }
            });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(pcnd::write(&doc).unwrap(), before);
    }
    let prepared =
        paint::prepare_document(&doc, options(), &Default::default(), &allowed, &|| Ok(()))
            .unwrap();
    count.set(0);
    prepared
        .encode(&allowed, &|| {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        assert_eq!(
            prepared
                .encode(&allowed, &|| {
                    calls.set(calls.get() + 1);
                    if calls.get() == stop {
                        Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
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

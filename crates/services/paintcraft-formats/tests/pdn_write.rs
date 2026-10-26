//! Native preparation: independent dense coordinate/padding and loss oracles.
use paintcraft_core::types::{Error, ErrorCode, Rgba};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_formats::{pcnd, pdn};
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::Cell;
use uuid::Uuid;
fn options() -> pdn::Options {
    pdn::Options {
        background: pdn::BackgroundRole::Ordinary,
        geometry: pdn::Geometry::Materialize,
    }
}
fn source() -> Document {
    let mut doc = new_document(4, 3, None, 7, POLICY).unwrap();
    let hidden = Rgba([11, 22, 33, 0]);
    let pixels = [
        90, 91, 92, 0, 1, 2, 3, 255, 4, 5, 6, 127, 80, 81, 82, 0, 7, 8, 9, 1, 10, 12, 14, 0,
    ];
    let a = Layer {
        id: Uuid::from_u128(1),
        name: "small 日本語".into(),
        origin: (1, 1),
        visible: true,
        opacity: 127,
        raster: RasterLayer::new(2, 2, hidden),
    };
    let b = Layer {
        id: Uuid::from_u128(2),
        name: "clipped".into(),
        origin: (-1, 0),
        visible: false,
        opacity: 1,
        raster: RasterLayer::from_rgba(3, 2, Rgba([0; 4]), &pixels, &mut Admission::new(65536))
            .unwrap(),
    };
    doc.roots = vec![a.id, b.id];
    doc.layers.insert(a.id, a);
    doc.layers.insert(b.id, b);
    doc
}
fn expected() -> [Vec<u8>; 2] {
    let z = [0, 0, 0, 0];
    let h = [11, 22, 33, 0];
    [
        [z, z, z, z, z, h, h, z, z, h, h, z].concat(),
        [
            [1, 2, 3, 255],
            [4, 5, 6, 127],
            z,
            z,
            [7, 8, 9, 1],
            [10, 12, 14, 0],
            z,
            z,
            z,
            z,
            z,
            z,
        ]
        .concat(),
    ]
}
#[test]
fn explicit_materialization_preserves_layers_hidden_rgb_and_only_pads_outside_extent() {
    let mut doc = source();
    let before = pcnd::write(&doc).unwrap();
    let allowed = vec![pdn::GEOMETRY_LOSS.into(), pdn::CLIP_LOSS.into()];
    let plan = pdn::preflight(&doc, options(), &Default::default(), &|| Ok(())).unwrap();
    assert_eq!(plan.source_pixels, 10);
    assert_eq!(plan.prepared_pixels, 24);
    assert_eq!(plan.clipped_samples, 2);
    assert_eq!(plan.losses, allowed);
    let prepared =
        pdn::prepare_document(&doc, options(), &Default::default(), &allowed, &|| Ok(())).unwrap();
    assert_eq!(pcnd::write(&doc).unwrap(), before);
    for (index, rgba) in expected().iter().enumerate() {
        assert_eq!(prepared.layers()[index].rgba(), rgba);
        assert!(!prepared.layers()[index].is_background());
    }
    assert_eq!(prepared.plan().options, options());
    assert_eq!(
        prepared
            .plan()
            .receipt()
            .get("background_role")
            .unwrap()
            .as_str(),
        Some("ordinary")
    );
    let encoded = prepared.encode(&allowed, &|| Ok(())).unwrap();
    let decoded = paintcraft_pdn::decode_layers(&encoded, &Default::default(), &|| Ok(())).unwrap();
    for (index, (layer, rgba)) in decoded.layers.iter().zip(expected()).enumerate() {
        assert_eq!(layer.rgba, rgba);
        assert_eq!(layer.name, prepared.layers()[index].name());
        assert_eq!(layer.opacity, prepared.layers()[index].opacity());
        assert_eq!(layer.visible, prepared.layers()[index].visible());
    }
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.name.clear();
    layer.opacity = 0;
    layer.origin = (i64::MAX, i64::MIN);
    assert_eq!(encoded, prepared.encode(&allowed, &|| Ok(())).unwrap());
}
#[test]
fn ordinary_roles_are_faithful_while_title_and_new_background_behavior_need_consent() {
    let mut doc = new_document(2, 2, Some(Rgba([1, 2, 3, 0])), 1, POLICY).unwrap();
    let ordinary = pdn::Options {
        background: pdn::BackgroundRole::Ordinary,
        geometry: pdn::Geometry::Exact,
    };
    let prepared =
        pdn::prepare_document(&doc, ordinary, &Default::default(), &[], &|| Ok(())).unwrap();
    assert!(prepared.plan().losses.is_empty());
    assert!(!prepared.layers()[0].is_background());
    doc.title = Some("native title".into());
    let bottom = pdn::Options {
        background: pdn::BackgroundRole::Bottom,
        ..ordinary
    };
    let plan = pdn::preflight(&doc, bottom, &Default::default(), &|| Ok(())).unwrap();
    assert_eq!(plan.losses, [pdn::TITLE_LOSS, pdn::BACKGROUND_LOSS]);
    for allowed in [vec![], vec![pdn::TITLE_LOSS.into()], vec!["flatten".into()]] {
        assert!(
            pdn::prepare_document(&doc, bottom, &Default::default(), &allowed, &|| Ok(())).is_err()
        );
    }
    let prepared =
        pdn::prepare_document(&doc, bottom, &Default::default(), &plan.losses, &|| Ok(())).unwrap();
    assert!(prepared.layers()[0].is_background());
}
#[test]
fn source_and_prepared_allowances_and_inventory_fail_before_preparation() {
    let doc = source();
    let before = pcnd::write(&doc).unwrap();
    let allowed = pdn::KNOWN_LOSSES
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    for case in 0..5 {
        let mut limits = pdn::Limits::default();
        match case {
            0 => limits.source_pixels = 9,
            1 => limits.prepared_pixels = 23,
            2 => limits.source_resident_bytes = 0,
            3 => limits.transient_bytes = 1,
            _ => limits.writer.metadata_bytes = 1,
        }
        assert_eq!(
            pdn::prepare_document(&doc, options(), &limits, &allowed, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    let mut invalid = doc.clone();
    invalid.roots[1] = invalid.roots[0];
    assert!(pdn::preflight(&invalid, options(), &Default::default(), &|| Ok(())).is_err());
    invalid = doc.clone();
    invalid.layers.remove(&invalid.roots[0]);
    assert!(pdn::preflight(&invalid, options(), &Default::default(), &|| Ok(())).is_err());
    assert!(
        pdn::preflight(
            &doc,
            pdn::Options {
                geometry: pdn::Geometry::Exact,
                ..options()
            },
            &Default::default(),
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(pcnd::write(&doc).unwrap(), before);
}
#[test]
fn extreme_origins_are_checked_and_clipped_samples_include_zero_alpha_data() {
    for origin in [(i64::MIN, i64::MAX), (i64::MAX, i64::MIN)] {
        let mut doc = source();
        for layer in doc.layers.values_mut() {
            layer.origin = origin;
        }
        let allowed = vec![pdn::GEOMETRY_LOSS.into(), pdn::CLIP_LOSS.into()];
        let prepared =
            pdn::prepare_document(&doc, options(), &Default::default(), &allowed, &|| Ok(()))
                .unwrap();
        assert_eq!(prepared.plan().clipped_samples, 10);
        for layer in prepared.layers() {
            assert!(layer.rgba().iter().all(|v| *v == 0));
        }
    }
}
#[test]
fn cancellation_never_exposes_partially_prepared_storage_or_changes_source() {
    let doc = source();
    let before = pcnd::write(&doc).unwrap();
    let allowed = vec![pdn::GEOMETRY_LOSS.into(), pdn::CLIP_LOSS.into()];
    let count = Cell::new(0);
    pdn::prepare_document(&doc, options(), &Default::default(), &allowed, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let result = pdn::prepare_document(&doc, options(), &Default::default(), &allowed, &|| {
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
}

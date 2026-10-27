use super::*;
use paintcraft_core::types::{Rgba, resources::SharedBudget};
use paintcraft_document::{POLICY, new_document};
use std::cell::{Cell, RefCell};
fn document(width: u32, height: u32, rgba: &[u8]) -> Document {
    let mut doc = new_document(width, height, Some(Rgba([0, 0, 0, 255])), 0, POLICY).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().raster = paintcraft_raster::RasterLayer::from_rgba(
        width,
        height,
        Rgba([0, 0, 0, 0]),
        rgba,
        &mut paintcraft_raster::Admission::new(1 << 20),
    )
    .unwrap();
    doc
}
fn prepare(doc: &Document, version: Version) -> Result<PreparedMsp> {
    prepare_document_with(
        doc,
        version,
        &[],
        |_, _| Ok(()),
        |_| Ok(()),
        |doc, check| paintcraft_render::render_cancellable(doc, check),
        &|| Ok(()),
    )
}
#[test]
fn v1_default_and_explicit_guarded_path_are_byte_identical_and_v2_matches_independent_golden() {
    let golden = include_bytes!("../../../tests/msp2-writer-fixtures/runs-2048.msp");
    let bitmap = codec::decode(golden, &|| Ok(())).unwrap();
    let rgba = bitmap.rgba(&|| Ok(())).unwrap();
    let doc = document(2048, 3, &rgba);
    let before = doc.clone();
    let default = export_flat(&doc, ImageFormat::Msp, 90, &[]).unwrap();
    assert_eq!(&default.bytes[..4], b"DanM");
    assert_eq!(
        prepare(&doc, Version::UncompressedV1).unwrap().bytes(),
        default.bytes
    );
    for version in [Version::UncompressedV1, Version::RowRleV2] {
        let mut options = FlatOptions::new(ImageFormat::Msp, 90);
        options.msp_version = Some(version);
        let value = export_flat_options(&doc, options, &[]).unwrap();
        assert!(value.losses.is_empty());
        assert_eq!(value.bytes, prepare(&doc, version).unwrap().bytes());
        if version == Version::RowRleV2 {
            assert_eq!(value.bytes, golden);
        }
    }
    assert!(doc.same_semantics(&before));
}
#[test]
fn two_stage_admission_precedes_one_render_and_full_packed_output_allocation() {
    let rgba = [255, 255, 255, 255].repeat(27);
    let doc = document(9, 3, &rgba);
    let pool = SharedBudget::new(32 << 20);
    let leases = RefCell::new(Vec::new());
    let phase = Cell::new(0);
    let value = prepare_document_with(
        &doc,
        Version::RowRleV2,
        &[],
        |layout, source| {
            assert_eq!(phase.replace(1), 0);
            assert_eq!(layout.packed_bytes(), 6);
            assert!(source > 0);
            leases
                .borrow_mut()
                .push(pool.reserve(render_reservation_bytes(layout)? + source)?);
            Ok(())
        },
        |plan| {
            assert_eq!(phase.replace(3), 2);
            assert_eq!(plan.encoded_bytes(), 47); // Header32 +map6 +three 3-byte literal rows.
            leases
                .borrow_mut()
                .push(pool.reserve(plan.additional_bytes() as u64)?);
            Ok(())
        },
        |_, check| {
            check()?;
            assert_eq!(phase.replace(2), 1);
            Ok(rgba.clone())
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(phase.get(), 3);
    assert_eq!(value.plan().layout().version(), Version::RowRleV2);
    assert_eq!(value.source_state(), doc.state_id);
    assert_eq!(
        codec::decode(value.bytes(), &|| Ok(()))
            .unwrap()
            .rgba(&|| Ok(()))
            .unwrap(),
        rgba
    );
    assert_eq!(leases.borrow().len(), 2);
    assert!(pool.usage().reserved > 0);
    drop(value);
    assert!(pool.usage().reserved > 0); // Caller keeps both charges through publication.
    leases.borrow_mut().clear();
    assert_eq!(pool.usage().reserved, 0);
}
#[test]
fn first_and_exact_second_admission_refusals_do_not_rerender_or_change_source() {
    let doc = document(9, 1, &[0, 0, 0, 255].repeat(9));
    let before = doc.clone();
    let denied = || Err(Error::new(ErrorCode::ResourceLimit, "test admission"));
    assert_eq!(
        prepare_document_with(
            &doc,
            Version::RowRleV2,
            &[],
            |_, _| denied(),
            |_| panic!("first admission denied"),
            |_, _| panic!("must not render"),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let renders = Cell::new(0);
    assert_eq!(
        prepare_document_with(
            &doc,
            Version::RowRleV2,
            &[],
            |_, _| Ok(()),
            |_| denied(),
            |doc, check| {
                renders.set(renders.get() + 1);
                paintcraft_render::render_cancellable(doc, check)
            },
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(renders.get(), 1);
    assert!(doc.same_semantics(&before));
}
#[test]
fn both_versions_refuse_gray_alpha_and_wrong_family_without_implicit_conversion() {
    for pixel in [[17, 17, 17, 255], [255, 255, 255, 254], [255, 255, 255, 0]] {
        let doc = document(1, 1, &pixel);
        for version in [Version::UncompressedV1, Version::RowRleV2] {
            assert_eq!(
                prepare(&doc, version).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
        }
    }
    let doc = document(1, 1, &[0, 0, 0, 255]);
    let mut options = FlatOptions::new(ImageFormat::Png, 90);
    options.msp_version = Some(Version::UncompressedV1);
    assert_eq!(
        export_flat_options_with(&doc, options, &[], |_| panic!("wrong family before render"))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
#[test]
fn cancellation_during_counting_and_encoding_preserves_input_and_plan_checks_extent() {
    let rgba = [255, 255, 255, 255].repeat(2048 * 3);
    let layout = RgbaLayout::new(Version::RowRleV2, 2048, 3).unwrap();
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        if calls.get() == 6 {
            Err(Error::new(ErrorCode::OperationCancelled, "count cancelled"))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        codec::preflight_rgba_exact(layout, &rgba, &cancel)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    let plan = codec::preflight_rgba_exact(layout, &rgba, &|| Ok(())).unwrap();
    let other: Vec<u8> = (0..2048 * 3)
        .flat_map(|i| {
            if i % 3 == 0 {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        })
        .collect();
    assert_ne!(
        codec::preflight_rgba_exact(layout, &other, &|| Ok(()))
            .unwrap()
            .encoded_bytes(),
        plan.encoded_bytes()
    );
    assert!(plan.encode(&other, &|| Ok(())).is_err());
    // Cancellation reaches the original packet/packing/encode work too.
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        if calls.get() == 35 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "encode cancelled",
            ))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        plan.encode(&rgba, &cancel).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    assert!(
        rgba.as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [255, 255, 255, 255])
    );
}

#[test]
fn actual_common_losses_need_consent_without_inventing_a_compression_loss() {
    let mut doc = document(1, 1, &[255, 255, 255, 255]);
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = uuid::Uuid::new_v4();
    layer.raster = paintcraft_raster::RasterLayer::new(1, 1, Rgba([17, 51, 93, 0]));
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
    let before = doc.clone();
    let error = prepare_document_with(
        &doc,
        Version::RowRleV2,
        &[],
        |_, _| Ok(()),
        |_| panic!("missing common consent before exact encode admission"),
        |doc, check| paintcraft_render::render_cancellable(doc, check),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    let allowed = vec!["flatten".into(), "hidden-rgb".into()];
    let prepared = prepare_document_with(
        &doc,
        Version::RowRleV2,
        &allowed,
        |_, _| Ok(()),
        |_| Ok(()),
        |doc, check| paintcraft_render::render_cancellable(doc, check),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(prepared.losses(), allowed);
    assert_eq!(
        codec::decode(prepared.bytes(), &|| Ok(()))
            .unwrap()
            .rgba(&|| Ok(()))
            .unwrap(),
        [255, 255, 255, 255]
    );
    assert!(doc.same_semantics(&before));
}

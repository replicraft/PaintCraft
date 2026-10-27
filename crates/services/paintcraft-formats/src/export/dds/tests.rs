use super::*;
use crate::dds::{Encoding, MipCount, MipFilter, SourceAlpha, TextureQuality, Transfer};
use paintcraft_core::types::{Rgba, json};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use std::cell::Cell;
fn options() -> TextureOptions {
    TextureOptions {
        encoding: Encoding::Rgba8,
        transfer: Transfer::Srgb,
        alpha: SourceAlpha::Straight,
        mips: MipCount::Levels(1),
        filter: MipFilter::Nearest,
        quality: TextureQuality::Normal,
    }
}
fn allow(codes: &[&str]) -> Vec<String> {
    codes.iter().map(|s| s.to_string()).collect()
}
#[test]
fn same_plan_one_admission_one_render_and_old_default_bytes() {
    let doc = new_document(7, 5, Some(Rgba([19, 57, 91, 127])), 0, POLICY).unwrap();
    let original = doc.clone();
    let admitted = Cell::new(false);
    let calls = Cell::new(0);
    let inventory = Cell::new(0usize);
    let result = prepare_document_with(
        &doc,
        options(),
        &[],
        |plan, resident| {
            assert!(!admitted.replace(true));
            assert_eq!(plan.source_bytes(), 7 * 5 * 4);
            assert_eq!(
                preparation_reservation_bytes(plan).unwrap(),
                plan.reservation_bytes().unwrap() + METADATA_PAYLOAD_BYTES
            );
            assert_eq!(
                resident,
                doc.layers
                    .values()
                    .map(|l| l.raster.resident_tiles() as u64
                        * paintcraft_raster::RasterLayer::TILE_BYTES as u64)
                    .sum()
            );
            inventory.set(plan.levels().as_ptr() as usize);
            Ok(())
        },
        |doc, check| {
            assert!(admitted.get());
            calls.set(calls.get() + 1);
            paintcraft_render::render_cancellable(doc, check)
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        inventory.get(),
        result.encoded().plan().levels().as_ptr() as usize
    );
    assert_eq!(result.source_state(), doc.state_id);
    assert_eq!(result.transfer().policy(), "native-srgb8-identity-v1");
    assert_eq!(result.transfer().quantized_rgb_samples(), 0);
    let old = export_flat_options(&doc, FlatOptions::new(ImageFormat::Dds, 100), &[]).unwrap();
    assert_eq!(result.encoded().bytes(), old.bytes);
    assert!(result.losses().is_empty());
    assert!(doc.same_semantics(&original));
    assert_eq!(doc.state_id, original.state_id);
}
#[test]
fn exact_all_256_transfer_samples_match_independent_integer_decimal_oracle() {
    let fixture = json::parse(include_bytes!(
        "../../../tests/dds-transfer-fixtures/linear8.json"
    ))
    .unwrap();
    let expected = fixture.get("table").unwrap().as_array().unwrap();
    assert_eq!(expected.len(), 256);
    assert_eq!(
        fixture
            .get("exact_continuous_codes")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect::<Vec<_>>(),
        [0, 255]
    );
    let mut rgba = (0..=255u8)
        .flat_map(|sample| [sample, sample, sample, 127])
        .collect::<Vec<_>>();
    let receipt = transfer::convert(&mut rgba, Transfer::Linear, &|| Ok(())).unwrap();
    for (sample, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        let wanted = expected[sample].as_i64().unwrap() as u8;
        assert_eq!(pixel, &[wanted, wanted, wanted, 127]);
    }
    assert_eq!(receipt.quantized_rgb_samples(), 254 * 3);
    assert_eq!(receipt.policy(), LINEAR_POLICY);
}
#[test]
fn linear_output_converts_samples_and_requires_actual_quantization_consent() {
    let mut session =
        Session::new(new_document(1, 1, Some(Rgba([128, 188, 255, 127])), 0, POLICY).unwrap());
    session.mark_unsaved();
    let before = session.document().clone();
    let ticket = session.history_ticket();
    let opts = TextureOptions {
        transfer: Transfer::Linear,
        ..options()
    };
    let error =
        prepare_document(session.document(), opts, &[], |_, _| Ok(()), &|| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert!(error.message.contains(LINEAR_QUANTIZATION_LOSS));
    let prepared = prepare_document(
        session.document(),
        opts,
        &allow(&[LINEAR_QUANTIZATION_LOSS]),
        |_, _| Ok(()),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(&prepared.encoded().bytes()[148..], [55, 128, 255, 127]);
    assert_eq!(prepared.transfer().output(), Transfer::Linear);
    assert_eq!(prepared.transfer().quantized_rgb_samples(), 2);
    let info = texture::inspect_checked(prepared.encoded().bytes(), 1, &|| Ok(())).unwrap();
    assert_eq!(info.transfer, Transfer::Linear);
    assert_eq!(session.history_ticket(), ticket);
    assert!(session.is_dirty());
    assert!(session.document().same_semantics(&before));
    // Physical conversion is exact only at endpoints; do not invent a loss.
    let exact = new_document(1, 1, Some(Rgba([0, 255, 0, 127])), 0, POLICY).unwrap();
    let prepared = prepare_document(&exact, opts, &[], |_, _| Ok(()), &|| Ok(())).unwrap();
    assert!(prepared.losses().is_empty());
    assert_eq!(prepared.transfer().quantized_rgb_samples(), 0);
    assert_eq!(&prepared.encoded().bytes()[148..], [0, 255, 0, 127]);
}
#[test]
fn derived_levels_are_receipt_policy_and_common_hidden_rgb_loss_is_not_duplicated() {
    let doc = new_document(2, 1, Some(Rgba([19, 57, 91, 0])), 0, POLICY).unwrap();
    let opts = TextureOptions {
        mips: MipCount::Full,
        filter: MipFilter::BoxStored,
        ..options()
    };
    let error = prepare_document(&doc, opts, &[], |_, _| Ok(()), &|| Ok(())).unwrap_err();
    assert!(error.message.contains("hidden-rgb"));
    let prepared = prepare_document(&doc, opts, &allow(&["hidden-rgb"]), |_, _| Ok(()), &|| {
        Ok(())
    })
    .unwrap();
    assert_eq!(prepared.losses(), ["hidden-rgb"]);
    assert_eq!(prepared.generated_levels(), 1);
    assert_eq!(
        prepared.encoded().plan().options().filter,
        MipFilter::BoxStored
    );
    assert!(!prepared.losses().iter().any(|s| s.contains("resampling")));
}
#[test]
fn compression_and_flattening_need_consent_but_alpha_is_not_silently_discarded() {
    let mut doc = new_document(3, 2, Some(Rgba([97, 151, 211, 127])), 0, POLICY).unwrap();
    let id = uuid::Uuid::from_u128(0xface);
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = id;
    doc.layers.insert(id, layer);
    doc.roots.push(id);
    let original = doc.clone();
    let opts = TextureOptions {
        encoding: Encoding::Bc3,
        quality: TextureQuality::High,
        ..options()
    };
    let error = prepare_document(&doc, opts, &[], |_, _| Ok(()), &|| Ok(())).unwrap_err();
    assert!(
        error.message.contains("flatten")
            && error.message.contains(texture::BLOCK_COMPRESSION_LOSS)
    );
    let prepared = prepare_document(
        &doc,
        opts,
        &allow(&["flatten", texture::BLOCK_COMPRESSION_LOSS]),
        |_, _| Ok(()),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        prepared.losses(),
        ["flatten", texture::BLOCK_COMPRESSION_LOSS]
    );
    assert_eq!(
        prepared.encoded().compression_policy(),
        Some(texture::UNIFORM_LOSSY_POLICY)
    );
    let bc1 = TextureOptions {
        encoding: Encoding::Bc1,
        alpha: SourceAlpha::Opaque,
        ..options()
    };
    let error = prepare_document(
        &doc,
        bc1,
        &allow(&["flatten", texture::BLOCK_COMPRESSION_LOSS]),
        |_, _| Ok(()),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert!(error.message.contains("opaque"));
    assert!(doc.same_semantics(&original));
}
#[test]
fn denial_unknown_codes_and_invalid_geometry_never_render() {
    let mut doc = new_document(3, 2, Some(Rgba([1, 2, 3, 255])), 0, POLICY).unwrap();
    let calls = Cell::new(0);
    let render = |_: &Document, _: &dyn Fn() -> Result<()>| {
        calls.set(calls.get() + 1);
        panic!("must not render")
    };
    let error = prepare_document_with(
        &doc,
        options(),
        &[],
        |_, _| Err(resource("caller denied")),
        render,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(calls.get(), 0);
    let error = prepare_document_with(
        &doc,
        options(),
        &allow(&["dds-unrecognized"]),
        |_, _| panic!("must not admit"),
        render,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert_eq!(calls.get(), 0);
    doc.canvas = (65537, 1);
    let error = prepare_document_with(
        &doc,
        options(),
        &[],
        |_, _| panic!("must not admit"),
        render,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(calls.get(), 0);
}
#[test]
fn cancellation_and_invalid_renderer_extent_preserve_source_and_publish_nothing() {
    let doc = new_document(2048, 1, Some(Rgba([128, 188, 255, 255])), 0, POLICY).unwrap();
    let before = doc.clone();
    let rendered = Cell::new(false);
    let after_checks = Cell::new(0);
    let check = || {
        if rendered.get() {
            let n = after_checks.get();
            after_checks.set(n + 1);
            if n >= 2 {
                return Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel during conversion",
                ));
            }
        }
        Ok(())
    };
    let error = prepare_document_with(
        &doc,
        TextureOptions {
            transfer: Transfer::Linear,
            ..options()
        },
        &allow(&[LINEAR_QUANTIZATION_LOSS]),
        |_, _| Ok(()),
        |doc, _| {
            let pixels = paintcraft_render::render_flat(doc)?;
            rendered.set(true);
            Ok(pixels)
        },
        &check,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(after_checks.get() >= 3);
    let error = prepare_document_with(
        &doc,
        options(),
        &[],
        |_, _| Ok(()),
        |_, _| Ok(vec![0; 3]),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvariantViolation);
    assert!(doc.same_semantics(&before));
    assert_eq!(doc.state_id, before.state_id);
}

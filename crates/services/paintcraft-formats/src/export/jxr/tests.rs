use super::*;
use paintcraft_core::types::{Rgba, resources::SharedBudget};
use paintcraft_editor::session::Session;
use std::cell::{Cell, RefCell};
fn image(rgba: &[u8]) -> Document {
    paintcraft_document::import_image((rgba.len() / 4) as u32, 1, rgba, &"0".repeat(64)).unwrap()
}
fn layered() -> Document {
    let mut session = Session::new(
        paintcraft_document::new_document(
            2,
            1,
            Some(Rgba([19, 47, 91, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap(),
    );
    let ticket = session.ticket();
    session.apply_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"layer.add","ref":"hidden","name":"Hidden RGB","width":2,"height":1,"origin":{{"x":0,"y":0}},"default_pixel":"#12345600"}}]}}"##,paintcraft_document::POLICY,ticket.state_id,ticket.publication_epoch).as_bytes()).unwrap();
    session.document().clone()
}
fn render(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    paintcraft_render::render_cancellable(doc, check)
}
#[test]
fn opaque_pixels_quotes_and_owned_payload_use_the_existing_compositor() {
    let doc = image(&[19, 47, 91, 255, 201, 7, 73, 255]);
    let before = crate::pcnd::write(&doc).unwrap();
    let layout = Layout::new(2, 1, 1024).unwrap();
    let total = layout.reservation_bytes().unwrap() + 8;
    let budget = SharedBudget::new(total);
    let grant = RefCell::new(None);
    let admitted = Cell::new(false);
    let prepared = prepare_opaque_rgb_checked(
        &doc,
        1024,
        &[],
        |actual, rgba_bytes| {
            assert_eq!(actual, layout);
            assert_eq!(rgba_bytes, 8);
            assert_eq!(actual.rgb_bytes(), 6);
            *grant.borrow_mut() = Some(budget.reserve(actual.reservation_bytes()? + rgba_bytes)?);
            admitted.set(true);
            Ok(())
        },
        |doc, check| {
            assert!(admitted.get());
            assert_eq!(budget.usage().reserved, total);
            render(doc, check)
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(prepared.rgb(), [19, 47, 91, 201, 7, 73]);
    assert!(prepared.losses().is_empty());
    let pointer = prepared.rgb().as_ptr();
    let (actual, rgb, losses) = prepared.into_parts();
    assert_eq!(
        rgb.as_ptr(),
        pointer,
        "moving into the encoder needs no payload copy"
    );
    assert_eq!(actual, layout);
    assert!(losses.is_empty());
    assert_eq!(crate::pcnd::write(&doc).unwrap(), before);
    drop(rgb);
    drop(grant.into_inner());
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn only_actual_base_losses_require_consent_and_no_alpha_loss_is_invented() {
    let doc = layered();
    for allowed in [vec![], vec!["flatten".into()], vec!["hidden-rgb".into()]] {
        let error =
            prepare_opaque_rgb_checked(&doc, 1024, &allowed, |_, _| Ok(()), render, &|| Ok(()))
                .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    }
    let allowed = ["flatten", "hidden-rgb", "alpha-discarded"].map(str::to_owned);
    let prepared =
        prepare_opaque_rgb_checked(&doc, 1024, &allowed, |_, _| Ok(()), render, &|| Ok(()))
            .unwrap();
    assert_eq!(prepared.losses(), ["flatten", "hidden-rgb"]);
    assert_eq!(prepared.rgb(), [19, 47, 91, 19, 47, 91]);
}
#[test]
fn hidden_rgb_and_every_nonopaque_alpha_refuse_even_with_discard_consent() {
    for alpha in [0, 1, 127, 128, 254] {
        let doc = image(&[19, 47, 91, alpha]);
        let allowed = KNOWN_LOSSES.map(str::to_owned);
        let error =
            prepare_opaque_rgb_checked(&doc, 1024, &allowed, |_, _| Ok(()), render, &|| Ok(()))
                .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(error.message.contains("entirely opaque"));
    }
}
#[test]
fn geometry_unknown_losses_admission_and_cancellation_precede_buffer_work() {
    let ordinary = image(&[19, 47, 91, 255]);
    let called = Cell::new(false);
    let render_never = |_: &Document, _: &dyn Fn() -> Result<()>| -> Result<Vec<u8>> {
        called.set(true);
        panic!("renderer must not allocate")
    };
    let mut too_large = ordinary.clone();
    too_large.canvas = (1025, 1024);
    assert!(
        prepare_opaque_rgb_checked(
            &too_large,
            1024,
            &[],
            |_, _| panic!("geometry before admission"),
            render_never,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        prepare_opaque_rgb_checked(
            &ordinary,
            1024,
            &["unknown".into()],
            |_, _| panic!("loss codes before admission"),
            render_never,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        prepare_opaque_rgb_checked(
            &ordinary,
            1024,
            &[],
            |_, _| panic!("cancel before admission"),
            render_never,
            &|| Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
        )
        .is_err()
    );
    let error = prepare_opaque_rgb_checked(
        &ordinary,
        1024,
        &[],
        |layout, rgba| {
            let budget = SharedBudget::new(layout.reservation_bytes()? + rgba - 1);
            let _grant = budget.reserve(layout.reservation_bytes()? + rgba)?;
            Ok(())
        },
        render_never,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(!called.get());
    let cancelled = Cell::new(false);
    let error = prepare_opaque_rgb_checked(
        &ordinary,
        1024,
        &[],
        |_, _| {
            cancelled.set(true);
            Ok(())
        },
        render_never,
        &|| {
            if cancelled.get() {
                Err(Error::new(ErrorCode::OperationCancelled, "after admission"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(!called.get());
    let max = paintcraft_document::new_document(1024, 1024, None, 0, paintcraft_document::POLICY)
        .unwrap();
    let error = prepare_opaque_rgb_checked(
        &max,
        1,
        &[],
        |layout, rgba| {
            assert_eq!(layout.rgb_bytes(), 3 * 1_048_576);
            assert_eq!(rgba, 4 * 1_048_576);
            Err(Error::new(
                ErrorCode::ResourceLimit,
                "declared before allocation",
            ))
        },
        render_never,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
}
#[test]
fn malformed_renderer_and_cancelled_composite_never_return_packed_rgb() {
    let doc = image(&[19, 47, 91, 255]);
    let error = prepare_opaque_rgb_checked(
        &doc,
        1024,
        &[],
        |_, _| Ok(()),
        |_, _| Ok(vec![1, 2, 3]),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvariantViolation);
    let cancelled = Cell::new(false);
    let error = prepare_opaque_rgb_checked(
        &doc,
        1024,
        &[],
        |_, _| Ok(()),
        |doc, check| {
            let rgba = render(doc, check)?;
            cancelled.set(true);
            Ok(rgba)
        },
        &|| {
            if cancelled.get() {
                Err(Error::new(ErrorCode::OperationCancelled, "after render"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}
#[test]
fn inline_export_bytes_and_losses_match_the_prefactor_snapshot() {
    use sha2::{Digest, Sha256};
    let doc = image(&[19, 47, 91, 255, 201, 7, 73, 255]);
    let allowed = KNOWN_LOSSES.map(str::to_owned);
    // Captured from export_flat before this factoring; raw log retained separately.
    for (format, sha) in [
        (
            ImageFormat::Png,
            "7b56a3b2386fe15781ca8ad59ba8e8d672294160d3b35bdf540a122efcca0f0e",
        ),
        (
            ImageFormat::Bmp,
            "2371ba7c7b93808f53ab05305e3816b5383a63168422b0a35c714be92cd95696",
        ),
        (
            ImageFormat::Dds,
            "a36de368b85dee54c7fee7239d1f4e2188763efc010e1b65f466bc894177aadd",
        ),
        (
            ImageFormat::Qoi,
            "e7fe46f928e07f848d9a0b0cbc5e1da0e1e42ca92454343abff8ecfe1735cf9b",
        ),
        (
            ImageFormat::Pnm,
            "00ae1ca385cc58ee638e2595e7fad0808fd265445a36b37178ddbad93d8f1263",
        ),
        (
            ImageFormat::Farbfeld,
            "7993186c0ad447d05926db4e44d571e9888ba340ba406b39bd0c09baae827f07",
        ),
        (
            ImageFormat::Jpeg,
            "cd991c769fe756424b864dc5d46b252fae0aefc8aefe02f042a6a3b923973e22",
        ),
    ] {
        let output = export_flat(&doc, format, 90, &allowed).unwrap();
        assert_eq!(
            paintcraft_core::types::policy::hex(&Sha256::digest(&output.bytes)),
            sha
        );
        assert_eq!(
            output.losses,
            if format == ImageFormat::Jpeg {
                vec!["lossy-jpeg".to_owned()]
            } else {
                vec![]
            }
        );
    }
    let called = Cell::new(false);
    let render = |_: &Document| {
        called.set(true);
        panic!("inline validation remains before rendering")
    };
    assert!(export_flat_with(&doc, ImageFormat::Png, 90, &["unknown".into()], render).is_err());
    assert!(!called.get());
    let mut options = FlatOptions::new(ImageFormat::Png, 90);
    options.pcx_mode = crate::pcx::Mode::IndexedExact;
    let error = export_flat_options_with_check(&doc, options, &[], render, &|| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert!(!called.get());
    let error =
        export_flat_with(&doc, ImageFormat::Png, 90, &[], |_| Ok(vec![1, 2, 3])).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvariantViolation);
    assert_eq!(
        error.message,
        "export renderer returned an invalid pixel buffer"
    );
}

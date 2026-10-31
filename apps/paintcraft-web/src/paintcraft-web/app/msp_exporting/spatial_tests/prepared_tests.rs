//! Real Main captured Export7 headers, codec responses and private authority.
use super::*;
use paintcraft_core::types::filters::ExposureSrgbV1;
fn exposure_document(enabled: bool) -> Document {
    let mut doc = paintcraft_document::import_image(
        2,
        1,
        &[64, 16, 128, 255, 64, 16, 128, 255],
        &"75".repeat(32),
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            uuid::Uuid::from_u128(71),
            enabled,
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(800).unwrap()),
        )])
        .unwrap();
    doc
}
fn pixels(prepared: &Prepared) -> Vec<u8> {
    prepared
        .preview
        .as_ref()
        .unwrap()
        .pixels
        .iter()
        .flat_map(|p| p.to_array())
        .collect()
}
#[test]
fn main_prepared7_actual_codec_pixels_final_source_facts_and_every_context_echo_byte() {
    let doc = exposure_document(true);
    let original = paintcraft_formats::pcnd::write_compact(&doc).unwrap();
    let captured = Arc::new(doc.clone());
    let plan = plan(&doc);
    assert_eq!(plan.protocol_version(), 7);
    admit_version(7).unwrap();
    let owner = Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 0,
        frontend: 3,
    };
    let (request, bytes) = artifact(&plan, &doc, owner);
    assert_eq!(request.header_bytes(), 448);
    assert_eq!(request.response_header(), 608);
    assert!(request.matches_plan(&plan));
    let mut validated = false;
    let prepared = response::validate_request(&request, &request.encode(), bytes.clone(), || {
        validated = true
    })
    .unwrap();
    assert!(validated);
    assert_eq!(prepared.encoded.start, 608);
    assert_eq!(pixels(&prepared), [255; 8]);
    let Request::V7(header) = &request else {
        panic!("prepared profile")
    };
    let source = plan.snapshot(&doc).unwrap();
    assert_eq!(header.source_len as usize, source.len());
    assert_eq!(
        header.programs.encode(),
        plan.program_context(source.len()).unwrap()
    );
    assert_eq!(
        header.spatial.encode_programs(),
        plan.spatial_context(source.len()).unwrap()
    );
    let mut exact_region = header.spatial.encode_programs();
    let before = u64::from_be_bytes(exact_region[40..48].try_into().unwrap());
    // Source length affects the finalized payload, never a substituted wire max.
    let extra = plan.spatial_context(source.len() + 1).unwrap();
    assert_eq!(
        u64::from_be_bytes(extra[40..48].try_into().unwrap()),
        before + 1
    );
    exact_region[40..48].copy_from_slice(&(before + 1).to_be_bytes());
    let mut stale = request.clone();
    let Request::V7(changed) = &mut stale else {
        unreachable!()
    };
    changed.spatial = export::SourceSpatialContext::decode_programs(
        &exact_region,
        wire::WORKER_PAYLOAD_BYTES_MAX,
    )
    .unwrap();
    assert!(!stale.matches_plan(&plan));
    for at in 256..448 {
        let mut echo = request.encode();
        echo[at] ^= 1;
        let mut validated = false;
        assert!(
            response::validate_request(&request, &echo, bytes.clone(), || validated = true)
                .is_err(),
            "request context byte{at}"
        );
        assert!(!validated);
    }
    for at in 416..608 {
        let mut changed = bytes.clone();
        changed[at] ^= 1;
        let mut validated = false;
        assert!(
            response::validate_request(&request, &request.encode(), changed, || validated = true)
                .is_err(),
            "response context byte{at}"
        );
        assert!(!validated);
    }
    assert_eq!(
        paintcraft_formats::pcnd::write_compact(&doc).unwrap(),
        original
    );
    assert_eq!(
        captured.layers[&captured.roots[0]].raster,
        doc.layers[&doc.roots[0]].raster
    );
}
#[test]
fn prepared7_disabled_intent_and_completed_refusal_do_not_mint_bake_authority() {
    let doc = exposure_document(false);
    let plan = plan(&doc);
    assert_eq!(plan.protocol_version(), 7);
    let owner = Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 0,
        frontend: 3,
    };
    let source = plan.snapshot(&doc).unwrap();
    let mut request = Request::capture(&plan, &source, owner, uuid::Uuid::new_v4()).unwrap();
    let Request::V7(header) = &mut request else {
        unreachable!()
    };
    let program = header.programs.encode();
    assert_eq!(u32::from_be_bytes(program[4..8].try_into().unwrap()), 1);
    assert_eq!(u32::from_be_bytes(program[8..12].try_into().unwrap()), 1);
    assert_eq!(u32::from_be_bytes(program[12..16].try_into().unwrap()), 0);
    assert_eq!(u32::from_be_bytes(program[16..20].try_into().unwrap()), 0);
    header.bake_intent = 0;
    let mut packet = request.encode();
    packet.extend(source);
    let bytes = wire::v7::execute(&packet, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
    let mut validated = false;
    let error = response::validate_request(&request, &request.encode(), bytes, || validated = true)
        .unwrap_err();
    assert!(validated, "authenticated completed refusal is releasable");
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
}

#[test]
fn actual_prepared_checkbox_publication_revocation_cancel_and_stale_owner_keep_source() {
    for stale_owner in [false, true] {
        let mut app = WebApp::new(&egui::Context::default());
        app.frontend = Frontend::Studio;
        app.editor = crate::editor::Editor::from_document(exposure_document(true)).unwrap();
        app.export_confirm = Some(ImageExport::Msp);
        app.msp_version = Version::RowRleV2;
        let original = app.editor.session.snapshot();
        let ticket = app.editor.session.history_ticket();
        let dirty = app.editor.session.is_dirty();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1200., 1000.))
            .build_ui_state(
                |ui, app: &mut WebApp| {
                    app.poll_msp_export(ui.ctx());
                    app.msp_export_dialog(ui.ctx());
                },
                app,
            );
        let ctx = h.ctx.clone();
        h.state_mut().begin_msp_export(&ctx);
        h.run();
        {
            let review = h.state().msp.review.as_ref().unwrap();
            assert!(!review.bake_filters);
            assert!(review.plan.is_none());
            assert!(review.pending.is_none());
        }
        h.get_by_label(CHECK).click();
        h.run_steps(3);
        let prepared = {
            let review = h.state().msp.review.as_ref().unwrap();
            assert!(review.bake_filters);
            let plan = review.plan.as_ref().unwrap();
            assert_eq!(plan.protocol_version(), 7);
            let (header, bytes) = artifact(plan, &review.frozen.document, review.wire_owner());
            let prepared =
                response::validate_request(&header, &header.encode(), bytes, || {}).unwrap();
            assert_eq!(pixels(&prepared), [255; 8]);
            prepared
        };
        let (target, _) = pending(h.state_mut().msp.review.as_mut().unwrap());
        *target.borrow_mut() = Some(Ok(prepared));
        h.state_mut().poll_msp_export(&ctx);
        // Completion replaces error/spinner with a texture and loss rows. The
        // modal's remembered centered rectangle updates on subsequent frames.
        h.run_steps(2);
        let checkbox = h.get_by_label(CHECK).rect();
        h.run_steps(1);
        assert_eq!(
            h.get_by_label(CHECK).rect(),
            checkbox,
            "layout must be current before coordinate click"
        );
        assert!(h.state().msp.review.as_ref().unwrap().prepared.is_some());
        assert!(h.state().msp.review.as_ref().unwrap().texture.is_some());
        h.get_by_label(CHECK).click();
        h.run_steps(3);
        {
            let review = h.state().msp.review.as_ref().unwrap();
            assert!(!review.bake_filters);
            assert!(review.prepared.is_none());
            assert!(review.plan.is_none());
            assert!(review.texture.is_none());
        }
        h.get_by_label(CHECK).click();
        h.run_steps(3);
        let (target, cancelled) = pending(h.state_mut().msp.review.as_mut().unwrap());
        let late = {
            let review = h.state().msp.review.as_ref().unwrap();
            let (header, bytes) = artifact(
                review.plan.as_ref().unwrap(),
                &review.frozen.document,
                review.wire_owner(),
            );
            response::validate_request(&header, &header.encode(), bytes, || {}).unwrap()
        };
        if stale_owner {
            h.state_mut().name = "changed.pcnd".into();
            h.state_mut().poll_msp_export(&ctx);
        } else {
            // Pending replaces the host-only unavailable diagnostic with a
            // spinner. Refresh centered footer geometry before pointer input.
            h.run_steps(2);
            let cancel = h.get_by_label("Cancel").rect();
            h.run_steps(1);
            assert_eq!(h.get_by_label("Cancel").rect(), cancel);
            assert!(h.state().msp.review.as_ref().unwrap().pending.is_some());
            h.get_by_label("Cancel").click();
            h.run_steps(3);
        }
        assert!(
            cancelled.get(),
            "cancel/stale-owner must revoke exact pending job; stale_owner={stale_owner}"
        );
        assert!(h.state().msp.review.is_none());
        *target.borrow_mut() = Some(Ok(late));
        h.state_mut().poll_msp_export(&ctx);
        assert!(h.state().msp.review.is_none());
        assert!(h.state().msp.download.is_none());
        assert!(Arc::ptr_eq(&original, &h.state().editor.session.snapshot()));
        assert_eq!(ticket, h.state().editor.session.history_ticket());
        assert_eq!(dirty, h.state().editor.session.is_dirty());
    }
}

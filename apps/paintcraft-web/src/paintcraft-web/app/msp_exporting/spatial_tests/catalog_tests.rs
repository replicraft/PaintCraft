//! Main6 uses actual Worker6 bytes and sealed native-ABI producer facts.
use super::*;
use paintcraft_core::types::filters::LevelsRgbV1;
fn catalog_document(enabled: bool) -> Document {
    let mut doc = paintcraft_document::import_image(2, 1, &[255; 8], &"13".repeat(32)).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            uuid::Uuid::from_u128(91),
            enabled,
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(16, 240, 200, 255, 0).unwrap()),
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
fn captured_catalog_request_literals_all_context_bytes_and_immutable_plan_bindings() {
    for enabled in [false, true] {
        let doc = catalog_document(enabled);
        let original = paintcraft_formats::pcnd::write(&doc).unwrap();
        let plan = plan(&doc);
        assert_eq!(plan.protocol_version(), 6);
        let owner = Owner {
            session: doc.document_id,
            state: doc.state_id,
            epoch: 0,
            frontend: 3,
        };
        let (request, bytes) = artifact(&plan, &doc, owner);
        assert_eq!(request.header_bytes(), 384);
        assert_eq!(request.response_header(), 544);
        assert_eq!(request.encode().len(), 384);
        let mut completed = false;
        let prepared =
            response::validate_request(&request, &request.encode(), bytes.clone(), || {
                completed = true
            })
            .unwrap();
        assert!(completed);
        assert!(prepared.header.matches_plan(&plan));
        assert_eq!(prepared.encoded.start, 544);
        // DDS: five nondefault settings give8+sqrt(224*q), q=clamp(v-16,0,224).
        // MSP: inverted endpoints map the opaque white source to exact black.
        let expected = if enabled {
            [0, 0, 0, 255, 0, 0, 0, 255]
        } else {
            [255; 8]
        };
        assert_eq!(pixels(&prepared), expected);
        for at in 256..384 {
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
        for at in 416..544 {
            let mut response_bytes = bytes.clone();
            response_bytes[at] ^= 1;
            let mut validated = false;
            assert!(
                response::validate_request(&request, &request.encode(), response_bytes, || {
                    validated = true
                })
                .is_err(),
                "response context byte{at}"
            );
            assert!(!validated);
        }
        let mut stale = prepared.header.clone();
        let Request::V6(h) = &mut stale else {
            panic!("wrong captured profile")
        };
        let mut context = h.spatial.encode_catalog();
        let value = u64::from_be_bytes(context[40..48].try_into().unwrap());
        context[40..48].copy_from_slice(&(value + 1).to_be_bytes());
        h.spatial =
            export::SourceSpatialContext::decode_catalog(&context, wire::WORKER_PAYLOAD_BYTES_MAX)
                .unwrap();
        assert!(!stale.matches_plan(&plan));
        let mut stale = prepared.header.clone();
        let Request::V6(h) = &mut stale else {
            unreachable!()
        };
        let mut context = h.catalog.encode();
        context[24] ^= 1;
        h.catalog = export::SourceCatalogContext::decode(&context).unwrap();
        assert!(!stale.matches_plan(&plan));
        let mut refused = request.clone();
        let Request::V6(h) = &mut refused else {
            unreachable!()
        };
        h.bake_intent = 0;
        let mut packet = refused.encode();
        packet.extend(plan.snapshot(&doc).unwrap());
        let response_bytes =
            wire::v6::execute(&packet, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
        let mut validated = false;
        let e = response::validate_request(&refused, &refused.encode(), response_bytes, || {
            validated = true
        })
        .unwrap_err();
        assert!(validated);
        assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(paintcraft_formats::pcnd::write(&doc).unwrap(), original);
    }
}
#[test]
fn actual_catalog_checkbox_publication_revocation_cancel_and_stale_owner_keep_source() {
    for stale_owner in [false, true] {
        let mut app = WebApp::new(&egui::Context::default());
        app.frontend = Frontend::Studio;
        app.editor = crate::editor::Editor::from_document(catalog_document(true)).unwrap();
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
            assert_eq!(plan.protocol_version(), 6);
            let (header, bytes) = artifact(plan, &review.frozen.document, review.wire_owner());
            let prepared =
                response::validate_request(&header, &header.encode(), bytes, || {}).unwrap();
            assert_eq!(pixels(&prepared), [0, 0, 0, 255, 0, 0, 0, 255]);
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

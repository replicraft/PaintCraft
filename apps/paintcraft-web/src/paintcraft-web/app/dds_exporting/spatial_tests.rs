//! Main host selects the captured version and validates actual Worker5 replies.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::filters::{BoxBlurV1, Descriptor};
use paintcraft_document::{Document, FilterInstance, FilterStack};
use paintcraft_formats::{
    codec_worker::Owner,
    export::{self, filter_bake, metadata_policy},
};
use std::sync::Arc;
const CHECK: &str = "Bake editable filters into this exported copy";
fn document(stored: bool) -> Document {
    let mut doc = paintcraft_document::import_image(
        2,
        1,
        &[255, 0, 0, 255, 0, 255, 0, 255],
        &"13".repeat(32),
    )
    .unwrap();
    if stored {
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                uuid::Uuid::from_u128(9),
                true,
                Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
            )])
            .unwrap();
    }
    doc
}
fn plan(doc: &Document) -> wire::SourcePlan {
    let target = metadata_policy::Target::new(
        paintcraft_formats::registry::Family::Dds,
        paintcraft_formats::registry::Operation::ExportFlat,
    );
    let policies = export::CopyPolicies {
        source_metadata: metadata_policy::Policy::default(),
        filter_bake: filter_bake::Policy::capture(doc, target, filter_bake::Intent::Bake, &|| {
            Ok(())
        })
        .unwrap(),
    };
    wire::SourcePlan::capture_with_policies(doc, options(Settings::default()), policies).unwrap()
}
fn artifact(plan: &wire::SourcePlan, doc: &Document, owner: Owner) -> (Request, Vec<u8>) {
    let source = plan.snapshot(doc).unwrap();
    if plan.protocol_version() == 5 {
        let exact = plan.spatial_context(source.len()).unwrap();
        let plus_one = plan.spatial_context(source.len() + 1).unwrap();
        assert_eq!(
            u64::from_be_bytes(plus_one[40..48].try_into().unwrap())
                - u64::from_be_bytes(exact[40..48].try_into().unwrap()),
            1
        );
    }

    let header = Request::capture(plan, &source, owner, uuid::Uuid::new_v4()).unwrap();
    assert_eq!(header.header_bytes(), header.encode().len());
    assert_eq!(header.source_sha256().len(), 32);
    assert!(header.response_max() > 480);
    let mut packet = header.encode();
    packet.extend(source);
    let response = match header.protocol_version() {
        4 => wire::execute(&packet, env!("CARGO_PKG_VERSION"), &|| Ok(())),
        5 => wire::v5::execute(&packet, env!("CARGO_PKG_VERSION"), &|| Ok(())),
        _ => unreachable!(),
    }
    .unwrap();
    (header, response)
}
#[test]
fn main_captured_protocol5_literal_and_echo_context_bindings_keep_legacy4() {
    for stored in [false, true] {
        let doc = document(stored);
        let plan = plan(&doc);
        assert_eq!(plan.protocol_version(), if stored { 5 } else { 4 });
        let owner = Owner {
            session: doc.document_id,
            state: doc.state_id,
            epoch: 0,
            frontend: 3,
        };
        let (request, bytes) = artifact(&plan, &doc, owner);
        let mut valid = false;
        let prepared =
            response::validate_request(&request, &request.encode(), bytes.clone(), || valid = true)
                .unwrap();
        assert!(valid);
        assert_eq!(prepared.header.protocol_version(), plan.protocol_version());
        assert!(prepared.header.matches_plan(&plan));
        assert_eq!(prepared.encoded.start, if stored { 480 } else { 416 });
        let pixels: Vec<u8> = prepared
            .preview
            .as_ref()
            .unwrap()
            .pixels
            .iter()
            .flat_map(|p| p.to_array())
            .collect();
        assert_eq!(
            pixels,
            if stored {
                [170, 85, 0, 255, 85, 170, 0, 255]
            } else {
                [255, 0, 0, 255, 0, 255, 0, 255]
            }
        );
        if stored {
            let mut stale = prepared.header.clone();
            if let Request::V5(h) = &mut stale {
                let mut context = h.spatial.encode();
                let peak = u64::from_be_bytes(context[40..48].try_into().unwrap());
                context[40..48].copy_from_slice(&(peak + 1).to_be_bytes());
                h.spatial = paintcraft_formats::export::SourceSpatialContext::decode(
                    &context,
                    wire::WORKER_PAYLOAD_BYTES_MAX,
                )
                .unwrap();
            }
            assert!(
                !stale.matches_plan(&plan),
                "prepared resource facts must recompute from original sealed plan"
            );
            for at in [256usize, 263, 280, 296, 319] {
                let mut echo = request.encode();
                echo[at] ^= 1;
                let mut completed = false;
                assert!(
                    response::validate_request(&request, &echo, bytes.clone(), || completed = true)
                        .is_err()
                );
                assert!(!completed);
            }
            for at in [416usize, 440, 456, 479] {
                let mut bad = bytes.clone();
                bad[at] ^= 1;
                let mut completed = false;
                assert!(
                    response::validate_request(&request, &request.encode(), bad, || completed =
                        true)
                    .is_err()
                );
                assert!(!completed);
            }
            let mut refused = request.clone();
            if let Request::V5(h) = &mut refused {
                h.bake_intent = 0;
            }
            let mut packet = refused.encode();
            packet.extend(plan.snapshot(&doc).unwrap());
            let bytes = wire::v5::execute(&packet, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
            let mut completed = false;
            let error =
                response::validate_request(&refused, &refused.encode(), bytes, || completed = true)
                    .unwrap_err();
            assert!(completed);
            assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        }
    }
}
fn pending(review: &mut Review) -> (Outcome<Prepared>, Rc<Cell<bool>>) {
    let target = Rc::new(RefCell::new(None));
    let cancelled = Rc::new(Cell::new(false));
    review.error = None;
    review.pending = Some(Pending {
        omit_source_metadata: review.omit_source_metadata,
        bake_filters: review.bake_filters,
        settings: review.settings,
        result: target.clone(),
        cancelled: cancelled.clone(),
        #[cfg(target_arch = "wasm32")]
        cancellation: Default::default(),
    });
    (target, cancelled)
}
#[test]
fn actual_box_checkbox_publication_revocation_cancel_and_stale_owner_keep_source() {
    for stale_owner in [false, true] {
        let mut app = WebApp::new(&egui::Context::default());
        app.frontend = Frontend::Studio;
        app.editor = crate::editor::Editor::from_document(document(true)).unwrap();
        app.export_confirm = Some(ImageExport::Dds);
        let original = app.editor.session.snapshot();
        let ticket = app.editor.session.history_ticket();
        let dirty = app.editor.session.is_dirty();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1200., 1000.))
            .build_ui_state(
                |ui, app: &mut WebApp| {
                    app.poll_dds_export(ui.ctx());
                    app.dds_export_dialog(ui.ctx());
                },
                app,
            );
        let ctx = h.ctx.clone();
        h.state_mut().begin_dds_export(&ctx);
        h.run();
        {
            let review = h.state().dds.review.as_ref().unwrap();
            assert!(!review.bake_filters);
            assert!(review.plan.is_none());
            assert!(review.pending.is_none());
        }
        h.get_by_label(CHECK).click();
        h.run_steps(3);
        let prepared = {
            let review = h.state().dds.review.as_ref().unwrap();
            assert!(review.bake_filters);
            let plan = review.plan.as_ref().unwrap();
            assert_eq!(plan.protocol_version(), 5);
            let (header, bytes) = artifact(plan, &review.frozen.document, review.wire_owner());
            response::validate_request(&header, &header.encode(), bytes, || {}).unwrap()
        };
        let (target, _) = pending(h.state_mut().dds.review.as_mut().unwrap());
        *target.borrow_mut() = Some(Ok(prepared));
        h.state_mut().poll_dds_export(&ctx);
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
        assert!(h.state().dds.review.as_ref().unwrap().prepared.is_some());
        assert!(h.state().dds.review.as_ref().unwrap().texture.is_some());
        h.get_by_label(CHECK).click();
        h.run_steps(3);
        {
            let review = h.state().dds.review.as_ref().unwrap();
            assert!(!review.bake_filters);
            assert!(review.prepared.is_none());
            assert!(review.plan.is_none());
            assert!(review.texture.is_none());
        }
        h.get_by_label(CHECK).click();
        h.run_steps(3);
        let (target, cancelled) = pending(h.state_mut().dds.review.as_mut().unwrap());
        let late = {
            let review = h.state().dds.review.as_ref().unwrap();
            let (header, bytes) = artifact(
                review.plan.as_ref().unwrap(),
                &review.frozen.document,
                review.wire_owner(),
            );
            response::validate_request(&header, &header.encode(), bytes, || {}).unwrap()
        };
        if stale_owner {
            h.state_mut().name = "changed.pcnd".into();
            h.state_mut().poll_dds_export(&ctx);
        } else {
            // Pending replaces the host-only unavailable diagnostic with a
            // spinner. Refresh centered footer geometry before pointer input.
            h.run_steps(2);
            let cancel = h.get_by_label("Cancel").rect();
            h.run_steps(1);
            assert_eq!(h.get_by_label("Cancel").rect(), cancel);
            assert!(h.state().dds.review.as_ref().unwrap().pending.is_some());
            h.get_by_label("Cancel").click();
            h.run_steps(3);
        }
        assert!(
            cancelled.get(),
            "cancel/stale-owner must revoke exact pending job; stale_owner={stale_owner}"
        );
        assert!(h.state().dds.review.is_none());
        *target.borrow_mut() = Some(Ok(late));
        h.state_mut().poll_dds_export(&ctx);
        assert!(h.state().dds.review.is_none());
        assert!(h.state().dds.download.is_none());
        assert!(Arc::ptr_eq(&original, &h.state().editor.session.snapshot()));
        assert_eq!(ticket, h.state().editor.session.history_ticket());
        assert_eq!(dirty, h.state().editor.session.is_dirty());
    }
}

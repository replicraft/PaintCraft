use super::*;
use paintcraft_ui::i18n::{Language, set_language, translate};
use std::sync::Arc;
fn document() -> paintcraft_document::Document {
    // Independently authored source of the pinned Pillow-qualified runs golden.
    let mut packed = vec![0; 256];
    packed.extend([255; 256]);
    packed.extend(0u8..=255);
    let rgba: Vec<_> = packed
        .iter()
        .flat_map(|b| (0..8).map(move |x| if b & (0x80 >> x) == 0 { 0 } else { 255 }))
        .flat_map(|c| [c, c, c, 255])
        .collect();
    paintcraft_document::import_image(2048, 3, &rgba, "msp-host").unwrap()
}
fn artifact(
    doc: &paintcraft_document::Document,
    owner: paintcraft_formats::codec_worker::Owner,
) -> (wire::Header, Vec<u8>) {
    let plan = wire::SourcePlan::capture(doc).unwrap();
    let source = plan.snapshot(doc).unwrap();
    let header = wire::Header::capture(&plan, &source, owner, uuid::Uuid::new_v4()).unwrap();
    let mut request = header.encode().to_vec();
    request.extend(source);
    let bytes = wire::execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
    (header, bytes)
}
fn review(app: &WebApp) -> Review {
    let owner = crate::importing::Owner::capture(app);
    let (header, bytes) = artifact(app.editor.session.document(), owner.wire());
    let prepared = response::validate(&header, &header.encode(), bytes, || {}).unwrap();
    Review {
        owner,
        name: app.name.clone(),
        filename: "copy.msp".into(),
        version: Version::RowRleV2,
        pending: None,
        prepared: Some(prepared),
        texture: None,
        shown: None,
        approved: false,
        error: None,
    }
}
fn layered_document() -> paintcraft_document::Document {
    let mut doc = document();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = uuid::Uuid::new_v4();
    layer.raster =
        paintcraft_raster::RasterLayer::new(2048, 3, paintcraft_core::types::Rgba([128, 0, 0, 0]));
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
    doc
}
#[test]
fn one_complete_rust_validation_retains_pillow_qualified_bytes_and_actual_preview() {
    let doc = document();
    let owner = paintcraft_formats::codec_worker::Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 0,
        frontend: 3,
    };
    let (header, bytes) = artifact(&doc, owner);
    let mut validated = false;
    let mut prepared = response::validate(&header, &header.encode(), bytes.clone(), || {
        validated = true
    })
    .unwrap();
    assert!(validated);
    let golden = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/msp2-writer-fixtures/runs-2048.msp"
    ));
    use sha2::{Digest, Sha256};
    assert_eq!(
        paintcraft_document::hex(&Sha256::digest(golden)),
        "febe39f118876a1f9299e0c0b0a208bc4159a9ef967bd066345eca06e6710056",
        "frozen Pillow-qualified fixture identity"
    );
    assert_eq!(prepared.bytes(), golden);
    let image = prepared.preview.take().unwrap();
    assert_eq!(image.size, [192, 1]);
    assert!(image.pixels.iter().all(|c| *c == egui::Color32::BLACK));
    for at in [0, 12, 32, 73, 89, 121, 129, 161, 199, wire::RESPONSE_HEADER] {
        let mut malformed = bytes.clone();
        malformed[at] ^= 255;
        let mut valid = false;
        assert!(response::validate(&header, &header.encode(), malformed, || valid = true).is_err());
        assert!(!valid);
    }
    let mut request = header.encode().to_vec();
    request.extend([0; 16]);
    let refused = wire::execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
    let mut valid = false;
    assert!(response::validate(&header, &header.encode(), refused, || valid = true).is_err());
    assert!(valid);
}
#[test]
fn actual_en_ja_preview_requires_consent_and_version_change_returns_to_unchanged_v1() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    for language in [Language::English, Language::Japanese] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1100.0, 850.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.state_mut().frontend = Frontend::Studio;
        h.state_mut().editor = Editor::from_document(layered_document()).unwrap();
        h.state_mut().export_confirm = Some(ImageExport::Msp);
        h.state_mut().msp_version = Version::RowRleV2;
        set_language(&h.ctx, language);
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        let name = h.state().name.clone();
        let r = review(h.state());
        assert_eq!(
            r.prepared.as_ref().unwrap().losses,
            ["flatten", "hidden-rgb"]
        );
        h.state_mut().msp.review = Some(r);
        h.run();
        let tr = |s| translate(language, s);
        let download = tr("Download MSP");
        assert!(
            h.query_by_role_and_label(egui::Role::Image, &tr("Base image preview"))
                .is_some()
        );
        assert!(h.get_by_label(&download).accesskit_node().is_disabled());
        let frame = h.ctx.cumulative_frame_nr();
        assert!(
            h.state_mut()
                .msp
                .review
                .as_mut()
                .unwrap()
                .take_ready(frame + 1)
                .is_err()
        );
        h.get_by_label(&tr("I accept these export changes."))
            .click();
        h.run();
        assert!(!h.get_by_label(&download).accesskit_node().is_disabled());
        h.get_by_label(&tr("MSP version")).click();
        h.run();
        h.get_by_label(&tr("Version 1 (uncompressed)")).click();
        h.run();
        assert!(h.state().msp.review.is_none());
        assert_eq!(h.state().msp_version, Version::UncompressedV1);
        assert_eq!(h.state().export_confirm, Some(ImageExport::Msp));
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(ticket, h.state().editor.session.ticket());
        assert_eq!(name, h.state().name);
    }
}
#[test]
fn source_owner_name_modal_or_version_change_cancels_without_adopting_late_results() {
    for case in 0..6 {
        let mut h = egui_kittest::Harness::builder().build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.state_mut().frontend = Frontend::Studio;
        h.state_mut().editor = Editor::from_document(document()).unwrap();
        h.state_mut().export_confirm = Some(ImageExport::Msp);
        h.state_mut().msp_version = Version::RowRleV2;
        let mut r = review(h.state());
        r.prepared = None;
        let target = Rc::new(RefCell::new(None));
        let cancelled = Rc::new(Cell::new(false));
        r.pending = Some(Pending {
            version: Version::RowRleV2,
            result: target.clone(),
            cancelled: cancelled.clone(),
            #[cfg(target_arch = "wasm32")]
            cancellation: Default::default(),
        });
        h.state_mut().msp.review = Some(r);
        match case {
            0 => h.state_mut().name = "renamed.pcnd".into(),
            1 => h.state_mut().frontend = Frontend::Classic,
            2 => h.state_mut().editor = Editor::from_document(document()).unwrap(),
            3 => h.state_mut().export_confirm = Some(ImageExport::Png),
            4 => h.state_mut().settings = true,
            _ => h.state_mut().msp_version = Version::UncompressedV1,
        }
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        let name = h.state().name.clone();
        let ctx = h.ctx.clone();
        h.state_mut().poll_msp_export(&ctx);
        assert!(cancelled.get());
        assert!(h.state().msp.review.is_none());
        assert!(h.state().msp.download.is_none());
        let owner = crate::importing::Owner::capture(h.state());
        let (header, bytes) = artifact(&before, owner.wire());
        *target.borrow_mut() = Some(response::validate(&header, &header.encode(), bytes, || {}));
        h.state_mut().poll_msp_export(&ctx);
        assert!(h.state().msp.review.is_none());
        assert!(h.state().msp.download.is_none());
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(ticket, h.state().editor.session.ticket());
        assert_eq!(name, h.state().name);
        if case == 3 {
            assert_eq!(h.state().export_confirm, Some(ImageExport::Png));
        }
    }
}
#[test]
fn preview_cannot_cross_a_prepared_owner_or_current_frame_and_cancel_keeps_source() {
    let mut h = egui_kittest::Harness::builder().build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.state_mut().frontend = Frontend::Studio;
    h.state_mut().editor = Editor::from_document(document()).unwrap();
    h.state_mut().msp_version = Version::RowRleV2;
    h.state_mut().export_confirm = Some(ImageExport::Msp);
    let mut r = review(h.state());
    r.shown = Some(2);
    assert!(r.take_ready(2).is_err());
    r.prepared.as_mut().unwrap().header.owner.epoch += 1;
    assert!(r.take_ready(3).is_err());
    assert!(r.prepared.is_some());
    r.retire_preview();
    assert!(!r.approved);
    assert!(r.shown.is_none());
    assert_eq!(Version::default(), Version::UncompressedV1);
}
#[test]
fn compact_preview_and_footer_remain_reachable_with_a_full_visible_image() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(390.0, 580.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    let doc = paintcraft_document::new_document(
        192,
        192,
        Some(paintcraft_core::types::Rgba([0, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    h.state_mut().frontend = Frontend::Studio;
    h.state_mut().editor = Editor::from_document(doc).unwrap();
    h.state_mut().export_confirm = Some(ImageExport::Msp);
    h.state_mut().msp_version = Version::RowRleV2;
    let before = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    h.state_mut().msp.review = Some(review(h.state()));
    h.run();
    assert!(h.state().msp.review.as_ref().unwrap().shown.is_some());
    let download = h.get_by_label("Download MSP");
    let rect = download.rect();
    assert!(
        rect.min.x >= 0.0 && rect.max.x <= 390.0 && rect.max.y <= 580.0,
        "{rect:?}"
    );
    assert!(!download.accesskit_node().is_disabled());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().msp.review.is_none());
    assert!(h.state().export_confirm.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    assert_eq!(ticket, h.state().editor.session.ticket());
}
#[test]
fn stale_view_and_finish_retire_only_their_own_export_modal() {
    for finish in [false, true] {
        for prepared in [false, true] {
            for format in [ImageExport::Msp, ImageExport::Dds, ImageExport::Png] {
                let mut h =
                    egui_kittest::Harness::builder().build_eframe(|cc| WebApp::new(&cc.egui_ctx));
                h.state_mut().frontend = Frontend::Studio;
                h.state_mut().editor = Editor::from_document(document()).unwrap();
                h.state_mut().msp_version = Version::RowRleV2;
                h.state_mut().export_confirm = Some(ImageExport::Msp);
                let mut r = review(h.state());
                r.shown = Some(0);
                r.approved = true;
                let cancelled = Rc::new(Cell::new(false));
                if !prepared {
                    r.prepared = None;
                    r.pending = Some(Pending {
                        version: Version::RowRleV2,
                        result: Rc::new(RefCell::new(None)),
                        cancelled: cancelled.clone(),
                        #[cfg(target_arch = "wasm32")]
                        cancellation: Default::default(),
                    });
                }
                h.state_mut().msp.review = Some(r);
                if format == ImageExport::Msp {
                    h.state_mut().name = "changed.pcnd".into();
                }
                h.state_mut().export_confirm = Some(format);
                let before = h.state().editor.session.snapshot();
                let ticket = h.state().editor.session.ticket();
                let name = h.state().name.clone();
                let ctx = h.ctx.clone();
                if finish {
                    h.state_mut().finish_msp_export(&ctx);
                } else {
                    h.state_mut().msp_export_dialog(&ctx);
                }
                assert!(h.state().msp.review.is_none());
                assert!(h.state().msp.download.is_none());
                assert_eq!(
                    h.state().export_confirm,
                    if format == ImageExport::Msp {
                        None
                    } else {
                        Some(format)
                    },
                    "finish={finish} prepared={prepared} format={format:?}"
                );
                if !prepared {
                    assert!(cancelled.get());
                }
                assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
                assert_eq!(ticket, h.state().editor.session.ticket());
                assert_eq!(name, h.state().name);
            }
        }
    }
}

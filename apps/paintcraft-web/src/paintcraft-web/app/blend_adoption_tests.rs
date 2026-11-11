//! Actual browser Loaded -> install boundary closes unsupported visual intent.
use super::*;
use paintcraft_core::types::{ErrorCode, LayerBlendMode, Rgba};
use std::sync::Arc;
#[test]
fn reader_known_pcnd8_and_typed_import_do_not_replace_browser_owner_or_pending_gesture() {
    for (frontend, variant) in [
        (Frontend::Classic, ClassicVariant::V7),
        (Frontend::Classic, ClassicVariant::V10),
        (Frontend::Classic, ClassicVariant::V11),
        (Frontend::Retro, ClassicVariant::V11),
    ] {
        for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
            let ctx = egui::Context::default();
            let mut app = WebApp::new(&ctx);
            app.frontend = frontend;
            app.variant = variant;
            let mut doc = paintcraft_document::new_document(
                2,
                1,
                Some(Rgba([17, 32, 255, 0])),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap();
            let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
            layer.blend_mode = mode;
            layer.visible = false;
            layer.opacity = 0;
            let bytes = paintcraft_formats::pcnd::write(&doc).unwrap();
            assert_eq!(bytes[4], 8);
            let loaded = crate::files::decode_for_profile(
                &bytes,
                "stored.pcnd".into(),
                app.format_profile(),
            )
            .unwrap();
            assert!(loaded.document.same_semantics(&doc));
            assert_eq!(
                Editor::from_document(doc).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            let target = app.editor.target.unwrap();
            app.editor.tools.tool = paintcraft_tools::Tool::Pencil;
            app.editor.tools.pointer_down_subpixel(
                target,
                (256, 256),
                paintcraft_input::PointerButton::Primary,
            );
            assert!(app.editor.tools.gesture_in_progress());
            let before = app.editor.session.snapshot();
            let ticket = app.editor.session.ticket();
            let selection = app.editor.session.selection();
            let undo = app.editor.session.undo_depth();
            let name = app.name.clone();
            let owner = app.studio_canvas_id();
            app.install(loaded);
            assert!(app.message.starts_with("Open failed:"));
            assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
            assert_eq!(app.editor.session.ticket(), ticket);
            assert_eq!(app.editor.session.selection(), selection);
            assert_eq!(app.editor.session.undo_depth(), undo);
            assert_eq!(app.editor.target, Some(target));
            assert_eq!(app.name, name);
            assert_eq!(app.studio_canvas_id(), owner);
            assert!(app.editor.tools.gesture_in_progress());
        }
    }
    let doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba::TRANSPARENT),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    assert!(Editor::from_document(doc).is_ok());
}

#[test]
fn studio_decoded_pcnd8_adopts_real_source_without_preview_edits_or_losing_stored_intent() {
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        for hidden in [false, true] {
            let ctx = egui::Context::default();
            let mut app = WebApp::new(&ctx);
            app.frontend = Frontend::Studio;
            let mut doc = paintcraft_document::import_image(
                2,
                1,
                &[17, 32, 255, 0, 80, 100, 120, 128],
                "decoded-pcnd8",
            )
            .unwrap();
            let root = doc.roots[0];
            let layer = doc.layers.get_mut(&root).unwrap();
            layer.blend_mode = mode;
            if hidden {
                layer.visible = false;
                layer.opacity = 0;
            }
            let mut bytes = paintcraft_formats::pcnd::write(&doc).unwrap();
            assert_eq!(bytes[4], 8);
            let loaded = crate::files::decode_for_profile(
                &bytes,
                "source8.pcnd".into(),
                app.format_profile(),
            )
            .unwrap();
            assert!(loaded.document.same_semantics(&doc));
            assert_eq!(
                Editor::from_document(loaded.document.clone())
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            bytes.fill(0); // The decoded document owns the actual persistent source.
            let old_owner = app.editor.session.owner_id();
            app.install(loaded);
            assert_ne!(app.editor.session.owner_id(), old_owner);
            assert!(app.editor.session.document().same_semantics(&doc));
            assert_eq!(app.editor.session.document().state_id, doc.state_id);
            assert_eq!(app.editor.target, Some(root));
            assert_eq!(app.editor.session.undo_depth(), 0);
            assert!(!app.unsaved());
            assert_eq!(app.name, "source8.pcnd");
            assert_eq!(app.message, "Opened source8.pcnd");
            let stored = &app.editor.session.document().layers[&root];
            assert_eq!(stored.blend_mode, mode);
            assert_eq!(stored.raster.get(0, 0), Rgba([17, 32, 255, 0]));
            assert_eq!(stored.visible, !hidden);
            assert_eq!(stored.opacity, if hidden { 0 } else { 255 });
            let before = app.editor.session.snapshot();
            let ticket = app.editor.session.ticket();
            let saved = paintcraft_formats::pcnd::write(&before).unwrap();
            let reopened =
                crate::files::decode_for_profile(&saved, "saved.pcnd".into(), app.format_profile())
                    .unwrap();
            assert!(reopened.document.same_semantics(&doc));
            assert_eq!(app.editor.session.ticket(), ticket);
            assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
            assert_eq!(app.editor.session.undo_depth(), 0);
        }
    }
}

#[test]
fn studio_source8_adoption_keeps_converted_copy_dirty_and_rejects_oversize_atomically() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba::TRANSPARENT),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().blend_mode = LayerBlendMode::Multiply;
    app.install(crate::files::Loaded {
        raster_preview: false,
        converted_copy: true,
        document: doc.clone(),
        name: "converted.pcnd".into(),
        losses: Vec::new(),
    });
    assert!(app.unsaved());
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    let owner = app.editor.session.owner_id();
    doc.canvas = (4097, 1024); // Strictly above the existing browser4MP ceiling.
    assert_eq!(
        Editor::from_source8_document(doc.clone()).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    app.install(crate::files::Loaded {
        raster_preview: false,
        converted_copy: false,
        document: doc,
        name: "oversize.pcnd".into(),
        losses: Vec::new(),
    });
    assert!(app.message.starts_with("Open failed:"));
    assert_eq!(app.name, "converted.pcnd");
    assert_eq!(app.editor.session.owner_id(), owner);
    assert_eq!(app.editor.session.ticket(), ticket);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert!(app.unsaved());
}

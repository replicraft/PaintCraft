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
        (Frontend::Studio, ClassicVariant::V11),
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

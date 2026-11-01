//! Real Main Context exposes a single filtered document while count stays one.
use super::*;
use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
use uuid::Uuid;
#[test]
fn web_context_offers_single_disabled_noise_intent_without_changing_layer_count() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(paintcraft_core::types::Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(34),
            false,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    let before = doc.clone();
    app.frontend = Frontend::Studio;
    app.editor = Editor::from_document(doc).unwrap();
    let context = app.context();
    assert_eq!(context.layer_count, 1);
    assert!(context.can_flatten);
    assert!(app.editor.session.document().same_semantics(&before));
    assert_eq!(app.editor.session.undo_depth(), 0);
}

fn shortcut_app(frontend: Frontend) -> (egui::Context, WebApp) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(paintcraft_core::types::Rgba([20, 115, 200, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(35),
            true,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    app.frontend = frontend;
    app.editor = Editor::from_document(doc).unwrap();
    (ctx, app)
}
fn flatten_key() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::F,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    }
}

#[test]
fn flatten_shortcut_routing_studio_captures_background_transaction() {
    let (ctx, mut app) = shortcut_app(Frontend::Studio);
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    assert!(app.context().can_flatten);
    assert!(app.keyboard_event(&ctx, &flatten_key()));
    assert!(app.noise_flatten_pending());
    // Routing must capture a transaction, not synchronously publish/render it.
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &app.editor.session.snapshot()
    ));
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert!(!app.editor.session.is_dirty());
}

#[test]
fn flatten_shortcut_routing_classic_refuses_studio_action() {
    let (ctx, mut app) = shortcut_app(Frontend::Classic);
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    assert!(app.context().can_flatten);
    assert!(!app.keyboard_event(&ctx, &flatten_key()));
    assert!(!app.noise_flatten_pending());
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &app.editor.session.snapshot()
    ));
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert!(!app.editor.session.is_dirty());
}

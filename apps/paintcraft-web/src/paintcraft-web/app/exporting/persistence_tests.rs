//! Real App entrypoints preserve a text draft when persistent publication fails.
use super::*;
use crate::editor::persistence_tests::fixture;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::sync::Arc;

#[test]
fn failed_text_placement_can_open_export_and_explicitly_omit_metadata_for_private_copy() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 1000.0))
        .build_eframe(|cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.frontend = Frontend::Studio;
            app.editor =
                crate::editor::Editor::from_document(fixture::exact_doc(64, true)).unwrap();
            app.controls.tool = Tool::Text;
            app.controls.primary = paintcraft_core::types::Rgba([0, 0, 0, 255]);
            app.sync_text();
            app.editor
                .tools
                .begin_text(app.editor.target.unwrap(), (4, 4));
            app.editor.tools.type_text("Ink");
            app
        });
    h.run();
    let original = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    let history = h.state().editor.session.history_view();
    h.get_by_label("Place text").click();
    h.run();
    assert!(Arc::ptr_eq(&original, &h.state().editor.session.snapshot()));
    assert_eq!(h.state().editor.tools.text_content(), Some("Ink"));
    assert!(h.state().unsaved());
    let revision = h.state().editor.tools.preview_revision();
    h.get_by_role_and_label(egui::Role::Button, "File").click();
    h.run();
    h.get_by_label("Export…").click();
    h.run();
    assert!(
        h.get_by_label("Download PNG")
            .accesskit_node()
            .is_disabled()
    );
    let copy = h.state().export_frozen.as_ref().unwrap().document.clone();
    assert!(copy.source_metadata.is_some());
    assert_ne!(copy.state_id, original.state_id);
    let pixels = paintcraft_render::render_flat(&copy).unwrap();
    assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[..3] != [255; 3]));
    h.get_by_label("Omit original source metadata from this exported copy")
        .click();
    h.run();
    assert!(
        !h.get_by_label("Download PNG")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Download PNG").click();
    h.run();
    // The actual dispatch reaches the browser-only boundary on this host build.
    assert!(
        h.state()
            .message
            .contains("Downloads are available in the browser build"),
        "App consumed the actual host-only download refusal: {}",
        h.state().message
    );
    assert!(
        h.state_mut().files.poll().is_none(),
        "App consumed the outcome"
    );
    assert!(Arc::ptr_eq(&original, &h.state().editor.session.snapshot()));
    assert_eq!(ticket, h.state().editor.session.ticket());
    assert_eq!(history, h.state().editor.session.history_view());
    assert_eq!(h.state().editor.tools.text_content(), Some("Ink"));
    assert_eq!(h.state().editor.tools.preview_revision(), revision);
    assert!(h.state().unsaved());
    assert!(h.state().export_frozen.is_none());
}

use super::*;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn keyed_roundtrip_keeps_session_snapshot_controls_text_canvas_and_view() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.name = "Classic owner".into();
    app.zoom = 2.5;
    app.controls.tool = Tool::Pencil;
    app.controls.options.pencil_size = 7;
    app.controls.primary = paintcraft_core::types::Rgba([9, 8, 7, 255]);
    app.text.search = "classic font search".into();
    app.text.font_generation = 13;
    app.workspace.layers = true;
    let rect = egui::Rect::from_min_size(egui::pos2(17.0, 29.0), egui::vec2(100.0, 80.0));
    app.canvas.rect = Some(rect);
    let classic = app.editor.session.history_ticket();
    let snapshot = app.editor.session.snapshot();
    app.switch_frontend(&ctx, Frontend::Studio);
    let studio = app.editor.session.history_ticket();
    app.name = "Studio owner".into();
    app.zoom = 0.5;
    app.text.search = "studio font search".into();
    app.controls.tool = Tool::Line;
    app.switch_frontend(&ctx, Frontend::Classic);
    assert_eq!(app.editor.session.history_ticket(), classic);
    assert!(std::sync::Arc::ptr_eq(
        &snapshot,
        &app.editor.session.snapshot()
    ));
    assert_eq!(app.name, "Classic owner");
    assert_eq!(app.zoom, 2.5);
    assert_eq!(app.controls.tool, Tool::Pencil);
    assert_eq!(app.controls.options.pencil_size, 7);
    assert_eq!(app.controls.primary.0, [9, 8, 7, 255]);
    assert_eq!(app.text.search, "classic font search");
    assert_eq!(app.text.font_generation, 13);
    assert_eq!(app.canvas.rect, Some(rect));
    assert!(app.workspace.layers);
    app.switch_frontend(&ctx, Frontend::Classic);
    assert_eq!(app.editor.session.history_ticket(), classic);
    app.switch_frontend(&ctx, Frontend::Studio);
    assert_eq!(app.editor.session.history_ticket(), studio);
    assert_eq!(app.name, "Studio owner");
    assert_eq!(app.zoom, 0.5);
    assert_eq!(app.text.search, "studio font search");
}

#[test]
fn failed_absent_destination_creation_preserves_active_composition_and_all_owners() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.switch_frontend(&ctx, Frontend::Studio);
    // Drop the Classic owner only to construct the real missing-target branch.
    // Production has no unkeyed pop, and this test never publishes a replacement.
    let original_classic = {
        let mut transfer = app
            .parked
            .prepare(Frontend::Studio, Frontend::Classic)
            .unwrap();
        transfer.take_destination().unwrap()
    };
    let studio = app.editor.session.history_ticket();
    app.test_start_preedit("still composing");
    assert!(app.text.is_composing());
    app.preferences.canvas.remember_last = false;
    app.preferences.canvas.configured = (4096, 4096);
    app.name = "unchanged source".into();
    assert!(!app.switch_frontend_checked(&ctx, Frontend::Classic, false));
    assert_eq!(app.frontend, Frontend::Studio);
    assert_eq!(app.editor.session.history_ticket(), studio);
    assert_eq!(app.name, "unchanged source");
    assert!(app.text.is_composing());
    assert!(app.parked.get(Frontend::Classic).is_none());
    assert!(!app.message.is_empty());
    drop(original_classic);
}

#[test]
fn real_discard_modal_cancels_or_resets_only_the_outgoing_owner() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let ctx = h.ctx.clone();
    h.state_mut().switch_frontend(&ctx, Frontend::Studio);
    let studio = h.state().editor.session.history_ticket();
    h.state_mut().switch_frontend(&ctx, Frontend::Classic);
    h.state_mut().edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([17, 21, 25, 255]),
    });
    let dirty = h.state().editor.session.history_ticket();
    h.state_mut()
        .action(&ctx, Action::ChooseFrontend(Frontend::Studio));
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().frontend, Frontend::Classic);
    assert_eq!(h.state().editor.session.history_ticket(), dirty);
    assert!(h.state().unsaved());
    h.state_mut()
        .action(&ctx, Action::ChooseFrontend(Frontend::Studio));
    h.run();
    h.get_by_label("Discard and continue").click();
    h.run();
    assert_eq!(h.state().frontend, Frontend::Studio);
    assert_eq!(h.state().editor.session.history_ticket(), studio);
    h.state_mut()
        .action(&ctx, Action::ChooseFrontend(Frontend::Classic));
    h.run();
    assert_ne!(h.state().editor.session.history_ticket().owner, dirty.owner);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    assert!(!h.state().unsaved());
}

#[test]
fn attempted_download_and_success_name_ack_do_not_save_or_switch_the_source() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([3, 4, 5, 255]),
    });
    let ticket = app.editor.session.history_ticket();
    let action = Action::ChooseFrontend(Frontend::Studio);
    app.action(&ctx, action);
    assert_eq!(app.confirm_replace, Some(action));
    app.download(&ctx, false);
    assert_eq!(app.frontend, Frontend::Classic);
    assert_eq!(app.editor.session.history_ticket(), ticket);
    assert_eq!(app.confirm_replace, Some(action));
    assert!(app.unsaved());
    // Actual successful browser-result name handling never marks a disk save.
    // This is a Rust handler proof, not an executed browser download claim.
    app.accept_download_name(ticket.owner, Some("downloaded.pcnd".into()));
    assert!(app.unsaved());
    assert_eq!(app.confirm_replace, Some(action));
    assert_eq!(app.frontend, Frontend::Classic);
}

#[test]
fn captured_source_owner_cannot_publish_into_another_key_or_discarded_editor() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let owner = crate::importing::Owner::capture(&app);
    assert!(owner.current(&app));
    app.switch_frontend(&ctx, Frontend::Studio);
    assert!(!owner.current(&app));
    let studio = app.editor.session.history_ticket();
    app.accept_download_name(owner.wire().session, Some("stale-title.pcnd".into()));
    assert_eq!(app.name, "Untitled");
    assert_eq!(app.editor.session.history_ticket(), studio);
    app.switch_frontend(&ctx, Frontend::Classic);
    assert!(owner.current(&app));
    app.edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([11, 12, 13, 255]),
    });
    app.action(&ctx, Action::ChooseFrontend(Frontend::Studio));
    app.discard_and_replace(&ctx);
    app.switch_frontend(&ctx, Frontend::Classic);
    assert!(!owner.current(&app));
}

#[test]
fn failed_discard_preparation_restores_the_existing_target_and_dirty_source() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.switch_frontend(&ctx, Frontend::Studio);
    let studio = app.editor.session.history_ticket();
    app.switch_frontend(&ctx, Frontend::Classic);
    app.edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([21, 22, 23, 255]),
    });
    let source = app.editor.session.history_ticket();
    let snapshot = app.editor.session.snapshot();
    app.test_start_preedit("retain source input");
    app.preferences.canvas.remember_last = false;
    app.preferences.canvas.configured = (4096, 4096);
    app.confirm_replace = Some(Action::ChooseFrontend(Frontend::Studio));
    app.discard_and_replace(&ctx);
    assert_eq!(app.frontend, Frontend::Classic);
    assert_eq!(app.editor.session.history_ticket(), source);
    assert!(std::sync::Arc::ptr_eq(
        &snapshot,
        &app.editor.session.snapshot()
    ));
    assert!(app.text.is_composing());
    assert!(app.unsaved());
    assert_eq!(
        app.confirm_replace,
        Some(Action::ChooseFrontend(Frontend::Studio))
    );
    assert_eq!(
        app.parked
            .get(Frontend::Studio)
            .unwrap()
            .editor
            .session
            .history_ticket(),
        studio
    );
}

#[test]
fn status_and_error_text_follow_the_exact_workspace_on_ordinary_roundtrip() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let classic = app.editor.session.history_ticket();
    app.message = "Open failed: Classic source stayed intact".into();
    app.switch_frontend(&ctx, Frontend::Studio);
    let studio = app.editor.session.history_ticket();
    assert_eq!(app.message, INITIAL_STATUS);
    app.message = "Export failed: Studio destination is unavailable".into();
    app.switch_frontend(&ctx, Frontend::Classic);
    assert_eq!(app.editor.session.history_ticket(), classic);
    assert_eq!(app.message, "Open failed: Classic source stayed intact");
    app.switch_frontend(&ctx, Frontend::Studio);
    assert_eq!(app.editor.session.history_ticket(), studio);
    assert_eq!(
        app.message,
        "Export failed: Studio destination is unavailable"
    );
}

#[test]
fn actual_unsaved_status_never_leaks_and_discard_resets_only_the_outgoing_status() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([51, 52, 53, 255]),
    });
    assert_eq!(app.message, "Unsaved changes - kept only in this tab");
    // Internal ordinary transfer intentionally parks the dirty owner; public
    // admission is separately exercised by the real discard-modal regression.
    app.switch_frontend(&ctx, Frontend::Studio);
    assert!(!app.unsaved());
    assert_eq!(app.message, INITIAL_STATUS);
    let studio = app.editor.session.history_ticket();
    app.message = "Print failed: no printer selected".into();
    app.switch_frontend(&ctx, Frontend::Classic);
    assert!(app.unsaved());
    assert_eq!(app.message, "Unsaved changes - kept only in this tab");
    app.action(&ctx, Action::ChooseFrontend(Frontend::Studio));
    app.discard_and_replace(&ctx);
    assert_eq!(app.editor.session.history_ticket(), studio);
    assert_eq!(app.message, "Print failed: no printer selected");
    app.switch_frontend(&ctx, Frontend::Classic);
    assert!(!app.unsaved());
    assert_eq!(app.message, INITIAL_STATUS);
    assert_eq!(app.editor.session.undo_depth(), 0);
}

#[test]
fn refused_discard_restores_target_status_without_a_blanket_ready_reset() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.switch_frontend(&ctx, Frontend::Studio);
    let studio = app.editor.session.history_ticket();
    app.message = "Studio error must remain available".into();
    app.switch_frontend(&ctx, Frontend::Classic);
    app.edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([61, 62, 63, 255]),
    });
    app.preferences.canvas.remember_last = false;
    app.preferences.canvas.configured = (4096, 4096);
    app.confirm_replace = Some(Action::ChooseFrontend(Frontend::Studio));
    app.discard_and_replace(&ctx);
    assert_eq!(app.frontend, Frontend::Classic);
    assert!(app.unsaved());
    let target = app.parked.get(Frontend::Studio).unwrap();
    assert_eq!(target.editor.session.history_ticket(), studio);
    assert_eq!(target.message, "Studio error must remain available");
}

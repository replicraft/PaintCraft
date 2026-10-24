use crate::WebApp;
use eframe::egui::{Key, Modifiers, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::Rgba;

fn harness() -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h
}
fn hex(h: &mut Harness<WebApp>, value: &str) {
    h.get_by_label("Hex RGB").focus();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    h.run();
    h.get_by_label("Hex RGB").type_text(value);
    h.run();
}

#[test]
fn custom_color_accept_and_cancel_preserve_document_and_history() {
    let mut h = harness();
    let before = h.state().editor.session.ticket();
    for accept in [false, true] {
        h.get_by_label("Palette custom color 10").click();
        h.run();
        hex(&mut h, "#2080FF");
        h.get_by_label("Add custom color").click();
        h.run();
        assert_eq!(h.state().controls.primary, Rgba([0, 0, 0, 255]));
        assert_eq!(h.state().workspace.custom_colors, [None; 28]);
        h.get_by_label(if accept { "OK" } else { "Cancel" }).click();
        h.run();
        assert_eq!(h.state().editor.session.ticket(), before);
        assert_eq!(h.state().editor.session.undo_depth(), 0);
    }
    assert_eq!(h.state().controls.primary, Rgba([32, 128, 255, 255]));
    assert_eq!(
        h.state().workspace.custom_colors[9],
        Some(Rgba([32, 128, 255, 255]))
    );
    assert_eq!(
        h.state().workspace.custom_colors.iter().flatten().count(),
        1
    );
}

#[test]
fn color_modal_owns_shortcuts_and_escape_discards_changes() {
    let mut h = harness();
    let before = h.state().editor.session.document().document_id;
    h.get_by_label("Edit colors…").click();
    h.run();
    hex(&mut h, "#D02040");
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    assert_eq!(h.state().editor.session.document().document_id, before);
    h.key_press(Key::Escape);
    h.run();
    assert!(h.query_by_label("Hex RGB").is_none());
    assert_eq!(h.state().controls.primary, Rgba([0, 0, 0, 255]));
    assert!(!h.state().editor.session.is_dirty());
}

#[test]
fn stale_color_draft_cannot_overwrite_a_newer_color() {
    let mut h = harness();
    h.get_by_label("Edit colors…").click();
    h.run();
    hex(&mut h, "#2080FF");
    h.state_mut().controls.primary = Rgba([90, 80, 70, 255]);
    h.run();
    assert!(h.query_by_label("Hex RGB").is_none());
    assert_eq!(h.state().controls.primary, Rgba([90, 80, 70, 255]));
    assert!(!h.state().editor.session.is_dirty());
}

#[test]
fn secondary_color_edits_do_not_replace_primary_and_invalid_hex_stays_open() {
    let mut h = harness();
    h.get_by_label("Fill color").click();
    h.run();
    hex(&mut h, "#12");
    h.get_by_label("OK").click();
    h.run();
    assert!(h.query_by_label("Hex RGB").is_some());
    assert_eq!(h.state().controls.secondary, Rgba([255; 4]));
    hex(&mut h, "#123456");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().controls.secondary, Rgba([18, 52, 86, 255]));
    assert_eq!(h.state().controls.primary, Rgba([0, 0, 0, 255]));
    assert!(!h.state().editor.session.is_dirty());
}

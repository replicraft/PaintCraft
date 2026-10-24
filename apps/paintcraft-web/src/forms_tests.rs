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

fn properties(h: &mut Harness<WebApp>) {
    h.key_press_modifiers(Modifiers::COMMAND, Key::E);
    h.run();
    assert!(h.query_by_label("Width").is_some());
}

#[test]
fn properties_resize_is_staged_bounded_and_undoable() {
    let mut h = harness();
    let before = h.state().editor.session.ticket();
    properties(&mut h);
    h.get_by_label("Width").set_value_accesskit(64.0);
    h.run();
    h.get_by_label("Height").set_value_accesskit(32.0);
    h.run();
    assert_eq!(h.state().editor.session.ticket(), before);
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, (64, 32));
    assert_eq!(h.state().editor.session.undo_depth(), 1);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, (1000, 486));
    properties(&mut h);
    h.get_by_label("Width").set_value_accesskit(4096.0);
    h.run();
    h.get_by_label("Height").set_value_accesskit(4096.0);
    h.run();
    let before = h.state().editor.session.ticket();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().editor.session.ticket(), before);
    assert!(h.query_by_label("Width").is_some());
    assert!(h.query_by_label("Image limit: 4194304 pixels").is_some());
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, (1000, 486));
}

#[test]
fn unchanged_properties_and_cancel_do_not_publish() {
    let mut h = harness();
    let before = h.state().editor.session.ticket();
    properties(&mut h);
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().editor.session.ticket(), before);
    properties(&mut h);
    h.get_by_label("Width").set_value_accesskit(64.0);
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().editor.session.ticket(), before);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}

#[test]
fn stale_properties_draft_cannot_edit_a_replaced_or_advanced_session() {
    for replace in [false, true] {
        let mut h = harness();
        properties(&mut h);
        h.get_by_label("Width").set_value_accesskit(64.0);
        h.run();
        if replace {
            let doc = h.state().editor.session.document().clone();
            h.state_mut().editor = crate::editor::Editor::from_document(doc).unwrap();
        } else {
            let layer = h.state().editor.target.unwrap();
            h.state_mut()
                .editor
                .apply(paintcraft_tools::Action::Fill {
                    layer,
                    at: (0, 0),
                    color: Rgba([20, 30, 40, 255]),
                    tolerance: 0,
                })
                .unwrap();
        }
        let before = h.state().editor.session.ticket();
        h.run();
        assert!(h.query_by_label("Width").is_none());
        assert_eq!(h.state().editor.session.ticket(), before);
        assert_eq!(h.state().editor.session.document().canvas, (1000, 486));
    }
}

fn defaults(h: &mut Harness<WebApp>) {
    h.get_by_role_and_label(eframe::egui::Role::Button, "View")
        .click();
    h.run();
    h.get_by_label("Default canvas size…").scroll_to_me();
    h.run();
    h.get_by_label("Default canvas size…").click();
    h.run();
}

#[test]
fn configured_default_and_remember_last_have_distinct_new_canvas_behavior() {
    for remember in [true, false] {
        let mut h = harness();
        properties(&mut h);
        h.get_by_label("Width").set_value_accesskit(64.0);
        h.run();
        h.get_by_label("Height").set_value_accesskit(32.0);
        h.run();
        h.get_by_label("OK").click();
        h.run();
        let before = h.state().editor.session.ticket();
        defaults(&mut h);
        h.get_by_label("Width").set_value_accesskit(80.0);
        h.run();
        h.get_by_label("Height").set_value_accesskit(40.0);
        h.run();
        h.get_by_label(if remember {
            "Remember last canvas size"
        } else {
            "Use configured default size"
        })
        .click();
        h.run();
        h.get_by_label("Apply").click();
        h.run();
        assert_eq!(h.state().editor.session.ticket(), before);
        assert_eq!(h.state().editor.session.document().canvas, (64, 32));
        h.key_press_modifiers(Modifiers::COMMAND, Key::N);
        h.run();
        h.get_by_label("Discard and continue").click();
        h.run();
        assert_eq!(
            h.state().editor.session.document().canvas,
            if remember { (64, 32) } else { (80, 40) }
        );
        assert_eq!(h.state().editor.session.undo_depth(), 0);
        assert!(!h.state().editor.session.is_dirty());
    }
}

#[test]
fn default_size_cancel_does_not_change_startup_policy_or_document() {
    let mut h = harness();
    let before = h.state().editor.session.ticket();
    defaults(&mut h);
    h.get_by_label("Width").set_value_accesskit(64.0);
    h.run();
    h.get_by_label("Use configured default size").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().editor.session.ticket(), before);
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, (1000, 486));
    assert!(!h.state().editor.session.is_dirty());
}

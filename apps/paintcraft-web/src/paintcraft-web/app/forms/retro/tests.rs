use super::*;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn retro_color_accept_and_cancel_are_staged_and_do_not_edit_document() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100.0, 800.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    let ctx = h.ctx.clone();
    h.state_mut().switch_frontend(&ctx, Frontend::Retro);
    h.run();
    let ticket = h.state().editor.session.history_ticket();
    let source = h.state().editor.session.snapshot();
    let primary = h.state().controls.primary;
    h.state_mut().begin_colors(false, None);
    let Some(Form::Colors(ref mut draft)) = h.state_mut().form else {
        panic!("color draft")
    };
    draft.editor.color = Rgba([11, 22, 33, 255]);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().controls.primary, primary);
    h.state_mut().begin_colors(true, None);
    let Some(Form::Colors(ref mut draft)) = h.state_mut().form else {
        panic!("color draft")
    };
    draft.editor.color = Rgba([11, 22, 33, 255]);
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().controls.secondary, Rgba([11, 22, 33, 255]));
    assert_eq!(h.state().controls.primary, primary);
    assert_eq!(h.state().editor.session.history_ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(
        &source,
        &h.state().editor.session.snapshot()
    ));
}

#[test]
fn retro_canvas_cancel_preserves_source_and_apply_is_one_undoable_edit() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100.0, 800.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    let ctx = h.ctx.clone();
    h.state_mut().switch_frontend(&ctx, Frontend::Retro);
    h.run();
    let size = h.state().editor.session.document().canvas;
    let depth = h.state().editor.session.undo_depth();
    h.state_mut().begin_canvas(Action::CanvasSize);
    let Some(Form::Canvas(ref mut draft)) = h.state_mut().form else {
        panic!("canvas draft")
    };
    draft.view.size = (32, 24);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, size);
    assert_eq!(h.state().editor.session.undo_depth(), depth);
    h.state_mut().begin_canvas(Action::CanvasSize);
    let Some(Form::Canvas(ref mut draft)) = h.state_mut().form else {
        panic!("canvas draft")
    };
    draft.view.size = (32, 24);
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, (32, 24));
    assert_eq!(h.state().editor.session.undo_depth(), depth + 1);
    h.state_mut().action(&ctx, Action::Undo);
    assert_eq!(h.state().editor.session.document().canvas, size);
}

#[test]
fn all_retro_modals_keep_real_fields_and_buttons_inside_a_320px_viewport() {
    for wide_first in [false, true] {
        for form in 0..3 {
            let initial = if wide_first { 1440.0 } else { 320.0 };
            let mut h = Harness::builder()
                .with_size(egui::vec2(initial, 740.0))
                .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
            let ctx = h.ctx.clone();
            h.state_mut().switch_frontend(&ctx, Frontend::Retro);
            h.run();
            let source = h.state().editor.session.snapshot();
            let ticket = h.state().editor.session.history_ticket();
            let fields: &[&str] = match form {
                0 => {
                    h.state_mut().begin_resize_skew();
                    &[
                        "Resize horizontal",
                        "Resize vertical",
                        "Skew horizontal",
                        "Skew vertical",
                    ]
                }
                1 => {
                    h.state_mut().begin_canvas(Action::CanvasSize);
                    &["Width", "Height"]
                }
                _ => {
                    h.state_mut().begin_colors(false, None);
                    &["Red", "Green", "Blue", "Alpha"]
                }
            };
            h.run();
            if wide_first {
                h.set_size(egui::vec2(320.0, 740.0));
                h.run();
            }
            let screen = h.ctx.content_rect();
            for label in fields {
                let rect = h
                    .get_by_role_and_label(egui::Role::SpinButton, label)
                    .rect();
                assert!(
                    screen.contains_rect(rect),
                    "{label}: {rect:?} outside {screen:?}"
                );
            }
            for label in ["OK", "Cancel"] {
                let rect = h.get_by_role_and_label(egui::Role::Button, label).rect();
                assert!(
                    screen.contains_rect(rect),
                    "{label}: {rect:?} outside {screen:?}"
                );
            }
            let title = match form {
                0 => "Resize and skew",
                1 => "Canvas size",
                _ => "Edit colors",
            };
            assert!(screen.contains_rect(h.get_by_label(title).rect()));
            h.get_by_role_and_label(egui::Role::Button, "Cancel")
                .click();
            h.run();
            assert!(h.state().form.is_none());
            assert_eq!(h.state().editor.session.history_ticket(), ticket);
            assert!(std::sync::Arc::ptr_eq(
                &source,
                &h.state().editor.session.snapshot()
            ));
        }
    }
}

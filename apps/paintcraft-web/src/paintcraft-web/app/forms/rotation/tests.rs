use super::*;
use crate::app::{Action, ClassicVariant, Editor};
use egui_kittest::{Harness, kittest::Queryable};
use std::sync::Arc;

fn harness(frontend: Frontend) -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100.0, 800.0))
        .build_eframe(move |cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.frontend = frontend;
            app.variant = ClassicVariant::V11;
            app.editor = Editor::from_document(
                paintcraft_document::import_image(
                    2,
                    1,
                    &[17, 34, 51, 0, 200, 100, 50, 128],
                    &"66".repeat(32),
                )
                .unwrap(),
            )
            .unwrap();
            app
        });
    h.run();
    h
}
fn angle(app: &mut WebApp, degrees: f64) {
    let Some(Form::Rotate(draft)) = &mut app.form else {
        panic!("captured rotation form")
    };
    draft.view.set_degrees(degrees);
}
#[test]
fn real_shared_modal_cancel_is_private_and_apply_is_one_atomic_undoable_rotation() {
    for frontend in [Frontend::Classic, Frontend::Studio] {
        let mut h = harness(frontend);
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.history_ticket();
        let ctx = h.ctx.clone();
        assert!(h.state().context().can_rotate_custom);
        h.state_mut().action(&ctx, Action::RotateCustom);
        angle(h.state_mut(), 90.0);
        h.run();
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.history_ticket(), ticket);
        h.get_by_label("Cancel").click();
        h.run();
        assert!(h.state().form.is_none());
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.history_ticket(), ticket);
        h.state_mut().action(&ctx, Action::RotateCustom);
        angle(h.state_mut(), 90.0);
        h.run();
        h.get_by_label("Apply").click();
        h.run();
        assert!(h.state().form.is_none());
        let e = &h.state().editor;
        assert_eq!(e.session.document().canvas, (1, 2));
        let raster = &e.session.document().layers[&e.target.unwrap()].raster;
        assert_eq!(raster.get(0, 0), Rgba([17, 34, 51, 0]));
        assert_eq!(raster.get(0, 1), Rgba([200, 100, 50, 128]));
        assert_eq!(e.session.undo_depth(), 1);
        h.state_mut().editor.apply(Edit::Undo).unwrap();
        assert!(h.state().editor.session.document().same_semantics(&before));
        h.state_mut().editor.apply(Edit::Redo).unwrap();
        assert_eq!(h.state().editor.session.document().canvas, (1, 2));
    }
}
#[test]
fn selection_change_closes_captured_modal_without_retargeting_or_publishing() {
    let mut h = harness(Frontend::Studio);
    let ctx = h.ctx.clone();
    h.state_mut().action(&ctx, Action::RotateCustom);
    angle(h.state_mut(), 90.0);
    let before = h.state().editor.session.snapshot();
    h.state_mut()
        .editor
        .apply(Edit::Select {
            corners: Some(((0, 0), (0, 0))),
        })
        .unwrap();
    let after_selection = h.state().editor.session.snapshot();
    assert!(after_selection.same_semantics(&before));
    assert_eq!(h.state().editor.session.undo_depth(), 1);
    let ticket = h.state().editor.session.history_ticket();
    let selection = h.state().editor.session.selection();
    h.run();
    assert!(h.state().form.is_none());
    assert!(Arc::ptr_eq(
        &after_selection,
        &h.state().editor.session.snapshot()
    ));
    assert_eq!(h.state().editor.session.history_ticket(), ticket);
    assert_eq!(h.state().editor.session.selection(), selection);
    assert_eq!(h.state().editor.session.undo_depth(), 1);
}
#[test]
fn unsupported_frontends_and_busy_form_cannot_enable_or_replace_rotation() {
    let mut h = harness(Frontend::Retro);
    assert!(!h.state().context().can_rotate_custom);
    h.state_mut().begin_rotate();
    assert!(h.state().form.is_none());
    for variant in ClassicVariant::ALL {
        h.state_mut().frontend = Frontend::Classic;
        h.state_mut().variant = variant;
        assert_eq!(
            h.state().context().can_rotate_custom,
            variant == ClassicVariant::V11
        );
    }
    h.state_mut().variant = ClassicVariant::V11;
    h.state_mut().begin_rotate();
    assert!(matches!(h.state().form, Some(Form::Rotate(_))));
    assert!(!h.state().context().can_rotate_custom);
    h.state_mut().begin_rotate();
    assert!(matches!(h.state().form, Some(Form::Rotate(_))));
}

#[test]
fn switching_between_supported_hosts_revokes_capture_without_reusing_old_clear_policy() {
    let mut h = harness(Frontend::Classic);
    h.state_mut().begin_rotate();
    angle(h.state_mut(), 90.0);
    let before = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.history_ticket();
    // Keep the SAME session to isolate captured experience from owner guards.
    h.state_mut().frontend = Frontend::Studio;
    h.run();
    assert!(h.state().form.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    assert_eq!(h.state().editor.session.history_ticket(), ticket);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}

#[test]
fn a_new_pending_modal_revokes_rotation_before_accepting_a_response() {
    let mut h = harness(Frontend::Studio);
    h.state_mut().begin_rotate();
    angle(h.state_mut(), 90.0);
    let before = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.history_ticket();
    h.state_mut().settings = true;
    h.run();
    assert!(h.state().form.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    assert_eq!(h.state().editor.session.history_ticket(), ticket);
}

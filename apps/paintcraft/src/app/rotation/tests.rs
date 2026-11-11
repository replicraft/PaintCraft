use super::*;
use egui_kittest::{Harness, kittest::Queryable};
fn app() -> App {
    let mut a = App::new(None);
    a.settings.experience = Experience::Studio;
    a.editor = Editor::with_document(
        paintcraft_document::import_image(
            2,
            1,
            &[17, 34, 51, 0, 200, 100, 50, 128],
            "native-rotation-form/v1",
        )
        .unwrap(),
        None,
    );
    a
}
fn open(a: &mut App, degrees: f64) {
    a.begin_rotate_custom();
    let Some(Confirm::RotateCustom(d)) = a.confirm.as_mut() else {
        panic!("rotation form")
    };
    d.view.set_degrees(degrees);
}
fn harness(a: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1000., 800.))
        .build_ui_state(
            |ui, a: &mut App| {
                a.confirm_dialog(ui);
            },
            a,
        )
}
#[test]
fn actual_modal_cancel_keeps_source_and_apply_runs_one_real_controller_command() {
    let mut a = app();
    let source = a.editor.document_snapshot();
    let history = a.editor.history_view();
    open(&mut a, 90.0);
    let mut h = harness(a);
    h.run();
    assert!(h.state().editor.document().same_semantics(&source));
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().confirm.is_none());
    assert_eq!(h.state().editor.history_view(), history);
    open(h.state_mut(), 90.0);
    h.run();
    h.get_by_label("Apply").click();
    h.run_steps(1);
    h.state_mut().editor.run_pending();
    assert!(h.state().confirm.is_none());
    assert_eq!(h.state().editor.document().canvas, (1, 2));
    assert_eq!(h.state().editor.undo_depth(), 1);
    let ctx = h.ctx.clone();
    h.state_mut().sync_texture(&ctx);
    let saved = paintcraft_formats::pcnd::write_compact(h.state().editor.document()).unwrap();
    let decoded = paintcraft_formats::pcnd::read(&saved).unwrap();
    assert!(decoded.same_semantics(h.state().editor.document()));
    h.state_mut().editor.request(19931, Action::Undo);
    h.state_mut().editor.run_pending();
    assert!(h.state().editor.document().same_semantics(&source));
}
#[test]
fn modal_stale_selection_and_ime_close_or_refuse_before_publication() {
    let mut a = app();
    open(&mut a, 45.0);
    let Some(Confirm::RotateCustom(draft)) = a.confirm.clone() else {
        panic!("draft")
    };
    a.editor.request(
        19940,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    a.editor.run_pending();
    let source = a.editor.document_snapshot();
    let history = a.editor.history_view();
    a.rotate_custom_dialog(&egui::Context::default(), draft);
    assert!(a.confirm.is_none());
    assert!(a.editor.document().same_semantics(&source));
    assert_eq!(a.editor.history_view(), history);
    a.ime_composing = true;
    a.begin_rotate_custom();
    assert!(a.confirm.is_none());
    assert!(a.editor.document().same_semantics(&source));
}
#[test]
fn native_host_capability_exposes_only_real_studio_and_classic11_consumers() {
    let mut a = app();
    assert!(paintcraft_ui::actions::Action::RotateCustom.enabled(a.action_context()));
    a.settings.experience = Experience::Classic;
    a.settings.classic_variant = Some(paintcraft_classic::ClassicVariant::V11);
    assert!(paintcraft_ui::actions::Action::RotateCustom.enabled(a.action_context()));
    a.settings.classic_variant = Some(paintcraft_classic::ClassicVariant::V7);
    assert!(!paintcraft_ui::actions::Action::RotateCustom.enabled(a.action_context()));
    a.settings.experience = Experience::Retro;
    assert!(!paintcraft_ui::actions::Action::RotateCustom.enabled(a.action_context()));
}

#[test]
fn actual_angle_widget_stages_two_values_cancel_and_reopen_apply_once() {
    use std::sync::Arc;
    for experience in [Experience::Classic, Experience::Studio] {
        let mut a = app();
        a.settings.experience = experience;
        a.settings.classic_variant = Some(paintcraft_classic::ClassicVariant::V11);
        let source = a.editor.document_snapshot();
        let history = a.editor.history_view();
        open(&mut a, 0.0);
        let mut h = harness(a);
        h.run();
        h.get_by_role_and_label(egui::Role::SpinButton, "Angle")
            .set_value_accesskit(90.0);
        h.run();
        let Some(Confirm::RotateCustom(d)) = &h.state().confirm else {
            panic!("form")
        };
        assert_eq!(d.view.degrees(), 90.0);
        assert_eq!(d.view.coefficients(), Some((0, 1 << 30)));
        assert_eq!(d.view.output_size(), Some((1, 2)));
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        h.get_by_role_and_label(egui::Role::SpinButton, "Angle")
            .set_value_accesskit(36.87);
        h.run();
        let Some(Confirm::RotateCustom(d)) = &h.state().confirm else {
            panic!("form")
        };
        assert!((d.view.degrees() - 36.87).abs() < 1e-10);
        let (c, s) = d.view.coefficients().expect("valid cached pair");
        assert!(c > 0 && s > 0 && c != s);
        assert_ne!((c, s), (0, 1 << 30));
        let q = 1i128 << 30;
        assert!((i128::from(c).pow(2) + i128::from(s).pow(2) - q * q).abs() <= 2 * q);
        // Independently frozen real-angle bounds, robust to nearestQ30 rounding;
        // 36.87 degrees is deliberately NOT the exact rational (.8,.6) pair.
        assert_eq!(d.view.output_size(), Some((3, 3)));
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        h.get_by_label("Cancel").click();
        h.run();
        assert!(h.state().confirm.is_none());
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        open(h.state_mut(), 0.0);
        h.run();
        h.get_by_role_and_label(egui::Role::SpinButton, "Angle")
            .set_value_accesskit(90.0);
        h.run();
        h.get_by_label("Apply").click();
        h.run_steps(1);
        h.state_mut().editor.run_pending();
        assert!(h.state().confirm.is_none());
        let doc = h.state().editor.document();
        assert_eq!(doc.canvas, (1, 2));
        let layer = &doc.layers[&doc.roots[0]];
        let rgba: Vec<u8> = (0..2).flat_map(|y| layer.raster.get(0, y).0).collect();
        assert_eq!(rgba, [17, 34, 51, 0, 200, 100, 50, 128]);
        assert_eq!(h.state().editor.undo_depth(), 1);
        let rotated = h.state().editor.document_snapshot();
        h.state_mut().editor.request(19960, Action::Undo);
        h.state_mut().editor.run_pending();
        assert!(h.state().editor.document().same_semantics(&source));
        h.state_mut().editor.request(19961, Action::Redo);
        h.state_mut().editor.run_pending();
        assert!(h.state().editor.document().same_semantics(&rotated));
    }
}

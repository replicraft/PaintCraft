use crate::WebApp;
use eframe::egui::{Event, Modifiers, PointerButton, Role, Vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::Rgba;
use paintcraft_tools::Action;

#[test]
fn crop_button_is_selection_gated_undoable_and_preserves_pixels_without_enabling_unimplemented_view()
 {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    assert!(
        h.get_by_role_and_label(Role::Button, "Crop")
            .accesskit_node()
            .is_disabled()
    );
    let layer = h.state().editor.target.unwrap();
    h.state_mut()
        .editor
        .apply(Action::Stroke(paintcraft_tools::Stroke {
            layer,
            points: vec![(40, 50)],
            point_scale: 1,
            color: Rgba([20, 40, 80, 0]),
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::Replace,
        }))
        .unwrap();
    h.get_by_role_and_label(Role::Button, "Select").click();
    h.run();
    let rect = h.state().canvas.rect.unwrap();
    let a = rect.min + Vec2::new(30.5, 40.5);
    let b = rect.min + Vec2::new(59.5, 79.5);
    h.input_mut().events.extend([
        Event::PointerMoved(a),
        Event::PointerButton {
            pos: a,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerMoved(b),
        Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.run();
    let before = h.state().editor.session.snapshot();
    let selection = h.state().editor.session.selection();
    assert!(h.state().context().can_crop_selection && !h.state().context().can_fit_selection);
    h.get_by_role_and_label(Role::Button, "Crop").click();
    h.run();
    let doc = h.state().editor.session.document();
    assert_eq!(doc.canvas, (30, 40));
    assert_eq!(h.state().preferences.canvas.last, Some((30, 40)));
    assert_eq!(doc.layers[&layer].raster.get(10, 10), Rgba([20, 40, 80, 0]));
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor.session.document().same_semantics(&before));
    assert_eq!(h.state().editor.session.selection(), selection);
    assert_eq!(h.state().preferences.canvas.last, Some((30, 40)));
}

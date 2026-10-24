use crate::WebApp;
use eframe::egui::{Event, Modifiers, PointerButton, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::Rgba;
use paintcraft_tools::{Action, Tool};
fn click(h: &mut Harness<WebApp>, x: f32, y: f32, button: PointerButton) {
    let p = h.state().canvas.rect.unwrap().min + Vec2::new(x + 0.5, y + 0.5) * h.state().zoom;
    h.input_mut().events.extend([
        Event::PointerMoved(p),
        Event::PointerButton {
            pos: p,
            button,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: p,
            button,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.run();
}
#[test]
fn web_picker_uses_real_document_pixels_and_updates_controls_without_history_or_dirtiness() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let clean = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    h.get_by_role_and_label(Role::Button, "Color picker")
        .click();
    h.run();
    click(&mut h, 40.0, 50.0, PointerButton::Primary);
    assert_eq!(h.state().controls.primary, Rgba([255; 4]));
    assert_eq!(h.state().controls.tool, Tool::Brush);
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(
        &clean,
        &h.state().editor.session.snapshot()
    ));
    assert!(!h.state().editor.session.is_dirty());
    h.get_by_role_and_label(Role::Button, "Pencil").click();
    h.run();
    let layer = h.state().editor.target.unwrap();
    h.state_mut().edit(Action::Stroke(paintcraft_tools::Stroke {
        layer,
        points: vec![(40, 50)],
        point_scale: 1,
        color: Rgba([255, 0, 0, 255]),
        size: 1,
        round: false,
        mode: paintcraft_tools::BlendMode::Replace,
    }));
    h.run();
    h.state_mut().edit(Action::Select {
        corners: Some(((70, 70), (71, 71))),
    });
    h.run();
    let before = h.state().editor.session.snapshot();
    let selection = h.state().editor.session.selection();
    let ticket = h.state().editor.session.ticket();
    h.get_by_role_and_label(Role::Button, "Color picker")
        .click();
    h.run();
    click(&mut h, 40.0, 50.0, PointerButton::Secondary);
    assert_eq!(h.state().controls.secondary, Rgba([255, 0, 0, 255]));
    assert_eq!(
        h.state().editor.tools.secondary,
        h.state().controls.secondary
    );
    assert_eq!(h.state().controls.primary, Rgba([255; 4]));
    assert_eq!(h.state().controls.tool, Tool::Pencil);
    assert_eq!(h.state().editor.session.selection(), selection);
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &h.state().editor.session.snapshot()
    ));
}

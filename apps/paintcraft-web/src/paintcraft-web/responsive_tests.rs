use crate::WebApp;
use eframe::egui::{self, Event, Modifiers, Vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn narrow_zoomed_canvas_can_reach_both_far_edges() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .with_max_steps(20)
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h.state_mut().zoom = 4.0;
    h.set_size(Vec2::new(320.0, 568.0));
    h.run();
    let screen = h.ctx.content_rect();
    for label in ["Scroll canvas right", "Scroll canvas down"] {
        let r = h.get_by_label(label).rect();
        assert!(screen.contains_rect(r), "{label}: {r:?}, screen={screen:?}");
    }
    let canvas = h.state().canvas.rect.unwrap();
    let p = screen.center() + Vec2::new(80.0, 60.0);
    h.event(Event::PointerMoved(p));
    h.event(Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::splat(-50_000.0),
        modifiers: Modifiers::NONE,
        source: egui::MouseWheelSource::Unknown,
        phase: egui::TouchPhase::Move,
    });
    h.run();
    let end = h.state().canvas.rect.unwrap();
    assert!(
        end.right() < screen.right() && end.bottom() < screen.bottom(),
        "before={canvas:?}, after={end:?}, screen={screen:?}"
    );
    assert!(
        h.get_by_label("Scroll canvas right")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        h.get_by_label("Scroll canvas down")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn middle_drag_pans_without_painting_or_changing_the_selected_brush() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(390.0, 740.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h.state_mut().zoom = 4.0;
    h.run();
    let before = h.state().canvas.rect.unwrap();
    let ticket = h.state().editor.session.ticket();
    let p = egui::pos2(270.0, 360.0);
    h.event(Event::PointerMoved(p));
    h.run();
    h.event(Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Middle,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.step();
    let target = p - Vec2::new(80.0, 45.0);
    h.event(Event::PointerMoved(target));
    h.step();
    h.event(Event::PointerButton {
        pos: target,
        button: egui::PointerButton::Middle,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run();
    let after = h.state().canvas.rect.unwrap();
    assert!(
        (before.left() - after.left() - 80.0).abs() < 1.0,
        "{before:?} -> {after:?}"
    );
    assert!((before.top() - after.top() - 45.0).abs() < 1.0);
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert_eq!(h.state().controls.tool, paintcraft_tools::Tool::Brush);
    assert!(!h.state().editor.tools.gesture_in_progress());
}

#[test]
fn studio_keeps_host_controls_at_the_same_upper_right_position() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let classic = h.get_by_label("Settings…").rect();
    h.state_mut().frontend = paintcraft_ui::actions::Frontend::Studio;
    h.run();
    let studio = h.get_by_label("Settings…").rect();
    assert!((classic.right() - studio.right()).abs() < 1.0);
    assert!(studio.bottom() < 50.0);
    assert!(
        h.get_by_role_and_label(egui::Role::Button, "Interface")
            .rect()
            .right()
            < studio.left()
    );
}

#[test]
fn two_finger_pan_moves_the_view_without_publishing_a_stroke() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(390.0, 740.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h.state_mut().zoom = 4.0;
    h.run();
    let before = h.state().canvas.rect.unwrap();
    let ticket = h.state().editor.session.ticket();
    h.event(Event::PointerMoved(egui::pos2(260.0, 370.0)));
    h.run();
    let touch = |id, phase, x, y| Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos: egui::pos2(x, y),
        force: None,
    };
    h.event(touch(1, egui::TouchPhase::Start, 240.0, 350.0));
    h.event(touch(2, egui::TouchPhase::Start, 280.0, 390.0));
    h.step();
    h.event(touch(1, egui::TouchPhase::Move, 180.0, 320.0));
    h.event(touch(2, egui::TouchPhase::Move, 220.0, 360.0));
    h.step();
    h.event(touch(1, egui::TouchPhase::End, 180.0, 320.0));
    h.event(touch(2, egui::TouchPhase::End, 220.0, 360.0));
    h.run();
    let after = h.state().canvas.rect.unwrap();
    assert!(
        (before.left() - after.left() - 60.0).abs() < 1.0,
        "{before:?} -> {after:?}"
    );
    assert!((before.top() - after.top() - 30.0).abs() < 1.0);
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(!h.state().editor.tools.gesture_in_progress());
}

#[test]
fn mobile_header_controls_remain_reachable_in_every_language() {
    use paintcraft_ui::i18n::{Language, set_language, translate};
    for language in Language::ALL {
        let mut h = Harness::builder()
            .with_size(Vec2::new(320.0, 568.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        set_language(&h.ctx, language);
        h.run();
        for label in ["Settings…", "Interface"] {
            let rect = h.get_by_label(&translate(language, label)).rect();
            assert!(
                h.ctx.content_rect().contains_rect(rect),
                "{language:?} {label}: {rect:?}"
            );
        }
        let rect = h.get_by_label(&translate(language, "Canvas zoom")).rect();
        assert!(
            h.ctx.content_rect().contains_rect(rect),
            "{language:?} zoom: {rect:?}"
        );
    }
}

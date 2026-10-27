//! Real eframe event ordering, including synthetic releases before Touch End.
use crate::WebApp;
use eframe::egui::{self, Event, Pos2, TouchPhase, Vec2};
use egui_kittest::Harness;
use paintcraft_ui::actions::Frontend;

fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    }
}
fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
pub(crate) fn canvas_point(h: &Harness<WebApp>, margin: f32) -> Pos2 {
    let (viewport, layer) = h.state().canvas.touch.hit_region().unwrap();
    let rect = viewport
        .intersect(h.state().canvas.rect.unwrap())
        .shrink(margin);
    for y in [0.5, 0.7, 0.3, 0.9, 0.1] {
        for x in [0.5, 0.7, 0.3, 0.9, 0.1] {
            let pos = rect.min + rect.size() * Vec2::new(x, y);
            if [-margin, 0.0, margin].into_iter().all(|dx| {
                let p = pos + Vec2::new(dx, 0.0);
                h.ctx.layer_id_at(p) == Some(layer)
                    && h.state().canvas.controls.is_none_or(|r| !r.contains(p))
            }) {
                return pos;
            }
        }
    }
    panic!("no unobstructed canvas point: {rect:?}");
}
pub(crate) fn start(h: &mut Harness<WebApp>, id: u64, p: Pos2) {
    h.input_mut().events.extend([
        Event::PointerMoved(p),
        pointer(p, true),
        touch(id, TouchPhase::Start, p),
    ]);
    h.step();
}
pub(crate) fn end(h: &mut Harness<WebApp>, id: u64, p: Pos2) {
    h.input_mut().events.extend([
        pointer(p, false),
        Event::PointerGone,
        touch(id, TouchPhase::End, p),
    ]);
    h.step();
}
#[test]
fn pinch_and_moving_midpoint_anchor_without_stroke_or_tail_for_both_owners() {
    for frontend in [Frontend::Classic, Frontend::Studio] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(600.0, 800.0))
            .with_max_steps(30)
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.run();
        h.state_mut().frontend = frontend;
        h.state_mut().zoom = 4.0;
        h.run();
        let center = canvas_point(&h, 70.0);
        let a = center - Vec2::new(50.0, 0.0);
        let b = center + Vec2::new(50.0, 0.0);
        let ticket = h.state().editor.session.ticket();
        start(&mut h, 1, a);
        let anchor = ((a + (b - a) * 0.5) - h.state().canvas.rect.unwrap().min) / h.state().zoom;
        start(&mut h, 2, b);
        h.run();
        let a2 = a + Vec2::new(-60.0, 20.0);
        let b2 = b + Vec2::new(40.0, 20.0);
        h.input_mut().events.extend([
            Event::PointerMoved(a2),
            touch(1, TouchPhase::Move, a2),
            touch(2, TouchPhase::Move, b2),
        ]);
        h.run();
        assert!(
            (h.state().zoom - 8.0).abs() < 0.001,
            "{frontend:?}: {}",
            h.state().zoom
        );
        let expected = a2 + (b2 - a2) * 0.5;
        let actual = h.state().canvas.rect.unwrap().min + anchor * h.state().zoom;
        assert!(
            actual.distance(expected) < 1.1,
            "{frontend:?}: {actual:?} != {expected:?}"
        );
        end(&mut h, 2, b2);
        h.event(Event::PointerMoved(a2 + Vec2::splat(20.0)));
        h.event(touch(1, TouchPhase::Move, a2 + Vec2::splat(20.0)));
        h.step();
        end(&mut h, 1, a2);
        h.run();
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert!(!h.state().editor.tools.gesture_in_progress());
    }
}
#[test]
fn cancel_only_releases_pointer_and_a_fresh_stroke_still_works() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 800.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let p = canvas_point(&h, 0.0);
    let ticket = h.state().editor.session.ticket();
    start(&mut h, 1, p);
    h.event(touch(1, TouchPhase::Cancel, p));
    h.run();
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(!h.ctx.input(|i| i.pointer.primary_down()));
    start(&mut h, 2, p);
    end(&mut h, 2, p);
    h.run();
    assert_ne!(h.state().editor.session.ticket(), ticket);
}
#[test]
fn coalesced_two_finger_tap_never_activates_magnifier_or_text() {
    for tool in [
        paintcraft_tools::Tool::Magnifier,
        paintcraft_tools::Tool::Text,
        paintcraft_tools::Tool::Brush,
    ] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(600.0, 800.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.run();
        h.state_mut().controls.tool = tool;
        let zoom = h.state().zoom;
        let ticket = h.state().editor.session.ticket();
        let p = canvas_point(&h, 0.0);
        for id in [1, 2] {
            h.input_mut()
                .events
                .extend([pointer(p, true), touch(id, TouchPhase::Start, p)]);
        }
        for id in [1, 2] {
            h.input_mut().events.extend([
                pointer(p, false),
                Event::PointerGone,
                touch(id, TouchPhase::End, p),
            ]);
        }
        h.run();
        assert_eq!(h.state().zoom, zoom);
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert!(!h.state().editor.has_text());
    }
}

#[test]
fn a_single_touch_still_activates_one_shot_tools_on_release() {
    for tool in [
        paintcraft_tools::Tool::Magnifier,
        paintcraft_tools::Tool::Text,
    ] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(600.0, 800.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.run();
        h.state_mut().controls.tool = tool;
        let p = canvas_point(&h, 0.0);
        let zoom = h.state().zoom;
        start(&mut h, 1, p);
        assert_eq!(h.state().zoom, zoom);
        assert!(!h.state().editor.has_text());
        end(&mut h, 1, p);
        h.run();
        if tool == paintcraft_tools::Tool::Magnifier {
            assert!(h.state().zoom > zoom);
        } else {
            assert!(h.state().editor.tools.is_editing_text());
        }
    }
}

#[test]
fn pinch_keeps_existing_text_draft_unpublished() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 800.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h.state_mut().controls.tool = paintcraft_tools::Tool::Text;
    let layer = h.state().editor.target.unwrap();
    h.state_mut().editor.tools.begin_text(layer, (0, 0));
    h.state_mut().editor.tools.type_text("Keep editable");
    h.run();
    let ticket = h.state().editor.session.ticket();
    let p = canvas_point(&h, 40.0);
    start(&mut h, 1, p);
    start(&mut h, 2, p + Vec2::new(30.0, 0.0));
    end(&mut h, 2, p + Vec2::new(30.0, 0.0));
    end(&mut h, 1, p);
    h.run();
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert_eq!(h.state().editor.tools.text_content(), Some("Keep editable"));
}

#[test]
fn deferred_taps_are_retired_by_a_blocking_picker_or_tool_changes() {
    for modal in [false, true] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(600.0, 800.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.run();
        h.state_mut().controls.tool = paintcraft_tools::Tool::Text;
        h.run();
        let p = canvas_point(&h, 0.0);
        let ticket = h.state().editor.session.ticket();
        start(&mut h, 1, p);
        if modal {
            h.state_mut().files.busy = true;
        } else {
            h.state_mut().controls.tool = paintcraft_tools::Tool::Magnifier;
        }
        let zoom = h.state().zoom;
        end(&mut h, 1, p);
        h.run();
        assert_eq!(h.state().zoom, zoom);
        assert!(!h.state().editor.tools.is_editing_text());
        assert_eq!(h.state().editor.session.ticket(), ticket);
    }
}
#[test]
fn focus_loss_without_cancel_retires_canvas_contact_and_allows_fresh_mouse() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 800.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let p = canvas_point(&h, 0.0);
    let ticket = h.state().editor.session.ticket();
    start(&mut h, 1, p);
    h.input_mut().focused = false;
    h.input_mut().events.push(Event::WindowFocused(false));
    h.step();
    assert_eq!(h.state().editor.session.ticket(), ticket);
    h.input_mut().focused = true;
    h.input_mut().events.push(Event::WindowFocused(true));
    h.step();
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), pointer(p, true)]);
    h.step();
    h.input_mut().events.push(pointer(p, false));
    h.run();
    assert_ne!(h.state().editor.session.ticket(), ticket);
}
#[test]
fn clamp_and_contact_replacement_keep_finite_zoom_and_no_publication() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 800.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let p = canvas_point(&h, 0.0);
    let ticket = h.state().editor.session.ticket();
    start(&mut h, 1, p);
    start(&mut h, 2, p + Vec2::new(10.0, 0.0));
    h.input_mut()
        .events
        .push(touch(2, TouchPhase::Move, p + Vec2::new(10000.0, 0.0)));
    h.step();
    assert_eq!(h.state().zoom, 10.0);
    h.input_mut()
        .events
        .push(touch(2, TouchPhase::Move, p + Vec2::new(0.001, 0.0)));
    h.step();
    assert_eq!(h.state().zoom, 0.125);
    end(&mut h, 2, p);
    start(&mut h, 3, p + Vec2::new(100.0, 0.0));
    assert_eq!(h.state().zoom, 0.125);
    h.input_mut()
        .events
        .push(touch(3, TouchPhase::Move, egui::pos2(f32::NAN, 0.0)));
    h.step();
    assert!(h.state().zoom.is_finite());
    end(&mut h, 3, p);
    end(&mut h, 1, p);
    h.run();
    assert_eq!(h.state().editor.session.ticket(), ticket);
}

#[test]
fn two_contacts_on_chrome_do_not_take_canvas_zoom_ownership() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 800.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let zoom = h.state().zoom;
    let ticket = h.state().editor.session.ticket();
    let a = egui::pos2(300.0, 8.0);
    let b = egui::pos2(340.0, 8.0);
    start(&mut h, 1, a);
    start(&mut h, 2, b);
    h.input_mut().events.extend([
        touch(1, TouchPhase::Move, a - egui::vec2(50.0, 0.0)),
        touch(2, TouchPhase::Move, b + egui::vec2(50.0, 0.0)),
    ]);
    h.step();
    end(&mut h, 2, b);
    end(&mut h, 1, a);
    h.run();
    assert_eq!(h.state().zoom, zoom);
    assert_eq!(h.state().editor.session.ticket(), ticket);
}

#[test]
fn pinch_takes_over_an_already_started_magnifier_center_animation() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 800.0))
        .with_max_steps(40)
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h.state_mut().frontend = Frontend::Studio;
    h.state_mut().zoom = 2.0;
    h.state_mut().controls.tool = paintcraft_tools::Tool::Magnifier;
    h.run();
    let p = canvas_point(&h, 70.0);
    start(&mut h, 1, p);
    end(&mut h, 1, p);
    // Consume pending_center into egui's animation, but do not wait it out.
    h.step();
    h.state_mut().controls.tool = paintcraft_tools::Tool::Brush;
    let center = canvas_point(&h, 70.0);
    let a = center - Vec2::new(40.0, 0.0);
    let b = center + Vec2::new(40.0, 0.0);
    start(&mut h, 2, a);
    let anchor = (center - h.state().canvas.rect.unwrap().min) / h.state().zoom;
    start(&mut h, 3, b);
    let a2 = a - Vec2::new(20.0, 0.0);
    let b2 = b + Vec2::new(20.0, 0.0);
    h.input_mut().events.extend([
        touch(2, TouchPhase::Move, a2),
        touch(3, TouchPhase::Move, b2),
    ]);
    h.run();
    let actual = h.state().canvas.rect.unwrap().min + anchor * h.state().zoom;
    assert!(actual.distance(center) < 1.1, "{actual:?} != {center:?}");
    end(&mut h, 3, b2);
    end(&mut h, 2, a2);
    h.run();
    let later = h.state().canvas.rect.unwrap().min + anchor * h.state().zoom;
    assert!(later.distance(center) < 1.1, "late {later:?} != {center:?}");
}

use crate::WebApp;
use eframe::egui::{Event, Key, Modifiers, PointerButton, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_tools::{Preview, Tool};

fn harness() -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .with_max_steps(12)
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h
}
fn edge(h: &mut Harness<WebApp>, a: (f32, f32), b: (f32, f32), finish: bool) {
    let rect = h.state().canvas.rect.unwrap();
    let a = rect.min + Vec2::new(a.0 + 0.5, a.1 + 0.5) * h.state().zoom;
    let b = rect.min + Vec2::new(b.0 + 0.5, b.1 + 0.5) * h.state().zoom;
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
    if finish {
        h.event(Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
    }
    h.run();
}

#[test]
fn polygon_coalesced_last_vertex_and_enter_publish_once_and_undo() {
    let mut h = harness();
    h.get_by_role_and_label(Role::Button, "Polygon").click();
    h.run();
    assert_eq!(h.state().controls.tool, Tool::Polygon);
    let ticket = h.state().editor.session.ticket();
    let base = h.state().editor.session.snapshot();
    edge(&mut h, (20.0, 20.0), (90.0, 20.0), false);
    assert!(h.state().editor.tools.is_constructing_shape());
    assert_eq!(h.state().editor.session.ticket(), ticket);
    edge(&mut h, (90.0, 90.0), (90.0, 90.0), true);
    assert_eq!(h.state().editor.session.undo_depth(), 1);
    assert!(!h.state().editor.tools.gesture_in_progress());
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor.session.document().same_semantics(&base));
}

#[test]
fn curve_survives_two_releases_then_last_handle_matches_release_pixels() {
    let mut h = harness();
    h.get_by_role_and_label(Role::Button, "Curve").click();
    h.run();
    let base = h.state().editor.session.snapshot();
    edge(&mut h, (20.0, 20.0), (90.0, 20.0), false);
    edge(&mut h, (20.0, 100.0), (20.0, 100.0), false);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    h.state_mut().editor.tools.pointer_move((90, 100));
    let preview = h.state().editor.display_document().unwrap().unwrap();
    assert!(matches!(
        h.state().editor.tools.preview(),
        Some(Preview::Contour(_))
    ));
    edge(&mut h, (90.0, 100.0), (90.0, 100.0), false);
    assert_eq!(h.state().editor.session.undo_depth(), 1);
    assert_eq!(
        paintcraft_render::render_flat(&preview).unwrap(),
        paintcraft_render::render_flat(h.state().editor.session.document()).unwrap()
    );
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor.session.document().same_semantics(&base));
}

#[test]
fn focus_loss_and_escape_cancel_only_unpublished_construction() {
    for focus in [true, false] {
        let mut h = harness();
        h.get_by_role_and_label(Role::Button, "Polygon").click();
        h.run();
        let before = h.state().editor.session.ticket();
        edge(&mut h, (20.0, 20.0), (90.0, 20.0), false);
        h.event(if focus {
            Event::WindowFocused(false)
        } else {
            Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }
        });
        h.run();
        assert!(!h.state().editor.tools.gesture_in_progress());
        assert_eq!(h.state().editor.session.ticket(), before);
        assert_eq!(h.state().editor.session.undo_depth(), 0);
    }
}

#[test]
fn historical_classic_skins_offer_presets_and_multistep_tools_too() {
    for variant in [
        paintcraft_classic::ClassicVariant::V7,
        paintcraft_classic::ClassicVariant::V10,
    ] {
        let mut h = harness();
        h.state_mut().variant = variant;
        h.run();
        h.get_by_role_and_label(Role::Button, "Rounded rectangle")
            .click();
        h.run();
        assert_eq!(
            h.state().controls.tool,
            Tool::Preset(paintcraft_tools::ShapePreset::RoundedRectangle)
        );
        h.get_by_role_and_label(Role::Button, "Curve").click();
        h.run();
        assert_eq!(h.state().controls.tool, Tool::Curve);
    }
}

#[test]
fn save_and_open_do_not_discard_or_download_an_unfinished_shape() {
    let mut h = harness();
    h.get_by_role_and_label(Role::Button, "Polygon").click();
    h.run();
    edge(&mut h, (20.0, 20.0), (90.0, 20.0), false);
    let ticket = h.state().editor.session.ticket();
    for key in [Key::S, Key::O, Key::N] {
        h.key_press_modifiers(Modifiers::COMMAND, key);
        h.run();
        assert!(h.state().editor.tools.is_constructing_shape());
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert_eq!(
            h.state().message,
            paintcraft_tools::FINISH_SHAPE_BEFORE_FILE
        );
    }
}

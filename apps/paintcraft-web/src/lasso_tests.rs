//! Browser controls must create and consume the same finite masks as native tools.
use crate::WebApp;
use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_classic::ClassicVariant;
use paintcraft_core::types::Rgba;
use paintcraft_raster::Selection;
use paintcraft_tools::{Action, SelectionMode};
use paintcraft_ui::i18n::{Language, set_language, translate};

fn inside(x: i64, y: i64) -> bool {
    (4..28).contains(&x) && (4..12).contains(&y) || (4..12).contains(&x) && (12..28).contains(&y)
}
fn check_mask(selection: Selection, inverted: bool) {
    let Selection::Mask(mask) = selection else {
        panic!("expected actual lasso mask");
    };
    for y in 0..32 {
        for x in 0..32 {
            assert_eq!(
                mask.coverage(x, y) != 0,
                inside(x, y) != inverted,
                "mask pixel {x},{y}"
            );
        }
    }
}
fn point(h: &Harness<WebApp>, x: i64, y: i64) -> Pos2 {
    h.state().canvas.rect.unwrap().min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * h.state().zoom
}
fn check_outline(h: &Harness<WebApp>) {
    let rect = h.state().canvas.rect.unwrap();
    let zoom = h.state().zoom;
    let boundary = |x: f32, y: f32| rect.min + Vec2::new(x, y) * zoom;
    let line = |a: Pos2, b: Pos2| {
        h.output().shapes.iter().any(|shape|matches!(&shape.shape,
        eframe::egui::Shape::LineSegment {points,stroke} if stroke.color==eframe::egui::Color32::BLACK &&
        ((points[0].distance(a)<0.01&&points[1].distance(b)<0.01)||(points[1].distance(a)<0.01&&points[0].distance(b)<0.01))))
    };
    assert!(
        line(boundary(12.0, 12.0), boundary(28.0, 12.0)),
        "actual concave boundary is visible"
    );
    assert!(
        !line(boundary(4.0, 28.0), boundary(28.0, 28.0)),
        "no rectangular substitute outline"
    );
}
fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}
fn shortcut(h: &mut Harness<WebApp>, key: Key, modifiers: Modifiers) {
    for pressed in [true, false] {
        h.event(Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        });
    }
    h.run();
}
fn mode(h: &mut Harness<WebApp>, language: Language, free: bool) {
    h.get_by_role_and_label(Role::Button, &translate(language, "Selection options"))
        .click();
    h.run();
    h.get_by_role_and_label(
        Role::Button,
        &translate(
            language,
            if free {
                "Free-form selection"
            } else {
                "Rectangular selection"
            },
        ),
    )
    .click();
    h.run();
    assert_eq!(
        h.state().controls.options.selection_mode,
        if free {
            SelectionMode::FreeForm
        } else {
            SelectionMode::Rectangle
        }
    );
}
fn check_pixels(h: &Harness<WebApp>, painted: bool) {
    let editor = &h.state().editor;
    let layer = &editor.session.document().layers[&editor.target.unwrap()];
    for y in 0..32 {
        for x in 0..32 {
            let expected = if painted && y == 20 && !inside(x, y) {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255; 4])
            };
            assert_eq!(layer.raster.get(x, y), expected, "document pixel {x},{y}");
        }
    }
}

#[test]
fn lasso_and_invert_route_browser_menus_pointer_keyboard_and_history_in_all_skins_and_languages() {
    for language in Language::ALL {
        for variant in ClassicVariant::ALL {
            let mut h = Harness::builder()
                .with_size(Vec2::new(1360.0, 900.0))
                .with_pixels_per_point(if variant == ClassicVariant::V10 {
                    2.0
                } else {
                    1.0
                })
                .build_ui_state(
                    |ui, app: &mut WebApp| {
                        app.show(ui);
                        if ui.ctx().current_pass_index() == 0 {
                            ui.ctx()
                                .request_discard("test: repeated browser lasso layout");
                        }
                    },
                    {
                        let mut app = WebApp::new(&eframe::egui::Context::default());
                        app.variant = variant;
                        app.edit(Action::ResizeCanvas {
                            width: 32,
                            height: 32,
                        });
                        app
                    },
                );
            set_language(&h.ctx, language);
            h.run();
            let source = h.state().editor.session.snapshot();
            let ticket = h.state().editor.session.ticket();
            let depth = h.state().editor.session.undo_depth();
            let dirty = h.state().editor.session.is_dirty();
            mode(&mut h, language, true);
            let a = point(&h, 4, 4);
            h.input_mut()
                .events
                .extend([Event::PointerMoved(a), button(a, true)]);
            h.run();
            for (x, y) in [(28, 4), (28, 12), (12, 12), (12, 28), (4, 28)] {
                let p = point(&h, x, y);
                h.event(Event::PointerMoved(p));
            }
            h.run();
            assert_eq!(h.state().editor.session.ticket(), ticket);
            assert_eq!(h.state().editor.session.undo_depth(), depth);
            assert!(std::sync::Arc::ptr_eq(
                &source,
                &h.state().editor.session.snapshot()
            ));
            let b = point(&h, 4, 28);
            h.event(button(b, false));
            h.run();
            check_mask(h.state().editor.session.selection(), false);
            check_outline(&h);
            assert_eq!(h.state().editor.session.is_dirty(), dirty);
            assert_eq!(h.state().editor.session.undo_depth(), depth + 1);
            assert!(!h.state().context().can_crop_selection);
            assert!(!h.state().context().can_transform_image);
            assert!(h.state().context().can_erase_selection);
            shortcut(&mut h, Key::I, Modifiers::COMMAND);
            check_mask(h.state().editor.session.selection(), true);
            assert_eq!(h.state().editor.session.is_dirty(), dirty);
            shortcut(&mut h, Key::Z, Modifiers::COMMAND);
            check_mask(h.state().editor.session.selection(), false);
            shortcut(&mut h, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
            check_mask(h.state().editor.session.selection(), true);
            h.get_by_role_and_label(Role::Button, &translate(language, "Pencil"))
                .click();
            h.run();
            let a = point(&h, 0, 20);
            let b = point(&h, 31, 20);
            h.input_mut().events.extend([
                Event::PointerMoved(a),
                button(a, true),
                Event::PointerMoved(b),
                button(b, false),
            ]);
            h.run();
            check_pixels(&h, true);
            shortcut(&mut h, Key::Delete, Modifiers::NONE);
            check_pixels(&h, false);
            check_mask(h.state().editor.session.selection(), true);
            shortcut(&mut h, Key::Z, Modifiers::COMMAND);
            check_pixels(&h, true);
            check_mask(h.state().editor.session.selection(), true);
            shortcut(&mut h, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
            check_pixels(&h, false);
            shortcut(&mut h, Key::D, Modifiers::COMMAND);
            assert_eq!(h.state().editor.session.selection(), Selection::Inactive);
            mode(&mut h, language, false);
            let a = point(&h, 6, 6);
            let b = point(&h, 10, 10);
            h.input_mut().events.extend([
                Event::PointerMoved(a),
                button(a, true),
                Event::PointerMoved(b),
                button(b, false),
            ]);
            h.run();
            assert_eq!(
                h.state().editor.session.selection(),
                Selection::Rect(paintcraft_core::types::Rect::new(6, 6, 5, 5).unwrap())
            );
        }
    }
}

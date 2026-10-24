//! Pointer-driven lasso coverage, rather than menu availability alone.
use super::*;
use eframe::egui::Role;
use paintcraft::settings::{ClassicVariant, Experience, Settings};
use paintcraft_raster::Selection;
use paintcraft_tools::{SelectionMode, Tool};
use paintcraft_ui::i18n::{Language, translate};

const VERTICES: [(i64, i64); 6] = [(4, 4), (28, 4), (28, 12), (12, 12), (12, 28), (4, 28)];
fn inside(x: i64, y: i64) -> bool {
    (4..28).contains(&x) && (4..12).contains(&y) || (4..12).contains(&x) && (12..28).contains(&y)
}
fn check_mask(selection: Selection, inverted: bool) {
    let Selection::Mask(mask) = selection else {
        panic!("expected actual selection mask");
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
fn point(h: &Harness<App>, x: i64, y: i64) -> Pos2 {
    let rect = h.state().canvas_rect().unwrap();
    rect.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * (rect.width() / 32.0)
}
fn check_outline(h: &Harness<App>) {
    let rect = h.state().canvas_rect().unwrap();
    let scale = rect.width() / 32.0;
    let boundary = |x: f32, y: f32| rect.min + Vec2::new(x, y) * scale;
    let line = |a: Pos2, b: Pos2| {
        h.output().shapes.iter().any(|shape|matches!(&shape.shape,
        eframe::egui::Shape::LineSegment {points,stroke} if stroke.color==eframe::egui::Color32::BLACK &&
        ((points[0].distance(a)<0.01&&points[1].distance(b)<0.01)||(points[1].distance(a)<0.01&&points[0].distance(b)<0.01))))
    };
    assert!(
        line(boundary(12.0, 12.0), boundary(28.0, 12.0)),
        "concave mask edge must be drawn"
    );
    assert!(
        !line(boundary(4.0, 28.0), boundary(28.0, 28.0)),
        "mask must not be drawn as its bounding rectangle"
    );
}
fn shortcut(h: &mut Harness<App>, key_code: Key, modifiers: Modifiers) {
    h.event(key(key_code, modifiers));
    h.event(key_up(key_code, modifiers));
    h.run();
}
fn mode(h: &mut Harness<App>, language: Language, studio: bool, free: bool) {
    let label = |s| translate(language, s);
    if studio {
        if h.state().editor().tool != Tool::Select {
            h.get_by_role_and_label(Role::Button, &label("Select"))
                .click();
            h.run();
        }
        h.get_by_label(&label("Selection shapes")).click();
        h.run();
        h.get_by_label(&label(if free {
            "Free-form selection"
        } else {
            "Rectangular selection"
        }))
        .click();
    } else {
        h.get_by_role_and_label(Role::Button, &label("Selection options"))
            .click();
        h.run();
        h.get_by_role_and_label(
            Role::Button,
            &label(if free {
                "Free-form selection"
            } else {
                "Rectangular selection"
            }),
        )
        .click();
    }
    h.run();
    assert_eq!(
        h.state().editor().options.selection_mode,
        if free {
            SelectionMode::FreeForm
        } else {
            SelectionMode::Rectangle
        }
    );
}
fn transparent(h: &mut Harness<App>, language: Language, studio: bool) {
    if !studio {
        h.get_by_role_and_label(Role::Button, &translate(language, "Selection options"))
            .click();
        h.run();
    }
    h.get_by_label(&translate(language, "Transparent selection"))
        .scroll_to_me();
    h.run();
    h.get_by_label(&translate(language, "Transparent selection"))
        .click();
    h.run();
}
fn check_pixels(h: &Harness<App>, selected_color: Rgba, painted: bool) {
    for y in 0..32 {
        for x in 0..32 {
            let expected = if !inside(x, y) && (!painted || y == 20) {
                selected_color
            } else {
                Rgba([255; 4])
            };
            assert_eq!(top_pixel(h, x, y), expected, "document pixel {x},{y}");
        }
    }
}

#[test]
fn lasso_and_invert_route_real_pointer_keyboard_and_history_in_every_native_frontend_and_language()
{
    for language in Language::ALL {
        for classic in [
            Some(ClassicVariant::V7),
            Some(ClassicVariant::V10),
            Some(ClassicVariant::V11),
            None,
        ] {
            let dir = scratch("lasso-functional");
            let path = dir.join("source.pcnd");
            let document = paintcraft_document::new_document(
                32,
                32,
                Some(Rgba([255; 4])),
                9,
                paintcraft_document::POLICY,
            )
            .unwrap();
            std::fs::write(&path, paintcraft_formats::pcnd::write(&document).unwrap()).unwrap();
            Settings {
                experience: if classic.is_some() {
                    Experience::Classic
                } else {
                    Experience::Studio
                },
                classic_variant: classic,
                language,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(
                App::new(Some(path)).with_settings(dir.clone()),
                if classic == Some(ClassicVariant::V10) {
                    2.0
                } else {
                    1.0
                },
                true,
            );
            let original = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            let depth = h.state().editor().undo_depth();
            mode(&mut h, language, classic.is_none(), true);
            assert!(!h.state().editor().options.transparent_selection);
            transparent(&mut h, language, classic.is_none());
            assert!(h.state().editor().options.transparent_selection);
            transparent(&mut h, language, classic.is_none());
            assert!(!h.state().editor().options.transparent_selection);
            let first = point(&h, 4, 4);
            h.input_mut()
                .events
                .extend([Event::PointerMoved(first), press(first)]);
            h.run();
            for (x, y) in VERTICES.into_iter().skip(1) {
                let pos = point(&h, x, y);
                h.event(Event::PointerMoved(pos));
            }
            h.run();
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert_eq!(h.state().editor().undo_depth(), depth);
            assert!(std::sync::Arc::ptr_eq(
                &original,
                &h.state().editor().document_snapshot()
            ));
            let last = point(&h, 4, 28);
            h.event(release(last));
            h.run();
            check_mask(h.state().editor().selection(), false);
            check_outline(&h);
            h.get_by_label(&format!(
                "{}: 24 × 24 px",
                translate(language, "Selection size")
            ));
            assert!(
                !h.state().editor().is_dirty(),
                "selection must not dirty saved pixels"
            );
            assert_eq!(h.state().editor().undo_depth(), depth + 1);
            shortcut(&mut h, Key::I, Modifiers::COMMAND);
            check_mask(h.state().editor().selection(), true);
            h.get_by_label(&format!(
                "{}: 32 × 32 px",
                translate(language, "Selection size")
            ));
            assert!(!h.state().editor().is_dirty());
            shortcut(&mut h, Key::Z, Modifiers::COMMAND);
            check_mask(h.state().editor().selection(), false);
            shortcut(&mut h, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
            check_mask(h.state().editor().selection(), true);
            let label = |s| translate(language, s);
            h.get_by_role_and_label(Role::Button, &label("Pencil"))
                .click();
            h.run();
            let a = point(&h, 0, 20);
            let b = point(&h, 31, 20);
            h.input_mut().events.extend([
                Event::PointerMoved(a),
                press(a),
                Event::PointerMoved(b),
                release(b),
            ]);
            h.run();
            check_pixels(&h, Rgba([0, 0, 0, 255]), true);
            shortcut(&mut h, Key::Delete, Modifiers::NONE);
            check_pixels(
                &h,
                if classic.is_some() {
                    Rgba([255; 4])
                } else {
                    Rgba::TRANSPARENT
                },
                false,
            );
            check_mask(h.state().editor().selection(), true);
            shortcut(&mut h, Key::Z, Modifiers::COMMAND);
            check_pixels(&h, Rgba([0, 0, 0, 255]), true);
            check_mask(h.state().editor().selection(), true);
            shortcut(&mut h, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
            check_pixels(
                &h,
                if classic.is_some() {
                    Rgba([255; 4])
                } else {
                    Rgba::TRANSPARENT
                },
                false,
            );
            shortcut(&mut h, Key::D, Modifiers::COMMAND);
            assert_eq!(h.state().editor().selection(), Selection::Inactive);
            mode(&mut h, language, classic.is_none(), false);
            let a = point(&h, 6, 6);
            let b = point(&h, 10, 10);
            h.input_mut().events.extend([
                Event::PointerMoved(a),
                press(a),
                Event::PointerMoved(b),
                release(b),
            ]);
            h.run();
            assert_eq!(
                h.state().editor().selection(),
                Selection::Rect(paintcraft_core::types::Rect::new(6, 6, 5, 5).unwrap())
            );
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn classic_move_commits_on_release_and_studio_regrabs_then_enter_commits_once() {
    for studio in [false, true] {
        let dir = scratch("floating-selection-native");
        let path = dir.join("source.pcnd");
        let doc = paintcraft_document::new_document(
            32,
            32,
            Some(Rgba([255; 4])),
            17,
            paintcraft_document::POLICY,
        )
        .unwrap();
        std::fs::write(&path, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
        Settings {
            experience: if studio {
                Experience::Studio
            } else {
                Experience::Classic
            },
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(Some(path)).with_settings(dir.clone()), 1.0, true);
        h.get_by_role_and_label(Role::Button, "Pencil").click();
        h.run();
        for (x, y) in [(6, 6), (20, 20)] {
            let p = point(&h, x, y);
            h.event(Event::PointerMoved(p));
            h.event(press(p));
            h.event(release(p));
            h.run();
        }
        mode(&mut h, Language::English, studio, false);
        let a = point(&h, 4, 4);
        let b = point(&h, 11, 11);
        h.event(Event::PointerMoved(a));
        h.event(press(a));
        h.event(Event::PointerMoved(b));
        h.event(release(b));
        h.run();
        let selection = h.state().editor().selection();
        let before = h.state().editor().document_snapshot();
        let depth = h.state().editor().undo_depth();
        let ticket = h.state().editor().history_view().ticket;
        let a = point(&h, 6, 6);
        h.event(Event::PointerMoved(a));
        h.run();
        assert_eq!(
            h.output().platform_output.cursor_icon,
            eframe::egui::CursorIcon::Move
        );
        let b = point(&h, 9, 8);
        h.event(Event::PointerMoved(a));
        h.event(press(a));
        h.event(Event::PointerMoved(b));
        h.run();
        assert!(std::sync::Arc::ptr_eq(
            &before,
            &h.state().editor().document_snapshot()
        ));
        assert_eq!(
            h.state().editor().display_selection(),
            Selection::Rect(paintcraft_core::types::Rect::new(7, 6, 8, 8).unwrap())
        );
        h.event(release(b));
        h.run();
        let destination = if studio {
            assert!(h.state().editor().has_floating_selection());
            assert!(std::sync::Arc::ptr_eq(
                &before,
                &h.state().editor().document_snapshot()
            ));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert_eq!(h.state().editor().undo_depth(), depth);
            // Two identical Undo activations in one frame cancel the draft once.
            for pressed in [true, true, false] {
                h.event(Event::Key {
                    key: Key::Z,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                });
            }
            h.run();
            assert!(!h.state().editor().has_floating_selection());
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert_eq!(h.state().editor().undo_depth(), depth);
            assert_eq!(h.state().editor().selection(), selection);
            let a = point(&h, 6, 6);
            let b = point(&h, 9, 8);
            h.event(Event::PointerMoved(a));
            h.event(press(a));
            h.event(Event::PointerMoved(b));
            h.event(release(b));
            h.run();
            assert!(h.state().editor().has_floating_selection());
            h.get_by_label("2×").click();
            h.run();
            assert!(
                h.state().editor().has_floating_selection(),
                "zoom preserves the floating draft"
            );
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            let a = point(&h, 9, 8);
            let b = point(&h, 11, 7);
            h.event(Event::PointerMoved(a));
            h.event(press(a));
            h.event(Event::PointerMoved(b));
            h.event(release(b));
            h.run();
            assert!(h.state().editor().has_floating_selection());
            assert_eq!(h.state().editor().undo_depth(), depth);
            assert_eq!(
                h.state().editor().display_selection(),
                Selection::Rect(paintcraft_core::types::Rect::new(9, 5, 8, 8).unwrap())
            );
            h.key_press(Key::Enter);
            h.run();
            (11, 7)
        } else {
            (9, 8)
        };
        assert!(!h.state().editor().has_floating_selection());
        assert_eq!(h.state().editor().undo_depth(), depth + 1);
        assert_eq!(
            top_pixel(&h, destination.0, destination.1),
            Rgba([0, 0, 0, 255])
        );
        assert_eq!(
            top_pixel(&h, 6, 6),
            if studio {
                Rgba::TRANSPARENT
            } else {
                Rgba([255; 4])
            }
        );
        assert_eq!(top_pixel(&h, 20, 20), Rgba([0, 0, 0, 255]));
        h.get_by_label("Undo").click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&before));
        assert_eq!(h.state().editor().selection(), selection);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

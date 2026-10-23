use super::*;
use eframe::egui::{Color32, Rect, Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

struct Scene {
    clip: Rect,
    canvas: Rect,
}

#[test]
fn classic_canvas_rails_stay_outside_pixels_and_keep_native_thumb_navigation() {
    for (ruled, ppp, dark) in [(false, 1.0, false), (true, 2.0, true)] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(400.0, 300.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(
                move |ui, scene: &mut Scene| {
                    ui.ctx().set_visuals(if dark {
                        eframe::egui::Visuals::dark()
                    } else {
                        eframe::egui::Visuals::light()
                    });
                    let id = eframe::egui::Id::unique("classic-scrollbars");
                    let paint = |_: &mut eframe::egui::Ui,
                                 response: eframe::egui::Response,
                                 painter: eframe::egui::Painter| {
                        painter.rect_filled(response.rect, 0, Color32::WHITE);
                        (painter.clip_rect(), response.rect)
                    };
                    let (clip, canvas) = if ruled {
                        paintcraft_classic::canvas::ruled_canvas(
                            ui,
                            id,
                            Vec2::new(2000.0, 1500.0),
                            (2000, 1500),
                            0.0,
                            paint,
                        )
                    } else {
                        paintcraft_classic::canvas::canvas(
                            ui,
                            id,
                            Vec2::new(2000.0, 1500.0),
                            0.0,
                            paint,
                        )
                    };
                    scene.clip = clip;
                    scene.canvas = canvas;
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx()
                            .request_discard("test: repeated canvas rail layout");
                    }
                },
                Scene {
                    clip: Rect::NOTHING,
                    canvas: Rect::NOTHING,
                },
            );
        h.run();
        for label in ["Scroll canvas left", "Scroll canvas up"] {
            assert!(h.get_by_label(label).accesskit_node().is_disabled());
        }
        for (label, axis) in [("Scroll canvas right", 0), ("Scroll canvas down", 1)] {
            let button = h.get_by_role_and_label(Role::Button, label).rect();
            assert!(!h.state().clip.intersects(button.shrink(0.1)));
            let before = h.state().canvas.min;
            h.get_by_label(label).click();
            h.run();
            assert!(
                (before[axis] - h.state().canvas.min[axis] - 40.0).abs() < 0.1,
                "{label}: before={before:?}, after={:?}, ruled={ruled}, ppp={ppp}",
                h.state().canvas.min
            );
        }
        let bar = h
            .get_by_role_and_label(Role::ScrollBar, "Vertical canvas scrollbar")
            .rect();
        assert!(h.state().clip.right() <= bar.left());
        let horizontal = h
            .get_by_role_and_label(Role::ScrollBar, "Horizontal canvas scrollbar")
            .rect();
        assert!(h.state().clip.bottom() <= horizontal.top());
        let thumb = h
            .output()
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Rect(rect)
                    if bar.contains_rect(rect.rect)
                        && rect.rect.width() <= 7.0
                        && rect.rect.height() > 14.0
                        && rect.fill.a() > 0 =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .max_by(|a, b| a.height().total_cmp(&b.height()))
            .expect("visible native scroll thumb");
        let before = h.state().canvas.top();
        let from = thumb.center();
        let to = from + Vec2::new(0.0, 20.0);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(from), press(from)]);
        h.step();
        h.event(Event::PointerMoved(to));
        h.step();
        h.event(release(to));
        h.run();
        assert!(
            h.state().canvas.top() < before - 20.0,
            "thumb drag must pan the canvas"
        );
        let arrow = h.get_by_label("Scroll canvas right").rect().center();
        let before = h.state().canvas.left();
        h.input_mut()
            .events
            .extend([Event::PointerMoved(arrow), press(arrow)]);
        h.step();
        assert!((before - h.state().canvas.left() - 40.0).abs() < 0.1);
        let now = h.ctx.input(|input| input.time);
        h.input_mut().time = Some(now + 0.5);
        h.step();
        assert!((before - h.state().canvas.left() - 80.0).abs() < 0.1);
        let after_repeat = h.state().canvas.left();
        h.event(release(arrow));
        h.run();
        assert_eq!(
            h.state().canvas.left(),
            after_repeat,
            "release must not add a step"
        );
    }
}

#[test]
fn classic_canvas_arrow_navigation_preserves_document_history_and_preferences() {
    let dir = scratch("canvas-arrow-isolation");
    Settings {
        appearance: Appearance::Dark,
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
    h.get_by_label("Canvas zoom").set_value_accesskit(400.0);
    h.run();
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    let saved = std::fs::read(dir.join("settings.json")).unwrap();
    for label in [
        "Scroll canvas right",
        "Scroll canvas down",
        "Scroll canvas left",
        "Scroll canvas up",
    ] {
        h.get_by_label(label).click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        assert!(!h.state().editor().gesture_in_progress());
    }
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "Explicit GPU Classic canvas rail review"]
fn export_classic_canvas_rails() {
    let output = std::env::temp_dir().join("paintcraft-canvas-rails");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (name, size, zoom, ruled) in [
            ("vertical", Vec2::new(1360.0, 700.0), 100.0, false),
            ("both", Vec2::new(900.0, 600.0), 200.0, false),
            ("ruled", Vec2::new(900.0, 600.0), 200.0, true),
            ("compact", Vec2::new(480.0, 360.0), 100.0, false),
        ] {
            let dir = scratch("canvas-rail-review");
            Settings {
                appearance,
                classic_variant: Some(ClassicVariant::V11),
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(size);
            h.run();
            h.get_by_label("Canvas zoom").set_value_accesskit(zoom);
            h.run();
            if ruled {
                h.key_combination_modifiers(Modifiers::COMMAND, &[Key::R]);
                h.run();
            }
            let snapshot = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}.png")))
                .unwrap();
            if name == "both" {
                h.get_by_label("Scroll canvas right").click();
                h.run();
                h.get_by_label("Scroll canvas down").click();
                h.run();
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-panned.png")))
                    .unwrap();
            }
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

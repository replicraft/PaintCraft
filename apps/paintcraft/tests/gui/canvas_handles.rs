use super::*;
use eframe::egui::{Color32, CursorIcon, Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn classic_canvas_handles_stay_outside_pixels_and_explain_their_preview_state() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("classic-canvas-handles");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        h.get_by_label("Zoom percentage").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Zoom percentage").type_text("50%");
        h.run();
        h.key_press(Key::Enter);
        h.run();
        let canvas = h.state().canvas_rect().unwrap();
        let outer = canvas.expand(3.5);
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (name, point) in [
            ("top left", outer.left_top()),
            ("top", outer.center_top()),
            ("top right", outer.right_top()),
            ("right", outer.right_center()),
            ("bottom right", outer.right_bottom()),
            ("bottom", outer.center_bottom()),
            ("bottom left", outer.left_bottom()),
            ("left", outer.left_center()),
        ] {
            let label = format!("Resize canvas from {name}");
            let handle = h.get_by_role_and_label(Role::Button, &label);
            assert!(handle.accesskit_node().is_disabled());
            assert_eq!(handle.rect().center(), point);
            assert_eq!(handle.rect().size(), Vec2::splat(7.0));
            assert!(!canvas.contains(point));
            assert!(h.output().shapes.iter().any(|shape| {
                matches!(&shape.shape, Shape::Rect(marker)
                    if marker.rect.center() == point && marker.rect.size() == Vec2::splat(5.0)
                    && marker.fill == Color32::WHITE && marker.stroke.width > 0.0)
            }));
            handle.hover();
            h.run();
            assert_eq!(
                h.output().platform_output.cursor_icon,
                CursorIcon::NotAllowed
            );
            assert_eq!(h.query_all_by_role_and_label(Role::Label,
                "Canvas resize handles are not available yet. Use the View menu's Canvas size command.").count(), 1);
            h.get_by_role_and_label(Role::Button, &label).click();
            h.run();
            assert_eq!(h.state().canvas_rect(), Some(canvas));
            assert_eq!(h.state().editor().document().canvas, (1000, 486));
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert!(!h.state().editor().gesture_in_progress());
        }
        h.run_steps(8);
        assert_eq!(
            h.state().canvas_rect(),
            Some(canvas),
            "handle overlays must not change layout"
        );
        // Normal paint still reaches the image's first pixel beside a corner marker.
        let point = canvas.min + Vec2::splat(0.25 / ppp);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point)]);
        h.step();
        h.event(release(point));
        h.run();
        assert_eq!(top_pixel(&h, 0, 0), Rgba([0, 0, 0, 255]));
        assert_eq!(publications(&h), 1);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
    let (h, dir) = super::studio::studio_harness(true);
    assert!(
        h.query_by_role_and_label(Role::Button, "Resize canvas from left")
            .is_none(),
        "Studio keeps its own canvas presentation"
    );
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

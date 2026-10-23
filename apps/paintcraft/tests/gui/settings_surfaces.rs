use super::*;
use eframe::egui::{Role, accesskit::Toggled, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn classic_settings_uses_consistent_controls_and_close_cancels_preview() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1360.0, 900.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 360.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("settings-surfaces");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(size);
        h.run();
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_label("Settings…").click();
        h.run();
        for label in [
            "Appearance",
            "Canvas",
            "Text preferences",
            "Apply",
            "Cancel",
            "Close settings",
        ] {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            assert_eq!(rect.height(), 40.0, "{label}");
            assert!(
                h.ctx.content_rect().contains_rect(rect),
                "{label}: {rect:?}"
            );
        }
        assert!(
            (h.get_by_label("Close settings").rect().right()
                - h.get_by_label("Cancel").rect().right())
            .abs()
                < 1.0,
            "close belongs at the upper right"
        );
        let selected = h.get_by_role_and_label(Role::Button, "Appearance");
        assert_eq!(selected.accesskit_node().toggled(), Some(Toggled::True));
        let bounds = selected.rect();
        assert!(
            h.output()
                .shapes
                .iter()
                .any(|shape| matches!(&shape.shape,Shape::Rect(rect)
            if rect.brush.is_none() && rect.rect.center().distance(bounds.center())<1.0
                && (rect.rect.size()-bounds.size()).length()<3.0 && rect.fill.a()>0
                && rect.fill.r()==rect.fill.g() && rect.fill.g()==rect.fill.b()
                && rect.stroke.color==h.ctx.style_of(h.ctx.theme()).visuals.selection.stroke.color))
        );
        let mut leading: Option<f32> = None;
        for label in [
            "Appearance",
            "Canvas",
            "Text preferences",
            "Apply",
            "Cancel",
        ] {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            let text = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Text(text)
                        if text.galley.job.text == label && rect.contains(text.pos) =>
                    {
                        Some(text)
                    }
                    _ => None,
                })
                .expect("control label is painted");
            assert_eq!(
                text.galley.job.sections[0].format.font_id.size, 16.0,
                "{label}"
            );
            if size.x > 1000.0 && matches!(label, "Appearance" | "Canvas" | "Text preferences") {
                assert!(h.output().shapes.iter().any(|shape|matches!(&shape.shape,Shape::Rect(image)
                    if image.brush.is_some() && rect.contains_rect(image.rect) && image.rect.size()==Vec2::splat(22.0)
                        && image.rect.right()<text.pos.x)),"{label} icon precedes its label");
                if let Some(x) = leading {
                    assert!((text.pos.x - x).abs() < 1.0, "sidebar labels align");
                }
                leading = Some(text.pos.x);
            }
        }
        h.get_by_label("Close settings").hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, "Close settings")
                .count(),
            1
        );
        h.get_by_label(if appearance == Appearance::Light {
            "Dark"
        } else {
            "Light"
        })
        .scroll_to_me();
        h.run();
        h.get_by_label(if appearance == Appearance::Light {
            "Dark"
        } else {
            "Light"
        })
        .click();
        h.run();
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        if ppp == 2.0 {
            h.get_by_label("Close settings").focus();
            h.run();
            h.key_press(Key::Space);
        } else {
            h.get_by_label("Close settings").click();
        }
        h.run();
        assert!(h.query_by_label("Apply").is_none());
        assert_eq!(
            h.ctx.theme(),
            if appearance == Appearance::Light {
                eframe::egui::Theme::Light
            } else {
                eframe::egui::Theme::Dark
            }
        );
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        assert!(h.state().editor().document().same_semantics(&document));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic Settings surface review"]
fn export_classic_settings_surfaces() {
    let output = std::env::temp_dir().join("paintcraft-settings-surfaces");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, layout) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let dir = scratch("settings-surfaces-review");
            Settings {
                appearance,
                classic_variant: Some(ClassicVariant::V11),
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let saved = std::fs::read(dir.join("settings.json")).unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(size);
            h.run();
            let document = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            h.get_by_label("Settings…").click();
            h.run();
            for (label, page) in [
                ("Appearance", "appearance"),
                ("Canvas", "canvas"),
                ("Text preferences", "text"),
            ] {
                h.get_by_role_and_label(Role::Button, label).click();
                h.run();
                h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
                h.run();
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{layout}-{page}.png")))
                    .unwrap();
            }
            h.get_by_role_and_label(Role::Button, "Close settings")
                .hover();
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{layout}-close-hover.png")))
                .unwrap();
            h.get_by_role_and_label(Role::Button, "Close settings")
                .click();
            h.run();
            assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
            assert!(h.state().editor().document().same_semantics(&document));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

use super::*;
use eframe::egui::{Color32, Rect, Role, Stroke, accesskit::Toggled, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn ratio(a: Color32, b: Color32) -> f32 {
    let luminance = |color: Color32| {
        let channel = |value: u8| {
            let v = f32::from(value) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn boundary(h: &Harness<App>, role: Role, label: &str) -> (Color32, Stroke) {
    fn flatten<'a>(clip: Rect, shape: &'a Shape, out: &mut Vec<(Rect, &'a Shape)>) {
        if let Shape::Vec(children) = shape {
            for child in children {
                flatten(clip, child, out);
            }
        } else {
            out.push((clip, shape));
        }
    }
    let mut shapes = Vec::new();
    for shape in &h.output().shapes {
        flatten(shape.clip_rect, &shape.shape, &mut shapes);
    }
    let row = h.get_by_role_and_label(role, label).rect();
    let (index, rect, fill, stroke) = shapes
        .iter()
        .enumerate()
        .find_map(|(index, (clip, shape))| {
            let (rect, fill, stroke) = match shape {
                Shape::Circle(circle) if role == Role::RadioButton && circle.radius >= 9.0 => (
                    Rect::from_center_size(circle.center, Vec2::splat(circle.radius * 2.0)),
                    circle.fill,
                    circle.stroke,
                ),
                Shape::Rect(rect)
                    if role == Role::CheckBox
                        && rect.brush.is_none()
                        && (20.0..=26.0).contains(&rect.rect.width())
                        && rect.rect.width() == rect.rect.height() =>
                {
                    (rect.rect, rect.fill, rect.stroke)
                }
                _ => return None,
            };
            (row.contains_rect(rect) && clip.contains_rect(rect))
                .then_some((index, rect, fill, stroke))
        })
        .expect("native control outline is painted within its row");
    let background = shapes[..index]
        .iter()
        .rev()
        .find_map(|(_, shape)| match shape {
            Shape::Rect(frame)
                if frame.brush.is_none()
                    && frame.fill.a() == 255
                    && frame.rect.contains_rect(rect) =>
            {
                Some(frame.fill)
            }
            _ => None,
        })
        .expect("control has an opaque surrounding surface");
    assert!(stroke.width >= 1.0, "{label} needs an outline");
    assert_eq!(stroke.color.a(), 255);
    assert!(
        ratio(stroke.color, fill) >= 3.0,
        "{label} border/fill contrast: {:.2}",
        ratio(stroke.color, fill)
    );
    assert!(
        ratio(stroke.color, background) >= 3.0,
        "{label} border/card contrast: {:.2}, border={:?}, background={:?}, fill={:?}",
        ratio(stroke.color, background),
        stroke.color,
        background,
        fill
    );
    (fill, stroke)
}

fn states(h: &mut Harness<App>, role: Role, label: &str) {
    h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
    h.run();
    let idle = boundary(h, role, label);
    h.get_by_role_and_label(role, label).hover();
    h.run();
    let hover = boundary(h, role, label);
    assert_ne!(idle, hover, "hover feedback");
    let point = h.get_by_role_and_label(role, label).rect().center();
    h.input_mut()
        .events
        .extend([Event::PointerMoved(point), press(point)]);
    h.step();
    let pressed = boundary(h, role, label);
    assert_ne!(hover, pressed, "pressed feedback");
    h.event(release(point));
    h.run();
    h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
    h.run();
    h.get_by_role_and_label(role, label).focus();
    h.run();
    boundary(h, role, label);
}

#[test]
fn classic_form_boundaries_are_visible_and_preserve_native_input_and_draft_ownership() {
    for (appearance, ppp) in [(Appearance::Light, 1.0), (Appearance::Dark, 2.0)] {
        let dir = scratch("classic-form-boundaries");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let global = h
            .ctx
            .style_of(h.ctx.theme())
            .visuals
            .widgets
            .inactive
            .bg_stroke;
        h.get_by_label("Settings…").click();
        h.run();
        boundary(&h, Role::RadioButton, "Use system setting");
        states(
            &mut h,
            Role::RadioButton,
            if appearance == Appearance::Light {
                "Light"
            } else {
                "Dark"
            },
        );
        h.get_by_role_and_label(Role::Button, "Text preferences")
            .click();
        h.run();
        let editable = "Keep text editable until save (prototype)";
        boundary(&h, Role::CheckBox, editable);
        states(&mut h, Role::CheckBox, editable);
        assert_eq!(
            h.get_by_label(editable).accesskit_node().toggled(),
            Some(Toggled::True)
        );
        h.key_press(Key::Space);
        h.run();
        assert_eq!(
            h.get_by_label(editable).accesskit_node().toggled(),
            Some(Toggled::False)
        );
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        h.get_by_role_and_label(Role::Button, "View").click();
        h.run();
        h.get_by_label("Default canvas size…").scroll_to_me();
        h.run();
        h.get_by_label("Default canvas size…").click();
        h.run();
        boundary(&h, Role::RadioButton, "Use configured default size");
        states(&mut h, Role::RadioButton, "Remember last canvas size");
        h.get_by_label("Cancel").click();
        h.run();
        h.get_by_label("Text").click();
        h.run();
        h.get_by_label("Background fill").scroll_to_me();
        h.run();
        states(&mut h, Role::CheckBox, "Background fill");
        assert!(Settings::load(&dir).0.text.background_fill);
        h.key_press(Key::Space);
        h.run();
        assert!(!Settings::load(&dir).0.text.background_fill);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        assert!(h.state().editor().document().same_semantics(&document));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(
            h.ctx
                .style_of(h.ctx.theme())
                .visuals
                .widgets
                .inactive
                .bg_stroke,
            global
        );
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic form boundary review"]
fn export_classic_form_boundaries() {
    let output = std::env::temp_dir().join("paintcraft-form-boundaries");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, layout) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let dir = scratch("form-boundaries-review");
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
            let saved = std::fs::read(dir.join("settings.json")).unwrap();
            let document = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            let capture = |h: &mut Harness<App>, surface: &str| {
                h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
                h.run();
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{layout}-{surface}.png")))
                    .unwrap();
            };
            h.get_by_label("Settings…").click();
            h.run();
            h.get_by_label("Use system setting").scroll_to_me();
            h.run();
            boundary(&h, Role::RadioButton, "Use system setting");
            capture(&mut h, "appearance");
            h.get_by_role_and_label(Role::Button, "Text preferences")
                .click();
            h.run();
            let editable = "Keep text editable until save (prototype)";
            h.get_by_label(editable).scroll_to_me();
            h.run();
            h.get_by_label(editable).click();
            h.run();
            boundary(&h, Role::CheckBox, editable);
            capture(&mut h, "settings-checked");
            h.get_by_label("Cancel").click();
            h.run();
            h.get_by_role_and_label(Role::Button, "View").click();
            h.run();
            h.get_by_label("Default canvas size…").scroll_to_me();
            h.run();
            h.get_by_label("Default canvas size…").click();
            h.run();
            h.get_by_label("Use configured default size").scroll_to_me();
            h.run();
            boundary(&h, Role::RadioButton, "Use configured default size");
            capture(&mut h, "startup-policy");
            h.get_by_label("Cancel").click();
            h.run();
            h.get_by_label("Text").click();
            h.run();
            h.get_by_label("Background fill").scroll_to_me();
            h.run();
            h.get_by_label("Background fill").click();
            h.run();
            boundary(&h, Role::CheckBox, "Background fill");
            capture(&mut h, "text-checked");
            h.get_by_label("Background fill").click();
            h.run();
            assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
            assert!(h.state().editor().document().same_semantics(&document));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

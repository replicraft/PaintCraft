use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn open(h: &mut Harness<App>, settings: bool, defaults: bool) {
    if settings {
        h.get_by_label("Settings…").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "Canvas").click();
        h.run();
    } else {
        h.get_by_role_and_label(Role::Button, "View").click();
        h.run();
        let label = if defaults {
            "Default canvas size…"
        } else {
            "Canvas size…"
        };
        h.get_by_label(label).scroll_to_me();
        h.run();
        h.get_by_label(label).click();
        h.run();
    }
}

fn leading(h: &Harness<App>, label: &str, value: u32) {
    let rect = h.get_by_role_and_label(Role::SpinButton, label).rect();
    assert_eq!(rect.height(), 40.0);
    let text = h
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            Shape::Text(text)
                if text.galley.job.text.parse::<u32>().ok() == Some(value)
                    && rect.contains(text.pos) =>
            {
                Some(text)
            }
            _ => None,
        })
        .expect("numeric value is painted in its field");
    let inset = text.pos.x - rect.left();
    assert!(
        (13.0..=16.0).contains(&inset),
        "{label} leading inset {inset} in {rect:?}"
    );
    assert_eq!(text.galley.job.sections[0].format.font_id.size, 16.0);
    assert_eq!(
        h.get_by_role_and_label(Role::SpinButton, label)
            .accesskit_node()
            .numeric_value(),
        Some(f64::from(value))
    );
}

#[test]
fn classic_dimension_fields_align_labels_and_values_without_adopting_cancelled_edits() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("dimension-field-alignment");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(size);
        h.run();
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        for (settings, defaults) in [(true, false), (false, false), (false, true)] {
            open(&mut h, settings, defaults);
            let labels = if settings {
                ["Default width", "Default height"]
            } else {
                ["Width", "Height"]
            };
            h.get_by_label(labels[0]).scroll_to_me();
            h.run();
            for (label, value) in [(labels[0], 777.0), (labels[1], 512.0)] {
                h.get_by_role_and_label(Role::SpinButton, label)
                    .set_value_accesskit(value);
                h.run();
            }
            let width = h.get_by_role_and_label(Role::SpinButton, labels[0]).rect();
            let height = h.get_by_role_and_label(Role::SpinButton, labels[1]).rect();
            assert_eq!(
                width.left(),
                height.left(),
                "dimension fields share a leading column"
            );
            assert_eq!(width.right(), height.right());
            for (label, value) in [(labels[0], 777), (labels[1], 512)] {
                leading(&h, label, value);
                h.get_by_role_and_label(Role::SpinButton, label).focus();
                h.run();
                leading(&h, label, value);
                h.key_press(Key::Enter);
                h.run();
            }
            let mut x = None;
            for label in labels {
                let caption = h
                    .output()
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        Shape::Text(text) if text.galley.job.text == label => Some(text),
                        _ => None,
                    })
                    .expect("dimension label is painted");
                if let Some(x) = x {
                    assert_eq!(caption.pos.x, x);
                }
                x = Some(caption.pos.x);
            }
            h.get_by_role_and_label(Role::SpinButton, labels[0]).focus();
            h.run();
            h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
            h.run();
            h.get_by_role_and_label(Role::SpinButton, labels[0])
                .type_text("888");
            h.run();
            h.key_press(Key::Enter);
            h.run();
            leading(&h, labels[0], 888);
            h.get_by_label("Cancel").click();
            h.run();
            assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
            assert!(h.state().editor().document().same_semantics(&document));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic dimension field alignment review"]
fn export_classic_dimension_fields() {
    let output = std::env::temp_dir().join("paintcraft-dimension-fields");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, layout) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let dir = scratch("dimension-fields-review");
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
            for (settings, defaults, surface) in [
                (true, false, "settings"),
                (false, false, "canvas"),
                (false, true, "defaults"),
            ] {
                open(&mut h, settings, defaults);
                let labels = if settings {
                    ["Default width", "Default height"]
                } else {
                    ["Width", "Height"]
                };
                h.get_by_label(labels[0]).scroll_to_me();
                h.run();
                h.get_by_label(labels[0]).set_value_accesskit(777.0);
                h.run();
                h.get_by_label(labels[1]).set_value_accesskit(512.0);
                h.run();
                h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
                h.run();
                leading(&h, labels[0], 777);
                leading(&h, labels[1], 512);
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{layout}-{surface}-idle.png")))
                    .unwrap();
                h.get_by_label(labels[0]).focus();
                h.run();
                leading(&h, labels[0], 777);
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{layout}-{surface}-focused.png")))
                    .unwrap();
                h.get_by_label("Cancel").click();
                h.run();
                assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
                assert!(h.state().editor().document().same_semantics(&document));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
            }
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

use super::*;
use paintcraft::settings::{Appearance, ClassicVariant, Experience, Settings};

fn configured(
    name: &str,
    experience: Experience,
    appearance: Appearance,
) -> (Harness<'static, App>, PathBuf) {
    let dir = scratch(&format!("appearance-{name}-{}", uuid::Uuid::new_v4()));
    Settings {
        experience,
        appearance,
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    (
        harness_with(App::new(None).with_settings(dir.clone()), 1.0, true),
        dir,
    )
}

#[test]
fn theme_staging_cancel_and_apply_preserve_document_and_survive_restart() {
    for experience in [Experience::Classic, Experience::Studio] {
        let (mut h, dir) = configured("preview", experience, Appearance::Light);
        let original = h.state().editor().document_snapshot();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        h.get_by_label("Settings…").click();
        h.run();
        h.get_by_label("Dark").click();
        h.run();
        assert_eq!(h.ctx.theme(), eframe::egui::Theme::Light);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(h.ctx.theme(), eframe::egui::Theme::Light);
        assert!(h.state().editor().document().same_semantics(&original));
        h.get_by_label("Settings…").click();
        h.run();
        h.get_by_label("Dark").click();
        h.run();
        h.get_by_label("Apply").click();
        h.run();
        assert_eq!(Settings::load(&dir).0.appearance, Appearance::Dark);
        assert!(!h.state().editor().is_dirty());
        let restarted = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        assert_eq!(restarted.ctx.theme(), eframe::egui::Theme::Dark);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn text_toolbar_and_deferred_text_prototype_preferences_are_explicit_and_persisted() {
    let (mut h, dir) = configured("text", Experience::Classic, Appearance::Dark);
    h.get_by_label("Text").click();
    h.run();
    for label in [
        "Font family",
        "Font size",
        "Bold",
        "Italic",
        "Underline",
        "Strikeout",
        "Align left",
        "Align center",
        "Align right",
        "Background fill",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    h.get_by_label("Bold").click();
    h.run();
    assert!(
        Settings::load(&dir).0.text.bold,
        "a repeated layout pass must not toggle twice"
    );
    h.get_by_label("Settings…").click();
    h.run();
    h.get_by_label("Text preferences").click();
    h.run();
    h.get_by_label("Keep text editable until save (prototype)")
        .click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    assert!(Settings::load(&dir).0.text.editable_until_save);
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires GPU; exports UI prototypes for visual review"]
fn export_theme_previews() {
    let output = std::env::temp_dir().join("paintcraft-theme-previews");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, name) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let (_, dir) = configured(name, Experience::Classic, appearance);
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{name}.png")))
            .unwrap();
        h.get_by_label("Text").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{name}-text.png")))
            .unwrap();
        h.get_by_label("Settings…").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{name}-settings.png")))
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
        let (_, dir) = configured(name, Experience::Studio, appearance);
        let mut studio = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        studio
            .render()
            .unwrap()
            .save(output.join(format!("studio-{name}.png")))
            .unwrap();
        studio.get_by_label("Settings…").click();
        studio.run();
        studio
            .render()
            .unwrap()
            .save(output.join(format!("studio-{name}-settings.png")))
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn interface_switch_requires_saving_and_restores_completely_separate_documents() {
    let (mut h, dir) = configured("workspaces", Experience::Studio, Appearance::Light);
    with_layer(&mut h);
    let source = h.state().editor().document_snapshot();
    let path = dir.join("studio.pcnd");
    h.state_mut().set_path_text(path.to_str().unwrap());
    switch_interface(&mut h, "Classic interface");
    assert_eq!(h.state().experience(), Experience::Studio);
    assert!(h.query_by_label("Save and switch").is_some());
    assert!(h.query_by_label("Discard and switch").is_some());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().editor().is_dirty());
    assert!(!path.exists());
    switch_interface(&mut h, "Classic interface");
    h.get_by_label("Save and switch").click();
    h.run();
    assert_eq!(h.state().experience(), Experience::Classic);
    assert_ne!(
        h.state().editor().document().document_id,
        source.document_id
    );
    assert_eq!(h.state().editor().document().canvas, (1000, 486));
    assert!(
        paintcraft::controller::Editor::open(&path)
            .unwrap()
            .document()
            .same_semantics(&source)
    );
    switch_interface(&mut h, "Studio interface");
    assert!(h.state().editor().document().same_semantics(&source));
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn settings_pages_fit_small_windows_and_cancel_keeps_drafts_out_of_preferences_and_history() {
    use eframe::egui::Role;
    for (size, ppp) in [
        (Vec2::new(1360.0, 900.0), 1.0),
        (Vec2::new(700.0, 500.0), 1.0),
        (Vec2::new(480.0, 500.0), 2.0),
        (Vec2::new(480.0, 360.0), 1.0),
    ] {
        let dir = scratch("responsive-settings");
        Settings {
            appearance: Appearance::Light,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, app: &mut App| {
                    app.ui(ui);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx().request_discard("test: repeated settings layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_label("Settings…").click();
        h.run();
        let footer = h.get_by_label("Apply").rect();
        h.run_steps(8);
        assert_eq!(
            h.get_by_label("Apply").rect(),
            footer,
            "settings must settle: {size:?}"
        );
        for page in ["Canvas", "Text preferences", "Appearance"] {
            h.get_by_role_and_label(Role::Button, page).click();
            h.run();
            for label in [
                "Appearance",
                "Canvas",
                "Text preferences",
                "Apply",
                "Cancel",
            ] {
                let rect = h.get_by_role_and_label(Role::Button, label).rect();
                assert!(
                    rect.min.x >= 0.0
                        && rect.min.y >= 0.0
                        && rect.max.x <= size.x
                        && rect.max.y <= size.y,
                    "{label}: {rect:?} outside {size:?}"
                );
            }
            assert_eq!(
                h.get_by_label("Apply").rect(),
                footer,
                "footer stays fixed across pages"
            );
        }
        h.get_by_label("Dark").scroll_to_me();
        h.run();
        h.get_by_label("Dark").click();
        h.run();
        assert_eq!(h.ctx.theme(), eframe::egui::Theme::Light);
        h.get_by_role_and_label(Role::Button, "Canvas").click();
        h.run();
        h.get_by_role_and_label(Role::SpinButton, "Default width")
            .set_value_accesskit(777.0);
        h.run();
        h.get_by_label("Use configured default size").scroll_to_me();
        h.run();
        h.get_by_label("Use configured default size").click();
        h.run();
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(h.ctx.theme(), eframe::egui::Theme::Light);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        assert!(h.state().editor().document().same_semantics(&document));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn narrow_settings_apply_persists_theme_and_canvas_defaults_without_resizing_the_open_image() {
    use eframe::egui::Role;
    let dir = scratch("narrow-settings-apply");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = Harness::builder()
        .with_size(Vec2::new(480.0, 500.0))
        .build_ui_state(
            |ui, app: &mut App| app.ui(ui),
            App::new(None).with_settings(dir.clone()),
        );
    h.run();
    let document = h.state().editor().document_snapshot();
    h.get_by_label("Settings…").click();
    h.run();
    h.get_by_label("Dark").scroll_to_me();
    h.run();
    let dark = h.get_by_label("Dark").rect();
    assert!(h.ctx.content_rect().contains_rect(dark), "dark={dark:?}");
    h.get_by_label("Dark").click();
    h.run();
    assert_eq!(
        h.ctx.theme(),
        eframe::egui::Theme::Light,
        "theme must stay unchanged before Apply"
    );
    h.get_by_role_and_label(Role::Button, "Canvas").click();
    h.run();
    h.get_by_role_and_label(Role::SpinButton, "Default width")
        .set_value_accesskit(777.0);
    h.run();
    h.get_by_label("Use configured default size").scroll_to_me();
    h.run();
    h.get_by_label("Use configured default size").click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    let saved = Settings::load(&dir).0;
    assert_eq!(saved.appearance, Appearance::Dark);
    assert_eq!(saved.canvas.new_size(), (777, 486));
    assert!(h.state().editor().document().same_semantics(&document));
    assert_eq!(
        App::new(None)
            .with_settings(dir.clone())
            .editor()
            .document()
            .canvas,
        (777, 486)
    );
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn classic_settings_descriptions_remain_readable_against_their_actual_card_surfaces() {
    use eframe::egui::epaint::Shape;
    for appearance in [Appearance::Light, Appearance::Dark] {
        let (mut h, dir) = configured("settings-text-contrast", Experience::Classic, appearance);
        h.get_by_label("Settings…").click();
        h.run();
        let shapes = &h.output().shapes;
        let mut checked = 0;
        for (index, shape) in shapes.iter().enumerate() {
            let Shape::Text(text) = &shape.shape else {
                continue;
            };
            if !text.galley.job.text.starts_with("Changes take effect when")
                && !text
                    .galley
                    .job
                    .text
                    .starts_with("Each interface has a separate workspace.")
            {
                continue;
            }
            let rect = text.visual_bounding_rect();
            let background = shapes[..index]
                .iter()
                .rev()
                .find_map(|shape| match &shape.shape {
                    Shape::Rect(frame)
                        if frame.brush.is_none()
                            && frame.fill.a() == 255
                            && frame.rect.contains_rect(rect) =>
                    {
                        Some(frame.fill)
                    }
                    _ => None,
                })
                .expect("description has an opaque card surface");
            let ink = text.galley.job.sections[0].format.color;
            let (a, b) = (luminance(ink), luminance(background));
            let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
            assert!(
                contrast >= 4.5,
                "description contrast must stay readable: {contrast:.2}"
            );
            checked += 1;
        }
        assert_eq!(checked, 2);
        h.get_by_label("Cancel").click();
        h.run();
        assert!(!h.state().editor().is_dirty());
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn luminance(color: eframe::egui::Color32) -> f32 {
        let channel = |value: u8| {
            let v = f32::from(value) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b, _] = color.to_array();
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }
}


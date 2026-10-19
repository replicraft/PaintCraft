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
fn theme_preview_cancel_and_apply_preserve_document_and_survive_restart() {
    for experience in [Experience::Classic, Experience::Studio] {
        let (mut h, dir) = configured("preview", experience, Appearance::Light);
        let original = h.state().editor().document_snapshot();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        h.get_by_label("Settings…").click();
        h.run();
        h.get_by_label("Dark").click();
        h.run();
        assert_eq!(h.ctx.theme(), eframe::egui::Theme::Dark);
        let visuals = &h.ctx.style_of(eframe::egui::Theme::Dark).visuals;
        assert!(
            visuals.panel_fill.r() < 80 && visuals.window_fill.r() < 80,
            "dark surfaces must follow the theme in both frontends"
        );
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
    assert!(h.query_by_label("Don't save").is_none());
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

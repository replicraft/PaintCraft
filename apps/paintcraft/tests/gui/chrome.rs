use super::*;

#[test]
fn first_visit_shows_the_picker_with_v11_selected_and_remembers_the_choice() {
    use paintcraft::settings::{ClassicVariant, Loaded, Settings};
    let dir = scratch("settings");
    {
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        assert!(h.state().picker_open());
        assert_eq!(
            h.state().variant(),
            ClassicVariant::V11,
            "the default before any choice"
        );
        h.get_by_label("Continue").click();
        h.run();
        assert!(!h.state().picker_open());
        assert_eq!(
            Settings::load(&dir).0.classic_variant,
            Some(ClassicVariant::V11),
            "accepting the default saves it"
        );
    }
    // A later start opens straight in the saved variant without asking again.
    let h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert!(!h.state().picker_open());
    drop(h);

    // Changing it later goes through the Interface button.
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    h.get_by_label("Interface").click();
    h.run();
    assert!(h.state().picker_open());
    h.get_by_role_and_label(eframe::egui::Role::RadioButton, "Paint v7")
        .click();
    h.run();
    h.get_by_label("Continue").click();
    h.run();
    assert_eq!(h.state().variant(), ClassicVariant::V7);
    assert_eq!(
        Settings::load(&dir),
        (
            Settings {
                classic_variant: Some(ClassicVariant::V7)
            },
            Loaded::Ok
        )
    );
    let h2 = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert_eq!(h2.state().variant(), ClassicVariant::V7);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_damaged_settings_file_is_kept_and_explained_and_the_picker_returns() {
    let dir = scratch("badsettings");
    std::fs::write(dir.join("settings.json"), b"{ not json").unwrap();
    let h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert!(h.state().picker_open());
    assert!(
        h.state()
            .editor()
            .status()
            .contains("Settings were unusable"),
        "{}",
        h.state().editor().status()
    );
    let kept: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        kept.iter().any(|n| n.starts_with("settings.json.unusable")),
        "{kept:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_variant_renders_a_usable_window_with_the_same_controls() {
    use paintcraft::settings::{ClassicVariant, Settings};
    for v in ClassicVariant::ALL {
        let dir = scratch("variant");
        Settings {
            classic_variant: Some(v),
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        with_layer(&mut h);
        h.get_by_label("Pencil").click();
        h.run();
        drag(&mut h, (5.0, 5.0), (30.0, 5.0));
        assert_eq!(top_pixel(&h, 20, 5).a(), 255, "{v:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn layers_can_be_moved_deleted_and_restored_from_the_panel() {
    let mut h = harness();
    with_layer(&mut h);
    let ids = || h.state().editor().document().roots.clone();
    let [bottom, top] = <[uuid::Uuid; 2]>::try_from(ids()).unwrap();
    let before = publications(&h);
    h.get_by_label("Move down").click();
    h.run();
    assert_eq!(h.state().editor().document().roots, [top, bottom]);
    assert_eq!(publications(&h), before + 1);
    assert!(
        h.get_by_label("Move down").accesskit_node().is_disabled(),
        "already at the bottom"
    );
    h.get_by_label("Delete layer").click();
    h.run();
    assert_eq!(
        h.state().editor().document().roots,
        [bottom],
        "the target (top) layer was deleted"
    );
    assert_eq!(
        h.state().editor().target(),
        Some(bottom),
        "the target moved to a layer that exists"
    );
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(h.state().editor().document().roots, [top, bottom]);
}

#[test]
fn renaming_and_opacity_apply_once_on_commit_not_per_keystroke() {
    let mut h = harness();
    with_layer(&mut h);
    let layer = h.state().editor().target().unwrap();
    let before = publications(&h);
    let field = h.get_by_label("Layer name");
    field.focus();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    h.run();
    h.get_by_label("Layer name").type_text("Sky");
    h.run();
    assert_eq!(publications(&h), before, "typing alone publishes nothing");
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state().editor().layer_properties(layer).map(|p| p.0),
        Some("Sky".to_owned())
    );
    assert_eq!(publications(&h), before + 1);
}

#[test]
fn classic_ribbon_tools_are_visible_and_activate_at_the_default_window_size() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(960.0, 720.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), App::new(None));
    h.run();
    let canvas = h.state().canvas_rect().unwrap();
    for label in [
        "Pencil",
        "Fill",
        "Text",
        "Rectangle",
        "Select",
        "Color #E0E0FF",
    ] {
        let rect = h.get_by_label(label).rect();
        assert!(
            rect.min.x >= 0.0 && rect.max.x <= 960.0,
            "{label} is outside the window: {rect:?}"
        );
        assert!(
            rect.max.y < canvas.min.y,
            "{label} must be in the ribbon above the canvas"
        );
    }
    h.get_by_label("Fill").click();
    h.run();
    assert_eq!(h.state().editor().tool, paintcraft::controller::Tool::Fill);
}

#[test]
#[ignore = "requires a real GPU; exports current application previews, not reference captures"]
fn export_classic_ui_previews() {
    use paintcraft::settings::{ClassicVariant, Settings};
    let output = std::env::var_os("PAINTCRAFT_PREVIEW_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-ui-previews"));
    std::fs::create_dir_all(&output).unwrap();
    for variant in ClassicVariant::ALL {
        let settings = scratch("preview-settings");
        Settings {
            classic_variant: Some(variant),
        }
        .save(&settings)
        .unwrap();
        let mut h = Harness::builder()
            .with_size(Vec2::new(960.0, 720.0))
            .build_ui_state(
                |ui, app: &mut App| app.ui(ui),
                App::new(None).with_settings(settings.clone()),
            );
        h.run();
        with_layer(&mut h);
        h.get_by_label("Color #00A2E8").click();
        h.run();
        h.get_by_label("Rectangle").click();
        h.run();
        h.get_by_label("Filled").click();
        h.run();
        drag(&mut h, (45.0, 70.0), (240.0, 195.0));
        h.get_by_label("Color #FFC90E").click();
        h.run();
        h.get_by_label("Ellipse").click();
        h.run();
        drag(&mut h, (100.0, 105.0), (285.0, 245.0));
        h.get_by_label("Pencil").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{}.png", variant.id())))
            .unwrap();
        std::fs::remove_dir_all(settings).unwrap();
    }
}

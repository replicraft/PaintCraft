use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{self, Language, translate};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/ase-fixtures/mixed.ase"
);
fn app(path: &Path) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.begin_palettes();
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let root =
            std::env::temp_dir().join(format!("paintcraft-palette-fixture-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join(path.file_name().unwrap());
        std::fs::write(&source, SOURCE).unwrap();
        source
    };
    app.install_palette(path).unwrap();
    app
}
#[test]
fn translated_native_palette_controls_pick_colors_without_editing_or_narrowing_the_resource() {
    for language in [Language::English, Language::Japanese] {
        let mut app = app(Path::new("palette.ase"));
        app.studio.edit_secondary = true;
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        let primary = app.editor.color;
        let mut h = Harness::builder()
            .with_max_steps(20)
            .with_size(Vec2::new(1360.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut App| {
                    if ui.ctx().missing_glyph_policy()
                        != egui::epaint::text::MissingGlyphPolicy::Tofu
                    {
                        paintcraft_ui::fonts::install(ui.ctx());
                        ui.ctx()
                            .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                    }
                    i18n::set_language(ui.ctx(), language);
                    app.confirm_dialog(ui);
                },
                app,
            );
        h.run();
        assert!(
            h.get_by_role_and_label(egui::Role::Button, "Cyan")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_role_and_label(egui::Role::Button, "赤 🚀").click();
        h.run();
        assert_eq!(h.state().editor.secondary, Rgba([64, 128, 255, 255]));
        assert_eq!(h.state().editor.color, primary);
        assert!(h.state().confirm.is_none());
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view().ticket, ticket);
        assert_eq!(h.state().editor.is_dirty(), dirty);
        assert_eq!(
            ase::encode(
                &h.state().palettes.loaded.as_ref().unwrap().palette,
                &|| Ok(())
            )
            .unwrap(),
            SOURCE
        );
        h.state_mut().begin_palettes();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Close"))
            .click();
        h.run();
        assert!(h.state().confirm.is_none());
    }
}
#[test]
fn native_palette_picker_routes_preserve_sources_and_refuse_stale_results() {
    let root = std::env::temp_dir().join(format!("paintcraft-palette-ui-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("source.ase");
    let output = root.join("copy.ase");
    std::fs::write(&source, SOURCE).unwrap();
    let ctx = egui::Context::default();
    let mut app = app(&source);
    let before = app.editor.document_snapshot();
    app.palettes.picker = Some(app.capture_palette_picker(Kind::ExportPalette).unwrap());
    app.complete_dialog(
        &ctx,
        paintcraft_platform::dialogs::Completion {
            kind: Kind::ExportPalette,
            path: Some(output.clone()),
        },
    );
    assert_eq!(std::fs::read(&output).unwrap(), SOURCE);
    assert_eq!(std::fs::read(&source).unwrap(), SOURCE);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.palettes.picker = Some(app.capture_palette_picker(Kind::ExportPalette).unwrap());
    app.complete_palette_picker(&ctx, Kind::ExportPalette, Some(source.clone()));
    assert!(app.palettes.error.is_some());
    assert_eq!(std::fs::read(&source).unwrap(), SOURCE);
    let loaded = app.palettes.loaded.clone().unwrap();
    app.palettes.picker = Some(app.capture_palette_picker(Kind::ImportPalette).unwrap());
    app.complete_palette_picker(&ctx, Kind::ImportPalette, None);
    assert!(Arc::ptr_eq(&loaded, app.palettes.loaded.as_ref().unwrap()));
    drop(loaded);
    for stale in 0..3 {
        std::fs::write(&output, b"existing file").unwrap();
        app.settings.experience = Experience::Studio;
        app.begin_palettes();
        app.palettes.picker = Some(app.capture_palette_picker(Kind::ExportPalette).unwrap());
        match stale {
            0 => app.settings.experience = Experience::Classic,
            1 => app.palettes.epoch = Uuid::new_v4(),
            _ => app.install_palette(source.clone()).unwrap(),
        }
        app.complete_palette_picker(&ctx, Kind::ExportPalette, Some(output.clone()));
        assert_eq!(std::fs::read(&output).unwrap(), b"existing file");
    }
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn palette_failures_do_not_replace_the_previous_resource_or_create_a_document() {
    let mut app = app(Path::new("source.ase"));
    let original = app.palettes.loaded.clone().unwrap();
    let broken = app
        .palettes
        .source_path()
        .unwrap()
        .with_file_name("broken.ase");
    for bytes in [b"ASEF".as_slice(), &SOURCE[..SOURCE.len() - 1]] {
        std::fs::write(&broken, bytes).unwrap();
        assert!(app.install_palette(broken.clone()).is_err());
        assert!(Arc::ptr_eq(
            &original,
            app.palettes.loaded.as_ref().unwrap()
        ));
    }
    app.settings.experience = Experience::Classic;
    assert!(app.install_palette(PathBuf::from("source.ase")).is_err());
}
#[cfg(unix)]
#[test]
fn palette_export_protects_sources_in_current_other_tab_and_parked_workspaces() {
    let root = std::env::temp_dir().join(format!("paintcraft-palette-source-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("palette.ase");
    std::fs::write(&source, SOURCE).unwrap();
    let ctx = egui::Context::default();
    for case in 0..3 {
        let mut app = app(&source);
        let project = root.join(format!("document-{case}.pcnd"));
        let alias = project.with_extension("ase");
        std::fs::write(&project, b"protected project bytes").unwrap();
        std::fs::hard_link(&project, &alias).unwrap();
        app.bound = Some(project.clone());
        match case {
            1 => app
                .add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                .unwrap(),
            2 => {
                let mut parked = tabs::WorkspaceDocuments::new((2, 2), false);
                parked.swap(&mut app);
                app.other_workspace = Some(Box::new(parked));
            }
            _ => {}
        }
        app.begin_palettes();
        app.palettes.picker = Some(app.capture_palette_picker(Kind::ExportPalette).unwrap());
        app.complete_palette_picker(&ctx, Kind::ExportPalette, Some(alias));
        assert!(app.palettes.error.is_some());
        assert_eq!(std::fs::read(&project).unwrap(), b"protected project bytes");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn palette_export_cannot_replace_a_loaded_shape_resource_alias() {
    let root = std::env::temp_dir().join(format!("paintcraft-cross-resource-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let palette = root.join("palette.ase");
    let shape = root.join("shape.csh");
    let alias = root.join("shape-alias.ase");
    let shapes = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh"
    );
    std::fs::write(&palette, SOURCE).unwrap();
    std::fs::write(&shape, shapes).unwrap();
    std::fs::hard_link(&shape, &alias).unwrap();
    let mut app = app(&palette);
    app.install_shape(shape.clone()).unwrap();
    let before = app.editor.document_snapshot();
    app.palettes.picker = Some(app.capture_palette_picker(Kind::ExportPalette).unwrap());
    app.complete_dialog(
        &egui::Context::default(),
        paintcraft_platform::dialogs::Completion {
            kind: Kind::ExportPalette,
            path: Some(alias.clone()),
        },
    );
    assert!(app.palettes.error.is_some());
    assert_eq!(std::fs::read(&alias).unwrap(), shapes);
    assert_eq!(std::fs::read(&shape).unwrap(), shapes);
    assert_eq!(std::fs::read(&palette).unwrap(), SOURCE);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.asset_source_paths().count(), 2);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn palette_bytes_and_frozen_identity_stay_from_one_acquisition_after_namespace_replacement() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-palette-acquisition-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let selected = root.join("selected.ase");
    let moved = root.join("moved.ase");
    let alias = root.join("picture.png");
    std::fs::write(&selected, SOURCE).unwrap();
    let acquired =
        crate::format_admission::acquire_resource_source(&selected, ase::BYTES_MAX as u64).unwrap();
    std::fs::rename(&selected, &moved).unwrap();
    let empty = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/ase-fixtures/empty.ase"
    );
    std::fs::write(&selected, empty).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.install_palette_source(acquired, selected.clone())
        .unwrap();
    assert_eq!(
        app.palettes.loaded.as_ref().unwrap().palette,
        ase::decode(SOURCE, &|| Ok(())).unwrap()
    );
    std::os::unix::fs::symlink(&moved, &alias).unwrap();
    app.do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
    assert_eq!(std::fs::read(&moved).unwrap(), SOURCE);
    assert_eq!(std::fs::read(&selected).unwrap(), empty);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

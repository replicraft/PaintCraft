use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{self, Language, translate};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh"
);
fn app(path: &Path) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.begin_shapes();
    app.install_shape(SOURCE, path.to_path_buf()).unwrap();
    app
}
#[test]
fn translated_native_shape_selection_previews_without_editing_the_document() {
    for language in [Language::English, Language::Japanese] {
        let app = app(Path::new("shapes.csh"));
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        assert_eq!(
            app.shapes.loaded.as_ref().unwrap().view.shapes[0].paths[0].knots[0],
            [1.0, -1.0 / 16_777_216.0, 1.0 / 16_777_216.0, 0.5, 0.0, -1.0]
        );
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
        h.get_by_role_and_label(egui::Role::Button, "Shape 2")
            .click();
        h.run();
        assert_eq!(h.state().shapes.selected, 2);
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view().ticket, ticket);
        assert_eq!(h.state().editor.is_dirty(), dirty);
        assert_eq!(
            h.state().shapes.loaded.as_ref().unwrap().library,
            csh::decode(SOURCE, &|| Ok(())).unwrap()
        );
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Close"))
            .click();
        h.run();
        assert!(h.state().confirm.is_none());
    }
}
#[test]
fn native_shape_picker_routes_preserve_sources_and_refuse_stale_results() {
    let root = std::env::temp_dir().join(format!("paintcraft-shape-ui-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("source.csh");
    let output = root.join("copy.csh");
    std::fs::write(&source, SOURCE).unwrap();
    let ctx = egui::Context::default();
    let mut app = app(&source);
    let before = app.editor.document_snapshot();
    app.shapes.picker = Some(app.capture_shape_picker(Kind::ExportShape).unwrap());
    app.complete_dialog(
        &ctx,
        paintcraft_platform::dialogs::Completion {
            kind: Kind::ExportShape,
            path: Some(output.clone()),
        },
    );
    assert_eq!(
        csh::decode(&std::fs::read(&output).unwrap(), &|| Ok(())).unwrap(),
        csh::decode(SOURCE, &|| Ok(())).unwrap()
    );
    assert_eq!(std::fs::read(&source).unwrap(), SOURCE);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.shapes.picker = Some(app.capture_shape_picker(Kind::ExportShape).unwrap());
    app.complete_shape_picker(&ctx, Kind::ExportShape, Some(source.clone()));
    assert!(app.shapes.error.is_some());
    assert_eq!(std::fs::read(&source).unwrap(), SOURCE);
    let loaded = app.shapes.loaded.clone().unwrap();
    app.shapes.picker = Some(app.capture_shape_picker(Kind::ImportShape).unwrap());
    app.complete_shape_picker(&ctx, Kind::ImportShape, None);
    assert!(Arc::ptr_eq(&loaded, app.shapes.loaded.as_ref().unwrap()));
    for stale in 0..3 {
        std::fs::write(&output, b"existing file").unwrap();
        app.settings.experience = Experience::Studio;
        app.begin_shapes();
        app.shapes.picker = Some(app.capture_shape_picker(Kind::ExportShape).unwrap());
        match stale {
            0 => app.settings.experience = Experience::Classic,
            1 => app.shapes.epoch = Uuid::new_v4(),
            _ => app.install_shape(SOURCE, source.clone()).unwrap(),
        }
        app.complete_shape_picker(&ctx, Kind::ExportShape, Some(output.clone()));
        assert_eq!(std::fs::read(&output).unwrap(), b"existing file");
    }
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn shape_failures_do_not_replace_the_previous_resource_or_create_a_document() {
    let mut app = app(Path::new("source.csh"));
    let original = app.shapes.loaded.clone().unwrap();
    for bytes in [b"cush".as_slice(), &SOURCE[..SOURCE.len() - 1]] {
        assert!(
            app.install_shape(bytes, PathBuf::from("broken.csh"))
                .is_err()
        );
        assert!(Arc::ptr_eq(&original, app.shapes.loaded.as_ref().unwrap()));
    }
    app.settings.experience = Experience::Classic;
    assert!(
        app.install_shape(SOURCE, PathBuf::from("source.csh"))
            .is_err()
    );
}
#[cfg(unix)]
#[test]
fn shape_export_protects_sources_in_current_other_tab_and_parked_workspaces() {
    let root = std::env::temp_dir().join(format!("paintcraft-shape-source-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("shape.csh");
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
        app.begin_shapes();
        app.shapes.picker = Some(app.capture_shape_picker(Kind::ExportShape).unwrap());
        app.complete_shape_picker(&ctx, Kind::ExportShape, Some(alias));
        assert!(app.shapes.error.is_some());
        assert_eq!(std::fs::read(&project).unwrap(), b"protected project bytes");
    }
    std::fs::remove_dir_all(root).unwrap();
}

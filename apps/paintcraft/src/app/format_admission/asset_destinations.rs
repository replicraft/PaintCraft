use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use std::time::Instant;

const ASE: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/ase-fixtures/mixed.ase"
);
const CSH: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh"
);

fn scratch() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-resource-destination-{}",
        Uuid::new_v4()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::canonicalize(root).unwrap()
}
fn loaded(root: &Path, shape: bool) -> (App, PathBuf, &'static [u8]) {
    let (name, bytes) = if shape {
        ("source.csh", CSH)
    } else {
        ("source.ase", ASE)
    };
    let source = root.join(name);
    std::fs::write(&source, bytes).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::new_document(2, 2).unwrap();
    if shape {
        app.begin_shapes();
        app.install_shape(bytes, source.clone()).unwrap();
    } else {
        app.begin_palettes();
        app.install_palette(bytes, source.clone()).unwrap();
    }
    app.confirm = None;
    (app, source, bytes)
}
fn confirm_save_if_offered(app: &mut App, ctx: &egui::Context) {
    // Follow the same confirmed replacement route used by the UI's Continue.
    if let Some(Confirm::ReplaceFile(path) | Confirm::ReplaceSharedFile(path)) = app.confirm.take()
    {
        app.do_save(ctx, path);
    }
    app.editor.run_pending();
    app.finish_save();
}
fn settle(app: &mut App, ctx: &egui::Context) {
    app.editor.run_pending();
    app.finish_save();
    let deadline = Instant::now() + Duration::from_secs(10);
    while app
        .saving
        .as_ref()
        .is_some_and(saving::NativeSaving::pending)
    {
        app.poll_native_saves(ctx);
        assert!(Instant::now() < deadline, "native save did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn choose(app: App, label: &str) -> App {
    let mut harness = Harness::builder().with_max_steps(20).build_ui_state(
        |ui, app: &mut App| {
            if ui.ctx().missing_glyph_policy() != egui::epaint::text::MissingGlyphPolicy::Tofu {
                paintcraft_ui::fonts::install(ui.ctx());
                ui.ctx()
                    .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
            }
            app.confirm_dialog(ui);
        },
        app,
    );
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, label)
        .click();
    harness.run();
    harness.into_state()
}

#[cfg(unix)]
#[test]
fn project_save_as_does_not_replace_a_loaded_resource_through_a_symlink() {
    for shape in [false, true] {
        let root = scratch();
        let (mut app, source, bytes) = loaded(&root, shape);
        let alias = root.join("project.pcnd");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        let ctx = egui::Context::default();
        let snapshot = app.editor.document_snapshot();
        app.save_selected(&ctx, alias.clone());
        confirm_save_if_offered(&mut app, &ctx);
        assert_eq!(
            std::fs::read(&source).unwrap(),
            bytes,
            "asset source changed"
        );
        assert_eq!(
            std::fs::read(&alias).unwrap(),
            bytes,
            "resource alias was replaced"
        );
        assert!(app.bound.is_none());
        assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn confirmed_image_export_does_not_replace_a_loaded_resource_through_a_symlink() {
    for shape in [false, true] {
        let root = scratch();
        let (mut app, source, bytes) = loaded(&root, shape);
        let alias = root.join("export.png");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        app.plan_export_settings(alias.clone(), export_options::Settings::default());
        if let Some(Confirm::Export {
            path,
            format,
            losses,
            jpeg_quality,
            pcx_encoding,
            ..
        }) = app.confirm.take()
        {
            app.do_export_settings(
                &path,
                format,
                &losses,
                export_options::Settings {
                    jpeg_quality,
                    pcx_encoding,
                },
            );
        }
        assert_eq!(
            std::fs::read(&source).unwrap(),
            bytes,
            "asset source changed"
        );
        assert_eq!(
            std::fs::read(&alias).unwrap(),
            bytes,
            "resource alias was replaced"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn project_save_and_flat_export_refuse_direct_resource_paths_and_hard_links() {
    for shape in [false, true] {
        for queued in [false, true] {
            let root = scratch();
            let (mut app, source, bytes) = loaded(&root, shape);
            let ctx = egui::Context::default();
            if queued {
                app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
            }
            let project_alias = root.join("alias.pcnd");
            let image_alias = root.join("alias.png");
            std::fs::hard_link(&source, &project_alias).unwrap();
            std::fs::hard_link(&source, &image_alias).unwrap();
            let snapshot = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            for path in [&source, &project_alias] {
                app.save_selected(&ctx, path.clone());
                confirm_save_if_offered(&mut app, &ctx);
                settle(&mut app, &ctx);
                assert_eq!(std::fs::read(path).unwrap(), bytes);
                assert!(app.bound.is_none());
            }
            app.do_export(
                &image_alias,
                ImageFormat::Png,
                &KNOWN_LOSSES.map(str::to_owned),
            );
            assert_eq!(std::fs::read(&image_alias).unwrap(), bytes);
            assert_eq!(std::fs::read(&source).unwrap(), bytes);
            assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
            assert_eq!(app.editor.history_view().ticket, ticket);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn unrelated_loaded_resources_do_not_deny_normal_bound_project_saves() {
    for queued in [false, true] {
        let root = scratch();
        let (mut app, source, bytes) = loaded(&root, true);
        let ctx = egui::Context::default();
        if queued {
            app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
        }
        let project = root.join("painting.pcnd");
        app.save_selected(&ctx, project.clone());
        settle(&mut app, &ctx);
        assert_eq!(app.bound.as_deref(), Some(project.as_path()));
        app.editor.request(91, Action::AddLayer);
        app.editor.run_pending();
        assert!(app.editor.is_dirty());
        // Saving again is a new UI activation, not a second layout pass of the
        // first Save As frame (which the controller correctly deduplicates).
        let mut frame = ctx.run_ui(Default::default(), |_| {});
        frame.textures_delta.clear();
        app.save_requested(&ctx);
        settle(&mut app, &ctx);
        assert!(!app.editor.is_dirty(), "{}", app.editor.status());
        assert!(
            Editor::open(&project)
                .unwrap()
                .document()
                .same_semantics(app.editor.document())
        );
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn confirmation_rechecks_a_destination_retargeted_to_a_loaded_resource() {
    for shape in [false, true] {
        for export in [false, true] {
            let root = scratch();
            let (mut app, source, bytes) = loaded(&root, shape);
            let ctx = egui::Context::default();
            let path = root.join(if export { "choice.png" } else { "choice.pcnd" });
            std::fs::write(&path, b"unrelated file selected for replacement").unwrap();
            if export {
                app.plan_export_settings(path.clone(), export_options::Settings::default());
            } else {
                app.save_selected(&ctx, path.clone());
            }
            assert!(app.confirm.is_some());
            let snapshot = app.editor.document_snapshot();
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(&source, &path).unwrap();
            app = choose(app, if export { "Export now" } else { "Continue" });
            settle(&mut app, &ctx);
            assert_eq!(std::fs::read(&source).unwrap(), bytes);
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert!(app.bound.is_none());
            assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn cancelled_replacements_preserve_loaded_resource_sources_and_document_state() {
    for shape in [false, true] {
        for export in [false, true] {
            let root = scratch();
            let (mut app, source, bytes) = loaded(&root, shape);
            let ctx = egui::Context::default();
            let path = root.join(if export { "choice.png" } else { "choice.pcnd" });
            std::fs::write(&path, b"unrelated replacement candidate").unwrap();
            if export {
                app.plan_export_settings(path.clone(), export_options::Settings::default());
            } else {
                app.save_selected(&ctx, path.clone());
            }
            let snapshot = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            app = choose(app, "Cancel");
            settle(&mut app, &ctx);
            assert!(app.confirm.is_none());
            assert!(app.bound.is_none());
            assert_eq!(app.asset_source_paths().collect::<Vec<_>>(), [&source]);
            assert_eq!(std::fs::read(&source).unwrap(), bytes);
            assert_eq!(
                std::fs::read(&path).unwrap(),
                b"unrelated replacement candidate"
            );
            assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
            assert_eq!(app.editor.history_view().ticket, ticket);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn flat_export_protects_document_sources_in_other_tabs_and_parked_workspaces() {
    for project in [false, true] {
        for location in 0..3 {
            let root = scratch();
            let (mut app, _, _) = loaded(&root, false);
            let source = root.join(if project { "source.pcnd" } else { "source.png" });
            let bytes = if project {
                paintcraft_formats::pcnd::write(app.editor.document()).unwrap()
            } else {
                image::encode(ImageFormat::Png, 1, 1, &[2, 3, 4, 255], 90).unwrap()
            };
            std::fs::write(&source, &bytes).unwrap();
            app.open_path(&source).unwrap();
            let ctx = egui::Context::default();
            match location {
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
            let alias = root.join("export.png");
            std::fs::hard_link(&source, &alias).unwrap();
            let snapshot = app.editor.document_snapshot();
            app.plan_export_settings(alias.clone(), export_options::Settings::default());
            assert!(app.confirm.is_none());
            app.do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
            assert_eq!(std::fs::read(&source).unwrap(), bytes);
            assert_eq!(std::fs::read(&alias).unwrap(), bytes);
            assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn resource_source_paths_remain_protected_while_studio_documents_are_parked() {
    let root = scratch();
    let (mut app, palette, _) = loaded(&root, false);
    let shape = root.join("second.csh");
    std::fs::write(&shape, CSH).unwrap();
    app.install_shape(CSH, shape.clone()).unwrap();
    let mut parked = tabs::WorkspaceDocuments::new((2, 2), false);
    parked.swap(&mut app);
    app.other_workspace = Some(Box::new(parked));
    app.settings.experience = Experience::Classic;
    let ctx = egui::Context::default();
    for (index, (source, bytes)) in [(&palette, ASE), (&shape, CSH)].into_iter().enumerate() {
        let project = root.join(format!("alias-{index}.pcnd"));
        let image = root.join(format!("alias-{index}.png"));
        std::fs::hard_link(source, &project).unwrap();
        std::fs::hard_link(source, &image).unwrap();
        app.save_selected(&ctx, project.clone());
        confirm_save_if_offered(&mut app, &ctx);
        app.do_export(&image, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
        assert_eq!(std::fs::read(source).unwrap(), bytes);
        assert_eq!(std::fs::read(project).unwrap(), bytes);
        assert_eq!(std::fs::read(image).unwrap(), bytes);
        assert!(app.bound.is_none());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_missing_resource_source_path_is_not_recreated_as_a_project() {
    for shape in [false, true] {
        let root = scratch();
        let (mut app, source, _) = loaded(&root, shape);
        std::fs::remove_file(&source).unwrap();
        let ctx = egui::Context::default();
        app.save_selected(&ctx, source.clone());
        settle(&mut app, &ctx);
        assert!(!source.exists());
        assert!(app.bound.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}

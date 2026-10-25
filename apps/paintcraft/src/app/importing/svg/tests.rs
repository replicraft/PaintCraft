use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{self, Language, translate};

const SOURCE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="#e04020"/><text x="2" y="12" font-family="Selawik" font-size="8">Paint</text></svg>"##;
fn app(path: &Path) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.import_selected(path.to_path_buf());
    let Candidate::Svg(source) = &mut app.pending_import.as_mut().unwrap().candidate else {
        panic!("expected SVG options");
    };
    source.state.config.width = 64;
    source.state.config.height = 32;
    app
}
#[test]
fn svg_options_preview_and_consent_publish_only_a_private_native_copy() {
    let dir = std::env::temp_dir().join(format!("paintcraft-svg-ui-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("source.svg");
    std::fs::write(&path, SOURCE).unwrap();
    for language in [Language::English, Language::Japanese] {
        let app = app(&path);
        assert!(
            app.format_filters(Kind::Import)[0]
                .extensions
                .contains(&"svg".into())
        );
        assert!(
            !app.format_filters(Kind::Open)[0]
                .extensions
                .contains(&"svg".into())
        );
        let before = app.editor.document_snapshot();
        let mut h = Harness::builder()
            .with_max_steps(16)
            .with_size(Vec2::new(1360.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut App| {
                    i18n::set_language(ui.ctx(), language);
                    app.import_dialog(ui.ctx());
                },
                app,
            );
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Update preview"))
            .click();
        h.run();
        assert!(
            h.query_by_label(&translate(language, "Import preview"))
                .is_some()
        );
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        h.get_by_label(&translate(language, "I accept these import changes."))
            .click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Import image"))
            .click();
        h.run();
        assert!(h.state().pending_import.is_none());
        let doc = h.state().editor.document();
        assert_eq!(doc.canvas, (64, 32));
        assert_eq!(doc.roots.len(), 1);
        assert_eq!(
            doc.layers[&doc.roots[0]].raster.get(50, 20).0,
            [224, 64, 32, 255]
        );
        assert!(h.state().editor.is_dirty());
        assert!(h.state().bound.is_none());
        assert_eq!(h.state().imported_source.as_deref(), Some(path.as_path()));
        assert!(
            paintcraft_formats::pcnd::read(&paintcraft_formats::pcnd::write_compact(doc).unwrap())
                .unwrap()
                .same_semantics(doc)
        );
        assert_eq!(std::fs::read(&path).unwrap(), SOURCE);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn svg_cancellation_profile_and_stale_owner_keep_the_document() {
    let ctx = egui::Context::default();
    let path = PathBuf::from("source.svg");
    let mut app = App::new(None);
    assert!(app.stage_svg_import(SOURCE.to_vec(), path.clone()).is_err());
    app.settings.experience = Experience::Studio;
    let before = app.editor.document_snapshot();
    app.stage_svg_import(SOURCE.to_vec(), path.clone()).unwrap();
    app.resolve_import(&ctx, false);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.stage_svg_import(SOURCE.to_vec(), path).unwrap();
    app.settings.experience = Experience::Classic;
    app.svg_import_dialog(&ctx);
    assert!(app.pending_import.is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
}

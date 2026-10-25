use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn app(path: &Path) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(
        paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255], "icon-ui")
            .unwrap(),
        None,
    );
    app.path_text = path.with_extension("pcnd").to_string_lossy().into_owned();
    app.begin_icon_export();
    app.icon_export.as_mut().unwrap().draft.config.entries = vec![
        widget::Entry {
            width: 16,
            height: 16,
            ..Default::default()
        },
        widget::Entry {
            width: 32,
            height: 32,
            encoding: widget::Encoding::Bitmap,
            ..Default::default()
        },
    ];
    app
}
fn stage_bytes(app: &mut App, ctx: &egui::Context) {
    let mut state = app.icon_export.take().unwrap();
    let config = state.draft.config.clone();
    let doc = app.editor.export_snapshot().unwrap();
    state.cache.update(ctx, config.clone(), || {
        icons::prepare_document(
            &doc,
            kind(&config),
            &specs(&config),
            icons::Filter::Nearest,
            icons::WriteLimits::default(),
            &|| Ok(()),
        )
        .map_err(|error| error.to_string())
    });
    let prepared = state.cache.value().unwrap();
    state.bytes = Some(prepared.encode(prepared.losses(), &|| Ok(())).unwrap());
    state.draft.accepted = true;
    app.icon_export = Some(state);
}

#[test]
fn native_icon_dialog_requires_consent_and_writes_the_previewed_entry_set() {
    use paintcraft_ui::i18n::{Language, translate};
    for language in [Language::English, Language::Japanese] {
        let path = std::env::temp_dir().join(format!("paintcraft-icon-ui-{}.ico", Uuid::new_v4()));
        let mut app = app(&path);
        app.settings.language = language;
        let before = app.editor.document_snapshot();
        let ticket = app.editor.render_key();
        let dirty = app.editor.is_dirty();
        let mut h = Harness::builder()
            .with_size(egui::vec2(900.0, 1000.0))
            .build_ui_state(|ui, app: &mut App| app.confirm_dialog(ui), app);
        paintcraft_ui::i18n::set_language(&h.ctx, language);
        h.run();
        let tr = |text| translate(language, text);
        assert!(
            h.get_by_label(&tr("Choose destination…"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&tr("I accept these export changes."))
            .click();
        h.run();
        assert!(
            !h.get_by_label(&tr("Choose destination…"))
                .accesskit_node()
                .is_disabled()
        );
        let prepared = h
            .state()
            .icon_export
            .as_ref()
            .unwrap()
            .cache
            .value()
            .unwrap();
        let expected = prepared.encode(prepared.losses(), &|| Ok(())).unwrap();
        h.get_by_label(&tr("Choose destination…")).click();
        h.run();
        assert!(!path.exists());
        assert_eq!(
            h.state().confirm,
            Some(Confirm::IconExport),
            "{}",
            h.state().editor.status()
        );
        h.get_by_label(&tr("Export now")).click();
        h.run();
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.render_key(), ticket);
        assert_eq!(h.state().editor.is_dirty(), dirty);
        assert!(h.state().bound.is_none());
        assert!(h.state().icon_export.is_none());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn stale_picker_destination_changes_and_source_aliases_cannot_publish() {
    let ctx = egui::Context::default();
    let folder = std::env::temp_dir().join(format!("paintcraft-icon-guards-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("output.ico");
    let mut stale = app(&path);
    stage_bytes(&mut stale, &ctx);
    stale.editor = Editor::with_document(stale.editor.document().clone(), None);
    let before = stale.editor.document_snapshot();
    stale.complete_icon_picker(&ctx, Some(path.clone()));
    stale.finish_icon_export(&ctx);
    assert!(!path.exists());
    assert!(Arc::ptr_eq(&before, &stale.editor.document_snapshot()));
    let mut changed = app(&path);
    stage_bytes(&mut changed, &ctx);
    std::fs::write(&path, b"original destination").unwrap();
    changed.complete_icon_picker(&ctx, Some(path.clone()));
    std::fs::write(&path, b"external edit").unwrap();
    changed.finish_icon_export(&ctx);
    assert_eq!(std::fs::read(&path).unwrap(), b"external edit");
    let source = folder.join("source.ico");
    std::fs::write(&source, b"foreign source").unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::hard_link(&source, &path).unwrap();
    let mut protected = app(&path);
    protected.imported_source = Some(source.clone());
    stage_bytes(&mut protected, &ctx);
    protected.complete_icon_picker(&ctx, Some(path.clone()));
    protected.finish_icon_export(&ctx);
    assert_eq!(std::fs::read(&source).unwrap(), b"foreign source");
    assert_eq!(std::fs::read(&path).unwrap(), b"foreign source");
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn imported_source_provenance_follows_its_document_across_tabs() {
    let ctx = egui::Context::default();
    let mut app = app(Path::new("output.ico"));
    app.confirm = None;
    app.icon_export = None;
    let original = app.active_tab_id();
    app.imported_source = Some("original.cur".into());
    app.add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
        .unwrap();
    assert!(app.imported_source.is_none());
    app.activate_tab(original, &ctx);
    assert_eq!(
        app.imported_source.as_deref(),
        Some(Path::new("original.cur"))
    );
}

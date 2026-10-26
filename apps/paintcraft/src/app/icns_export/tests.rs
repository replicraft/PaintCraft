use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, format_message, translate};
fn app(path: &Path) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(
        paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255], "icns-ui")
            .unwrap(),
        None,
    );
    app.path_text = path.with_extension("pcnd").to_string_lossy().into_owned();
    app.begin_icns_export();
    app
}
fn label(lang: Language, kind: widget::Kind) -> String {
    format_message(
        lang,
        "{ $tag }: { $logical } × { $logical } at { $scale }×, { $pixels } × { $pixels } px",
        &[
            ("tag", kind.tag().into()),
            ("logical", kind.logical_size().to_string()),
            ("scale", kind.scale().to_string()),
            ("pixels", kind.pixel_size().to_string()),
        ],
    )
}
fn stage(app: &mut App, ctx: &egui::Context) {
    let mut state = app.icns_export.take().unwrap();
    state.draft.config.kinds = vec![widget::Kind::Retina16];
    let config = state.draft.config.clone();
    let doc = app.editor.export_snapshot().unwrap();
    state.cache.update(ctx, config.clone(), || {
        icns::prepare_document(&doc, &specs(&config), icons::Filter::Nearest, &|| Ok(()))
            .map_err(|e| e.to_string())
    });
    let prepared = state.cache.value().unwrap();
    state.bytes = Some(prepared.encode(prepared.losses(), &|| Ok(())).unwrap());
    state.draft.accepted = true;
    app.icns_export = Some(state);
}
#[test]
fn translated_icns_controls_require_explicit_types_preview_consent_and_write_exact_reviewed_pixels()
{
    for lang in [Language::English, Language::Japanese] {
        let path = std::env::temp_dir().join(format!("paintcraft-icns-ui-{}.icns", Uuid::new_v4()));
        let app = app(&path);
        let before = app.editor.document_snapshot();
        let dirty = app.editor.is_dirty();
        let key = app.editor.render_key();
        let mut h = Harness::builder()
            .with_max_steps(30)
            .with_size(egui::vec2(1000.0, 1100.0))
            .build_ui_state(
                move |ui, app: &mut App| {
                    if ui.ctx().missing_glyph_policy()
                        != egui::epaint::text::MissingGlyphPolicy::Tofu
                    {
                        paintcraft_ui::fonts::install(ui.ctx());
                        ui.ctx()
                            .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                    }
                    paintcraft_ui::i18n::set_language(ui.ctx(), lang);
                    app.confirm_dialog(ui);
                },
                app,
            );
        h.run();
        assert!(
            h.get_by_label(&translate(lang, "Choose destination…"))
                .accesskit_node()
                .is_disabled()
        );
        for kind in [widget::Kind::Size128, widget::Kind::Retina16] {
            h.get_by_role_and_label(egui::Role::CheckBox, &label(lang, kind))
                .click();
            h.run();
        }
        let consent = translate(lang, "I accept these export changes.");
        h.get_by_role_and_label(egui::Role::CheckBox, &consent)
            .scroll_to_me();
        h.run();
        assert!(
            h.get_by_label(&translate(lang, "Choose destination…"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_role_and_label(egui::Role::CheckBox, &consent)
            .click();
        h.run();
        let prepared = h
            .state()
            .icns_export
            .as_ref()
            .unwrap()
            .cache
            .value()
            .unwrap();
        let expected = prepared.encode(prepared.losses(), &|| Ok(())).unwrap();
        let entries = icns::inspect(&expected, &|| Ok(())).unwrap().entries;
        assert_eq!(
            entries.iter().map(|e| e.tag).collect::<Vec<_>>(),
            [*b"ic07", *b"ic11"]
        );
        assert_eq!(
            (entries[1].logical_width, entries[1].scale, entries[1].width),
            (16, 2, 32)
        );
        h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Choose destination…"))
            .click();
        h.run();
        assert!(!path.exists());
        assert_eq!(
            h.state().confirm,
            Some(Confirm::IcnsExport),
            "{}",
            h.state().editor.status()
        );
        h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Export now"))
            .click();
        h.run();
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert_eq!(key, h.state().editor.render_key());
        assert_eq!(dirty, h.state().editor.is_dirty());
        assert!(h.state().icns_export.is_none());
        drop(h);
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn icns_export_stale_picker_cancel_alias_and_replaced_destination_leave_sources_unchanged() {
    let ctx = egui::Context::default();
    let root = std::env::temp_dir().join(format!("paintcraft-icns-export-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("copy.icns");
    let mut stale = app(&path);
    stage(&mut stale, &ctx);
    stale.editor = Editor::with_document(stale.editor.document().clone(), None);
    stale.complete_icns_picker(&ctx, Some(path.clone()));
    stale.finish_icns_export(&ctx);
    assert!(!path.exists());
    let mut cancel = app(&path);
    stage(&mut cancel, &ctx);
    let before = cancel.editor.document_snapshot();
    cancel.complete_icns_picker(&ctx, None);
    cancel.finish_icns_export(&ctx);
    assert!(!path.exists());
    assert!(Arc::ptr_eq(&before, &cancel.editor.document_snapshot()));
    let mut changed = app(&path);
    stage(&mut changed, &ctx);
    std::fs::write(&path, b"original").unwrap();
    changed.complete_icns_picker(&ctx, Some(path.clone()));
    std::fs::write(&path, b"external change").unwrap();
    changed.finish_icns_export(&ctx);
    assert_eq!(std::fs::read(&path).unwrap(), b"external change");
    let source = root.join("painting.pcnd");
    let document =
        paintcraft_document::new_document(2, 2, None, 0, paintcraft_document::POLICY).unwrap();
    let bytes = paintcraft_formats::pcnd::write(&document).unwrap();
    std::fs::write(&source, &bytes).unwrap();
    let alias = root.join("source-alias.icns");
    std::fs::hard_link(&source, &alias).unwrap();
    let mut alias_app = app(&path);
    alias_app.confirm = None;
    alias_app.icns_export = None;
    alias_app.open_path(&source).unwrap();
    alias_app.capture_icns_export();
    stage(&mut alias_app, &ctx);
    alias_app.complete_icns_picker(&ctx, Some(alias.clone()));
    alias_app.finish_icns_export(&ctx);
    assert_eq!(std::fs::read(alias).unwrap(), bytes);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn a_newly_acquired_import_source_after_destination_review_cannot_be_overwritten() {
    let ctx = egui::Context::default();
    let root = std::env::temp_dir().join(format!("paintcraft-icns-late-source-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("source.icns");
    let original = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/icns-fixtures/modern.icns"
    );
    std::fs::write(&path, original).unwrap();
    let mut app = app(&path);
    stage(&mut app, &ctx);
    app.complete_icns_picker(&ctx, Some(path.clone()));
    assert!(app.icns_export.as_ref().unwrap().destination.is_some());
    let before = app.editor.document_snapshot();
    app.import_selected(path.clone());
    assert!(app.pending_import.is_some(), "{}", app.editor.status());
    app.finish_icns_export(&ctx);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert!(app.pending_import.is_some());
    std::fs::remove_dir_all(root).unwrap();
}

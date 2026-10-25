use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn browser_icon_dialog_requires_consent_and_preserves_project_name_and_history() {
    use paintcraft_ui::i18n::{Language, format_message, set_language, translate};
    for language in [Language::English, Language::Japanese] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1000.0, 1100.0))
            .build_eframe(|cc| {
                let mut app = WebApp::new(&cc.egui_ctx);
                app.frontend = Frontend::Studio;
                let doc = paintcraft_document::import_image(
                    2,
                    1,
                    &[255, 0, 0, 255, 0, 0, 255, 255],
                    "web-icon-ui",
                )
                .unwrap();
                app.editor = Editor::from_document(doc).unwrap();
                app.name = "painting.pcnd".into();
                app.begin_icon_export();
                let state = app.icon_export.as_mut().unwrap();
                state.draft.config.kind = widget::Kind::Cursor;
                state.draft.config.entries = vec![
                    widget::Entry {
                        width: 16,
                        height: 16,
                        hotspot: (3, 7),
                        ..Default::default()
                    },
                    widget::Entry {
                        width: 32,
                        height: 32,
                        encoding: widget::Encoding::Bitmap,
                        hotspot: (9, 11),
                    },
                ];
                set_language(&cc.egui_ctx, language);
                app
            });
        h.run();
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        let dirty = h.state().editor.session.is_dirty();
        let download = format_message(
            language,
            "Download { $format }",
            &[("format", "CUR".into())],
        );
        assert!(h.get_by_label(&download).accesskit_node().is_disabled());
        h.get_by_label(&translate(language, "I accept these export changes."))
            .click();
        h.run();
        assert!(!h.get_by_label(&download).accesskit_node().is_disabled());
        let prepared = h
            .state()
            .icon_export
            .as_ref()
            .unwrap()
            .cache
            .value()
            .unwrap();
        let bytes = prepared.encode(prepared.losses(), &|| Ok(())).unwrap();
        let entries = icons::inspect(&bytes).unwrap().entries;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].hotspot, Some((3, 7)));
        assert_eq!(entries[1].hotspot, Some((9, 11)));
        h.get_by_label(&download).click();
        h.run();
        assert!(h.state().icon_export.is_none());
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert_eq!(h.state().editor.session.is_dirty(), dirty);
        assert_eq!(h.state().name, "painting.pcnd");
    }
}

#[test]
fn invalid_bitmap_policy_changed_source_and_cancel_cannot_export() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 1100.0))
        .build_eframe(|cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.frontend = Frontend::Studio;
            app.editor = Editor::from_document(
                paintcraft_document::new_document(2, 1, None, 0, paintcraft_document::POLICY)
                    .unwrap(),
            )
            .unwrap();
            app.begin_icon_export();
            app
        });
    h.run();
    h.get_by_label("I accept these export changes.").click();
    h.run();
    assert!(
        !h.get_by_label("Download ICO")
            .accesskit_node()
            .is_disabled()
    );
    h.state_mut()
        .icon_export
        .as_mut()
        .unwrap()
        .draft
        .config
        .entries[0]
        .encoding = widget::Encoding::Bitmap;
    h.run();
    assert!(
        h.get_by_label("Download ICO")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        h.query_by_label(
            "Icon preview is unavailable. Check the entry settings or reduce the canvas size."
        )
        .is_some()
    );
    h.state_mut()
        .icon_export
        .as_mut()
        .unwrap()
        .draft
        .config
        .entries[0]
        .encoding = widget::Encoding::Png;
    h.run();
    assert!(!h.state().icon_export.as_ref().unwrap().draft.accepted);
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().icon_export.is_none());
    h.state_mut().begin_icon_export();
    h.state_mut().editor = Editor::new(3, 3).unwrap();
    let before = h.state().editor.session.snapshot();
    h.run();
    assert!(h.state().icon_export.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
}

#[test]
fn narrow_browser_dialog_keeps_consent_and_download_reachable() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(390.0, 640.0))
        .build_eframe(|cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.frontend = Frontend::Studio;
            app.editor = Editor::new(2, 1).unwrap();
            app.begin_icon_export();
            app
        });
    h.run();
    h.get_by_label("I accept these export changes.")
        .scroll_to_me();
    h.run();
    h.get_by_label("I accept these export changes.").click();
    h.run();
    assert!(
        !h.get_by_label("Download ICO")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().icon_export.is_none());
}

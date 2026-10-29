//! Real consent binds the immutable text-copy snapshot, not the live base state.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, set_language, translate};
fn app() -> App {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ));
    let attached = paintcraft_formats::pcnd::read(bytes).unwrap();
    let mut doc = paintcraft_document::new_document(
        160,
        64,
        Some(paintcraft_core::types::Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.source_metadata = attached.source_metadata;
    let mut app = App::new(None);
    app.editor = Editor::with_document(doc, None);
    app
}
fn harness(app: App, language: Language) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1000., 1000.))
        .build_ui_state(
            move |ui, app: &mut App| {
                set_language(ui.ctx(), language);
                app.confirm_dialog(ui);
            },
            app,
        )
}
const CHECK: &str = "Omit original source metadata from this exported copy";
#[test]
fn every_language_requires_explicit_metadata_checkbox_and_cancel_keeps_attachment() {
    for language in Language::ALL {
        let mut app = app();
        let source = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        app.begin_export_options(ImageExport::Png);
        let mut h = harness(app, language);
        h.run();
        let tr = |source| translate(language, source);
        assert!(
            h.get_by_label(&tr("Choose destination…"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&tr(CHECK)).click();
        h.run();
        assert!(
            !h.get_by_label(&tr("Choose destination…"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&tr("Cancel")).click();
        h.run();
        assert!(h.state().confirm.is_none());
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(ticket, h.state().editor.history_view().ticket);
        assert!(h.state().editor.document().source_metadata.is_some());
    }
}
#[test]
fn text_draft_exports_the_once_frozen_copy_and_keeps_editable_text_history_and_metadata() {
    let mut app = app();
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((4, 4));
    app.editor.pointer_up();
    app.editor.type_text("Ink");
    let source = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let path =
        std::env::temp_dir().join(format!("paintcraft-metadata-text-{}.png", Uuid::new_v4()));
    app.path_text = path.to_string_lossy().into_owned();
    app.begin_export_options(ImageExport::Png);
    let Some(Confirm::ExportOptions(draft)) = &app.confirm else {
        panic!("options");
    };
    let frozen = draft.frozen.clone();
    assert_ne!(frozen.document.state_id, source.state_id);
    let expected = paintcraft_render::render_flat(&frozen.document).unwrap();
    let mut h = harness(app, Language::English);
    h.run();
    h.get_by_label(CHECK).click();
    h.run();
    h.get_by_label("Choose destination…").click();
    h.run();
    let Some(Confirm::Export {
        frozen: captured,
        losses,
        ..
    }) = &h.state().confirm
    else {
        panic!("review");
    };
    assert!(Arc::ptr_eq(&captured.document, &frozen.document));
    assert_eq!(
        losses
            .iter()
            .filter(|loss| loss.as_str() == export::metadata_policy::OMITTED_REASON)
            .count(),
        1
    );
    h.get_by_label("Export now").click();
    h.run();
    let decoded = paintcraft_formats::image::decode(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(decoded.rgba, expected);
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(ticket, h.state().editor.history_view().ticket);
    assert!(h.state().editor.is_editing_text());
    assert!(h.state().editor.document().source_metadata.is_some());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn changed_text_and_cancelled_picker_drop_authority_without_export() {
    let mut app = app();
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((4, 4));
    app.editor.pointer_up();
    app.editor.type_text("Ink");
    app.begin_export_options(ImageExport::Png);
    let Some(Confirm::ExportOptions(draft)) = &app.confirm else {
        panic!("options");
    };
    let frozen = draft.frozen.clone();
    app.editor.type_text("changed");
    assert!(!frozen.current(&app));
    let mut h = harness(app, Language::English);
    h.run();
    assert!(h.state().confirm.is_none());
    h.state_mut().export_dialog_frozen = Some(frozen);
    h.state_mut().export_dialog_metadata = Intent::Omit;
    h.state_mut().complete_dialog(
        &egui::Context::default(),
        paintcraft_platform::dialogs::Completion {
            kind: Kind::Export,
            path: None,
        },
    );
    assert!(h.state().export_dialog_frozen.is_none());
    assert_eq!(h.state().export_dialog_metadata, Intent::Refuse);
    assert!(h.state().editor.document().source_metadata.is_some());
}

#[test]
fn export_boundary_captures_earlier_edit_and_holds_later_work_until_cancel() {
    let mut app = app();
    let context = egui::Context::default();
    app.context = context.clone();
    app.initialize_editing(context.clone());
    let release = app.editing.as_ref().unwrap().pause();
    app.editor.request(
        9001,
        Action::ClearCanvas {
            color: Rgba([0, 0, 0, 255]),
        },
    );
    app.begin_export_options(ImageExport::Png);
    assert_eq!(app.deferred_files.len(), 1);
    app.editor.request(
        9002,
        Action::ClearCanvas {
            color: Rgba([255, 0, 0, 255]),
        },
    );
    release.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        app.poll_edits(&context);
        if matches!(app.confirm, Some(Confirm::ExportOptions(_))) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "export boundary stalled"
        );
        std::thread::yield_now();
    }
    let Some(Confirm::ExportOptions(draft)) = &app.confirm else {
        unreachable!()
    };
    assert!(draft.frozen.current(&app));
    assert_eq!(
        draft.frozen.document.layers[&draft.frozen.document.roots[0]]
            .raster
            .default_pixel(),
        Rgba([0, 0, 0, 255])
    );
    assert!(draft.frozen.document.source_metadata.is_some());
    app.confirm = None;
    loop {
        app.poll_edits(&context);
        if !app.any_pending_edits() && !app.any_deferred_files() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "queued edit did not resume"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        app.editor.document().layers[&app.editor.document().roots[0]]
            .raster
            .default_pixel(),
        Rgba([255, 0, 0, 255])
    );
    assert!(app.editor.document().source_metadata.is_some());
}

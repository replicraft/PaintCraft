//! Real metadata-bound placement refusal, deferred save cancellation and explicit copy rescue.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_document::source_metadata::{BYTES_MAX, SourceMetadata};
use paintcraft_ui::i18n::{Language, set_language};
use std::{fs, time::Instant};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/native-retained-text/runs")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
pub(crate) fn boundary() -> paintcraft_document::Document {
    let original = paintcraft_formats::pcnd::read(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    )))
    .unwrap();
    let original = original.source_metadata.unwrap();
    let mut doc = paintcraft_document::new_document(
        128,
        64,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let framing = paintcraft_formats::pcnd::write_compact(&doc).unwrap().len() - 32
        + paintcraft_document::source_metadata::WIRE_HEADER_BYTES;
    let bytes = (8 << 20) - framing;
    let mut profile = original.profile().unwrap().to_vec();
    profile.resize(bytes, 0);
    profile[..4].copy_from_slice(&(bytes as u32).to_be_bytes());
    let realm = paintcraft_core::types::resources::SharedBudget::new(BYTES_MAX + 4096);
    doc.source_metadata = Some(
        SourceMetadata::capture(
            *original.descriptor(),
            Some(&profile),
            BYTES_MAX,
            &realm,
            &|| Ok(()),
        )
        .unwrap(),
    );
    assert_eq!(
        paintcraft_formats::pcnd::write_compact(&doc).unwrap().len(),
        (8 << 20) + 32
    );
    doc
}
#[test]
fn real_limit_refusal_preserves_private_text_history_save_destination_and_explicit_export_rescue() {
    for asynchronous in [false, true] {
        let folder = Folder::new();
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.editor = Editor::with_document(boundary(), None);
        app.editor.test_persistent_validator(Some(
            paintcraft_formats::pcnd::check_compact_metadata_admission,
        ));
        let layer = app.editor.document().roots[0];
        app.editor.request(
            91999,
            Action::Rename {
                layer,
                name: "Foreground".into(),
            },
        );
        app.editor.run_pending();
        assert_eq!(app.editor.undo_depth(), 1);
        assert_eq!(
            paintcraft_formats::pcnd::write_compact(app.editor.document())
                .unwrap()
                .len(),
            (8 << 20) + 32
        );
        let ctx = egui::Context::default();
        if asynchronous {
            app.initialize_editing(ctx.clone());
        }
        app.editor.select_tool(Tool::Text);
        app.editor.pointer_down((2, 2));
        app.editor.pointer_up();
        app.editor.type_text("Ink");
        let before = app.editor.document_snapshot();
        let history = app.editor.history_view();
        let dirty = app.editor.is_dirty();
        let source_digest = before.source_metadata.as_ref().unwrap().digest();
        let destination = folder.0.join("existing.pcnd");
        fs::write(&destination, b"unchanged destination sentinel").unwrap();
        app.close_after_edits = true;
        app.tab_save_close = Some((app.active_tab_id(), before.state_id));
        app.do_save(&ctx, destination.clone());
        let deadline = Instant::now() + Duration::from_secs(10);
        while !app.editor.failed_text_retained()
            || !app.editor.can_capture_export()
            || !app.deferred_files.is_empty()
        {
            app.poll_edits(&ctx);
            assert!(Instant::now() < deadline, "{}", app.editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(app.editor.history_view(), history);
        assert_eq!(app.editor.is_dirty(), dirty);
        assert_eq!(app.editor.canvas_text_content(), Some("Ink"));
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"unchanged destination sentinel"
        );
        assert!(!app.close_after_edits);
        assert!(app.tab_save_close.is_none());
        assert!(app.deferred_files.is_empty());
        // A later Undo must stay behind the failed text; export must exclude it.
        app.editor.request(92001, Action::Undo);
        app.editor.run_pending();
        let output = folder.0.join("rescued.png");
        app.path_text = output.to_string_lossy().into_owned();
        app.begin_export_options(paintcraft_ui::actions::ImageExport::Png);
        assert!(
            matches!(app.confirm, Some(Confirm::ExportOptions(_))),
            "failed text must be rescuable through real options"
        );
        let expected_copy = app.editor.export_snapshot().unwrap();
        let expected = paintcraft_render::render_flat(&expected_copy).unwrap();
        assert!(!expected_copy.same_semantics(&before));
        let mut h = Harness::builder()
            .with_size(egui::vec2(1000., 1000.))
            .build_ui_state(
                |ui, app: &mut App| {
                    set_language(ui.ctx(), Language::English);
                    app.confirm_dialog(ui);
                },
                app,
            );
        h.run();
        h.get_by_label("Omit original source metadata from this exported copy")
            .click();
        h.run();
        h.get_by_label("Choose destination…").click();
        h.run();
        let Some(Confirm::Export {
            frozen: captured,
            losses,
            ..
        }) = &h.state().confirm
        else {
            panic!("export review");
        };
        assert!(captured.document.same_semantics(&expected_copy));
        assert_eq!(captured.document.state_id, expected_copy.state_id);
        assert_eq!(
            losses
                .iter()
                .filter(|loss| loss.as_str()
                    == paintcraft_formats::export::metadata_policy::OMITTED_REASON)
                .count(),
            1
        );
        h.get_by_label("Export now").click();
        h.run();
        let decoded = paintcraft_formats::image::decode(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(decoded.rgba, expected);
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        assert_eq!(h.state().editor.canvas_text_content(), Some("Ink"));
        assert_eq!(
            h.state()
                .editor
                .document()
                .source_metadata
                .as_ref()
                .unwrap()
                .digest(),
            source_digest
        );
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"unchanged destination sentinel"
        );
        h.state_mut().editor.cancel_text();
        h.state_mut().editor.run_pending();
        assert_eq!(
            h.state().editor.history_view(),
            history,
            "Escape must discard held Undo"
        );
    }
}

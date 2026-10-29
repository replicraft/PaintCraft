//! Actual advanced selectors/encoded previews retain one frozen copy authority.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::{
    fs,
    time::{Duration, Instant},
};
const CHECK: &str = "Omit original source metadata from this exported copy";
fn app() -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let metadata=paintcraft_formats::pcnd::read(include_bytes!("../../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd")).unwrap().source_metadata;
    let doc = paintcraft_document::new_document(
        64,
        32,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap()
    .with_source_metadata_checked(metadata, &|| Ok(()))
    .unwrap();
    app.editor = Editor::with_document(doc, None);
    app.initialize_editing(app.context.clone());
    app
}
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/native-advanced-metadata")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn begin(mut app: App, format: ImageExport) -> (Harness<'static, App>, Frozen) {
    app.begin_export_options(format);
    let Some(Confirm::ExportOptions(draft)) = &mut app.confirm else {
        panic!("options")
    };
    if format == ImageExport::Dds {
        draft.dds.mips = paintcraft_ui::dds_export::Mips::Full;
    }
    let frozen = draft.frozen.clone();
    let mut h = Harness::builder()
        .with_max_steps(40)
        .with_size(egui::vec2(1100., 1000.))
        .build_ui_state(
            |ui, app: &mut App| {
                app.poll_edits(ui.ctx());
                app.poll_dds_export(ui.ctx());
                app.poll_msp_export(ui.ctx());
                app.confirm_dialog(ui);
            },
            app,
        );
    h.run();
    h.get_by_label("Preview export").click();
    h.run();
    (h, frozen)
}
fn ready(h: &mut Harness<App>) {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        h.step();
        if !h
            .get_by_label("Choose destination…")
            .accesskit_node()
            .is_disabled()
        {
            break;
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    h.run();
}
#[test]
fn advanced_dds_and_msp_explicit_omission_publish_encoded_pixels_and_keep_source_metadata() {
    for format in [ImageExport::Dds, ImageExport::Msp] {
        let folder = Folder::new();
        let extension = if format == ImageExport::Dds {
            "dds"
        } else {
            "msp"
        };
        let target = folder.0.join(format!("copy.{extension}"));
        let mut app = app();
        app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
        if format == ImageExport::Dds {
            app.editor.select_tool(Tool::Text);
            app.editor.pointer_down((4, 4));
            app.editor.pointer_up();
            app.editor.type_text("Ink");
        }
        let original = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        let (mut h, frozen) = begin(app, format);
        let expected = paintcraft_render::render_flat(&frozen.document).unwrap();
        if format == ImageExport::Dds {
            assert_ne!(frozen.document.state_id, original.state_id);
        }
        assert!(
            h.get_by_label("Choose destination…")
                .accesskit_node()
                .is_disabled()
        );
        assert!(!target.exists());
        h.get_by_label(CHECK).click();
        ready(&mut h);
        h.get_by_label("Choose destination…").click();
        h.run();
        h.get_by_label("Export now").click();
        h.run();
        let bytes = fs::read(&target)
            .unwrap_or_else(|e| panic!("{format:?}: {e}; status={}", h.state().editor.status()));
        let actual = if format == ImageExport::Dds {
            paintcraft_formats::dds::decode_top_mip_checked(&bytes, 4096, &|| Ok(()))
                .unwrap()
                .rgba
        } else {
            paintcraft_formats::msp::decode(&bytes, &|| Ok(()))
                .unwrap()
                .rgba(&|| Ok(()))
                .unwrap()
        };
        assert_eq!(actual, expected);
        assert!(Arc::ptr_eq(
            &original,
            &h.state().editor.document_snapshot()
        ));
        assert_eq!(ticket, h.state().editor.history_view().ticket);
        assert_eq!(dirty, h.state().editor.is_dirty());
        assert_eq!(
            original.source_metadata,
            h.state().editor.document().source_metadata
        );
        let saved = paintcraft_formats::pcnd::write_compact(h.state().editor.document()).unwrap();
        assert_eq!(saved[4], 2);
        assert_eq!(
            paintcraft_formats::pcnd::read(&saved)
                .unwrap()
                .source_metadata,
            original.source_metadata
        );
        if format == ImageExport::Dds {
            assert!(h.state().editor.is_editing_text());
        }
    }
}
#[test]
fn advanced_omission_revocation_cancel_and_changed_text_never_publish_or_mutate_history() {
    for format in [ImageExport::Dds, ImageExport::Msp] {
        let folder = Folder::new();
        let target = folder.0.join(if format == ImageExport::Dds {
            "copy.dds"
        } else {
            "copy.msp"
        });
        fs::write(&target, b"existing").unwrap();
        let mut app = app();
        app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
        let source = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let (mut h, _) = begin(app, format);
        h.get_by_label(CHECK).click();
        ready(&mut h);
        h.get_by_label(CHECK).click();
        h.run();
        assert!(
            h.get_by_label("Choose destination…")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(fs::read(&target).unwrap(), b"existing");
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(ticket, h.state().editor.history_view().ticket);
        assert!(h.state().editor.document().source_metadata.is_some());
    }
    let folder = Folder::new();
    let target = folder.0.join("stale.dds");
    let mut app = app();
    app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((4, 4));
    app.editor.pointer_up();
    app.editor.type_text("Ink");
    let (mut h, _) = begin(app, ImageExport::Dds);
    h.get_by_label(CHECK).click();
    ready(&mut h);
    h.state_mut().editor.type_text(" changed");
    h.run();
    assert!(h.state().dds_export.is_none());
    assert!(!target.exists());
    assert!(h.state().editor.is_editing_text());
}

#[test]
fn advanced_jxr_unchecked_privacy_never_launches_a_helper_and_cancel_keeps_text_editable() {
    let mut app = app();
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((4, 4));
    app.editor.pointer_up();
    app.editor.type_text("Ink");
    let source = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let frozen = Frozen::capture(&app).unwrap();
    assert_ne!(frozen.document.state_id, source.state_id);
    let reserved = app.worker_budget.usage().reserved;
    app.begin_jxr_export_frozen(frozen, Intent::Refuse);
    let mut h = Harness::builder()
        .with_max_steps(40)
        .with_size(egui::vec2(1000., 1000.))
        .build_ui_state(
            |ui, app: &mut App| {
                app.poll_edits(ui.ctx());
                app.poll_jxr_export(ui.ctx());
                app.confirm_dialog(ui);
            },
            app,
        );
    h.run();
    assert!(h.state().jxr_export.is_some());
    assert!(h.query_all_by_label_contains(CHECK).count() > 0);
    assert_eq!(h.state().worker_budget.usage().reserved, reserved);
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().jxr_export.is_none());
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(ticket, h.state().editor.history_view().ticket);
    assert!(h.state().editor.is_editing_text());
    assert!(h.state().editor.document().source_metadata.is_some());
    assert_eq!(h.state().worker_budget.usage().reserved, reserved);
}

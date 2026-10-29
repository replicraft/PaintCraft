use super::*;
use std::{
    fs,
    time::{Duration, Instant},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/stock-source/runs")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, name: &str, format: ImageFormat) -> PathBuf {
        let path = self.0.join(name);
        fs::write(
            &path,
            image::encode(format, 2, 1, &[13, 71, 201, 255, 7, 8, 9, 255], 90).unwrap(),
        )
        .unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn changed_stock_raster_after_consent_capture_cannot_replace_nonempty_work() {
    let ctx = egui::Context::default();
    for replacement in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.write("image.gif", ImageFormat::Gif);
        let mut app = App::new(None);
        app.editor.request(
            719,
            Action::ClearCanvas {
                color: Rgba([9, 10, 11, 255]),
            },
        );
        app.editor.run_pending();
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        app.open_path(&path).unwrap();
        assert!(app.pending_import.is_some());
        let original = fs::read(&path).unwrap();
        if replacement {
            fs::rename(&path, fixture.0.join("original.gif")).unwrap();
            fs::write(&path, &original).unwrap();
        } else {
            let mut changed = original.clone();
            changed[13] ^= 1;
            fs::write(&path, changed).unwrap();
        }
        app.resolve_import(&ctx, true);
        assert!(
            Arc::ptr_eq(&before, &app.editor.document_snapshot()),
            "replacement={replacement}"
        );
        assert_eq!(ticket, app.editor.history_view().ticket);
        assert_eq!(dirty, app.editor.is_dirty());
        assert!(app.bound.is_none());
        assert!(app.recent.entries.is_empty());
    }
}
#[test]
fn stock_raster_frozen_alias_guards_survive_actual_pcnd_save_and_source_path_replacement() {
    for (format, extension) in [
        (ImageFormat::Png, "png"),
        (ImageFormat::Bmp, "bmp"),
        (ImageFormat::Jpeg, "jpg"),
        (ImageFormat::Gif, "gif"),
        (ImageFormat::Tiff, "tiff"),
    ] {
        let fixture = Fixture::new();
        let path = fixture.write(&format!("image.{extension}"), format);
        let bytes = fs::read(&path).unwrap();
        let mut app = App::new(None);
        let ctx = app.context.clone();
        app.open_path(&path).unwrap();
        if app.pending_import.is_some() {
            app.resolve_import(&ctx, true);
        }
        assert_eq!(app.editor.document().canvas, (2, 1));
        let project = fixture.0.join("copy.pcnd");
        app.save_selected(&ctx, project.clone());
        app.editor.run_pending();
        let end = Instant::now() + Duration::from_secs(15);
        while !project.is_file() || app.any_pending_edits() || app.any_deferred_files() {
            app.poll_edits(&ctx);
            app.finish_save();
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
        let doc = paintcraft_formats::pcnd::read(&fs::read(&project).unwrap()).unwrap();
        assert_eq!(doc.canvas, (2, 1));
        let moved = fixture.0.join("moved");
        let alias = fixture.0.join("alias.pcnd");
        fs::rename(&path, &moved).unwrap();
        fs::hard_link(&moved, &alias).unwrap();
        fs::write(&path, b"replacement path").unwrap();
        assert!(app.guard_save_destination(&alias).is_err(), "{extension}");
        assert!(app.guard_export_destination(&alias).is_err(), "{extension}");
        assert_eq!(fs::read(&moved).unwrap(), bytes);
        app.add_document_tab(Editor::new_document(3, 2).unwrap(), &ctx)
            .unwrap();
        assert!(
            app.guard_save_destination(&alias).is_err(),
            "parked {extension}"
        );
        assert!(
            app.guard_export_destination(&alias).is_err(),
            "parked {extension}"
        );
    }
}

#[test]
fn cancelled_or_stale_stock_consent_keeps_existing_history_and_never_retains_unadopted_source() {
    for stale in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.write("image.gif", ImageFormat::Gif);
        let mut app = App::new(None);
        let ctx = app.context.clone();
        app.editor.request(
            720,
            Action::ClearCanvas {
                color: Rgba([9, 10, 11, 255]),
            },
        );
        app.editor.run_pending();
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        app.open_path(&path).unwrap();
        assert!(
            app.pending_import
                .as_ref()
                .unwrap()
                .captured_source
                .is_some()
        );
        if stale {
            app.settings.classic_variant = Some(ClassicVariant::V7);
        }
        app.resolve_import(&ctx, stale);
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(ticket, app.editor.history_view().ticket);
        assert_eq!(dirty, app.editor.is_dirty());
        assert!(app.pending_import.is_none());
        assert!(app.auxiliary_sources.is_empty());
        assert!(app.guard_save_destination(&path).is_ok());
    }
}
